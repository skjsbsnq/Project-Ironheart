//! V5 阶段 C.2 + F.2：政治面板。
//!
//! 默认渲染路径已切到 `vanilla_gui` runtime：加载原版
//! `interface/countrypoliticsview.gui` 的窗口几何、动画和 idea category 模板，
//! 再绑定本项目的政治/法律数据。原版资源不可用时只显示 runtime 诊断，不回退 V9/旧 UI。
//!
//! - C.2：党派色块 + 民众支持率条 + ideas 列表 + 5 顾问槽位（空）
//! - F.2：决议列表 — 5 分类 tab，按可见性筛选，按按钮显示状态：
//!     - 可点（绿色）/ 灰显（条件不满足）/ 冷却中 / 进行中 / 已触发
//!     - 点击后返回命令，由调用方执行。
//!       `DecisionState::activate(...)` 真正执行。

#![allow(dead_code, deprecated)]

use crate::vanilla_gui::VanillaPanelProfile;
use crate::{
    components, i18n::tr, law_panel, vanilla_iron::VanillaIron, ActiveDetailPanel,
    ActivePrimaryPanel, PanelCommand,
};
use egui::{Color32, Pos2, Rect, RichText, Sense, Vec2};

use hoi4_content::{Decision, DecisionCategory, DecisionMechanicKind, Effect};
use hoi4_state::LawCategory;
use std::borrow::Cow;

const PANEL_CARD: Color32 = Color32::from_rgb(0x0d, 0x10, 0x0f);
const PANEL_CARD_SOFT: Color32 = Color32::from_rgb(0x14, 0x18, 0x17);
const STROKE_DARK: Color32 = Color32::from_rgb(0x28, 0x31, 0x31);
const GOOD: Color32 = Color32::from_rgb(0x70, 0xc8, 0x78);
const WARN: Color32 = Color32::from_rgb(0xff, 0xc0, 0x60);
const BAD: Color32 = Color32::from_rgb(0xe0, 0x60, 0x58);
const IDEA_SLOT_SIZE: f32 = 54.0;
const IDEA_ICON_SIZE: f32 = 44.0;
const VANILLA_CARD_GAP: f32 = 6.0;
const VANILLA_IDEA_CATEGORY_ROW_H: f32 = 100.0;
const VANILLA_IDEA_CATEGORY_GAP: f32 = 5.0;
const POLITICS_PANEL_SPRITES: &[&str] = crate::vanilla_gui::POLITICS_REQUIRED_SPRITES;

const COUNTRY_POLITICS_GUI_FILE: &str = crate::vanilla_gui::COUNTRY_POLITICS_GUI_FILE;
const COUNTRY_POLITICS_PROFILE_ID: &str = crate::vanilla_gui::COUNTRY_POLITICS_PROFILE_ID;
const COUNTRY_POLITICS_ROOT: &str = crate::vanilla_gui::COUNTRY_POLITICS_ROOT;
const COUNTRY_POLITICS_IDEA_CATEGORY_TEMPLATE: &str = "country_politics_idea_category_entry";
const COUNTRY_POLITICS_PARTY_TEMPLATE: &str = "political_party_info_entry";

type VanillaPoliticsGuiContext = crate::vanilla_gui::VanillaGuiRuntimeContext;

fn politics_vanilla_gui_context() -> Option<&'static VanillaPoliticsGuiContext> {
    crate::vanilla_gui::country_politics_runtime_context()
}

pub fn warm_country_politics_runtime(icon_bank: &mut crate::icons::IconBank) {
    let context = politics_vanilla_gui_context();
    icon_bank.add_politics_search_dirs();
    let _ = icon_bank.diagnose_sprites(POLITICS_PANEL_SPRITES.iter().copied());
    if let Some(context) = context {
        let sprites = crate::vanilla_gui::collect_profile_gfx_references(
            context,
            &crate::vanilla_gui::COUNTRY_POLITICS_DESCRIPTOR,
        );
        let _ = icon_bank.diagnose_sprites(sprites.iter().map(String::as_str));
    }
}

struct CountryPoliticsProfile;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CountryPoliticsBindingValue {
    Visibility,
    Text,
    TextColor,
    Sprite,
    Tint,
    Progress,
    Pie,
    Tooltip,
    Command,
    Instances,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CountryPoliticsBindingSpec {
    path_pattern: &'static str,
    values: &'static [CountryPoliticsBindingValue],
    reason: &'static str,
}

const COUNTRY_POLITICS_BINDING_SPECS: &[CountryPoliticsBindingSpec] = &[
    CountryPoliticsBindingSpec {
        path_pattern: "countrypoliticsview.hidden-subsystems",
        values: &[CountryPoliticsBindingValue::Visibility],
        reason: "subjects, exiles, occupation and power balance are not wired to game data yet",
    },
    CountryPoliticsBindingSpec {
        path_pattern: "countrypoliticsview.political_title",
        values: &[CountryPoliticsBindingValue::Text],
        reason: "localized panel title",
    },
    CountryPoliticsBindingSpec {
        path_pattern: "countrypoliticsview.leader",
        values: &[
            CountryPoliticsBindingValue::Sprite,
            CountryPoliticsBindingValue::Tooltip,
        ],
        reason: "runtime leader portrait and hover text",
    },
    CountryPoliticsBindingSpec {
        path_pattern: "countrypoliticsview.leader_name",
        values: &[CountryPoliticsBindingValue::Text],
        reason: "runtime leader display name",
    },
    CountryPoliticsBindingSpec {
        path_pattern: "countrypoliticsview.active_goal",
        values: &[CountryPoliticsBindingValue::Tooltip, CountryPoliticsBindingValue::Command],
        reason: "focus tree hit target uses vanilla active_goal geometry",
    },
    CountryPoliticsBindingSpec {
        path_pattern: "countrypoliticsview.active_goal.add_national_goal_button",
        values: &[CountryPoliticsBindingValue::Tooltip, CountryPoliticsBindingValue::Command],
        reason: "focus tree button uses vanilla button sprite and state frames",
    },
    CountryPoliticsBindingSpec {
        path_pattern: "countrypoliticsview.active_goal.title",
        values: &[CountryPoliticsBindingValue::Text],
        reason: "current national focus name",
    },
    CountryPoliticsBindingSpec {
        path_pattern: "countrypoliticsview.active_goal.empty-focus-children",
        values: &[CountryPoliticsBindingValue::Visibility],
        reason: "hide vanilla focus progress chrome when no focus exists",
    },
    CountryPoliticsBindingSpec {
        path_pattern: "countrypoliticsview.active_goal.focus_cost",
        values: &[CountryPoliticsBindingValue::Text],
        reason: "current focus remaining days",
    },
    CountryPoliticsBindingSpec {
        path_pattern: "countrypoliticsview.active_goal.progress",
        values: &[CountryPoliticsBindingValue::Progress],
        reason: "current focus progress value for vanilla progressbartype",
    },
    CountryPoliticsBindingSpec {
        path_pattern: "countrypoliticsview.national_spirit",
        values: &[CountryPoliticsBindingValue::Tooltip],
        reason: "runtime tooltip for the vanilla national spirit header",
    },
    CountryPoliticsBindingSpec {
        path_pattern: "countrypoliticsview.national_spirit.spirit_grid",
        values: &[CountryPoliticsBindingValue::Instances],
        reason: "national spirit slot count",
    },
    CountryPoliticsBindingSpec {
        path_pattern: "countrypoliticsview.national_spirit.spirit_grid.political_idea_entry[*].add_idea_button",
        values: &[
            CountryPoliticsBindingValue::Sprite,
            CountryPoliticsBindingValue::Tooltip,
        ],
        reason: "national spirit icon and tooltip inside vanilla idea slot template",
    },
    CountryPoliticsBindingSpec {
        path_pattern: "countrypoliticsview.ruling_party_info.ideology",
        values: &[CountryPoliticsBindingValue::Text],
        reason: "ruling ideology text",
    },
    CountryPoliticsBindingSpec {
        path_pattern: "countrypoliticsview.ruling_party_info.elections",
        values: &[CountryPoliticsBindingValue::Text],
        reason: "election status text",
    },
    CountryPoliticsBindingSpec {
        path_pattern: "countrypoliticsview.political_pie_chart",
        values: &[CountryPoliticsBindingValue::Pie],
        reason: "party popularity values for vanilla pie chart",
    },
    CountryPoliticsBindingSpec {
        path_pattern: "countrypoliticsview.parties_grid",
        values: &[CountryPoliticsBindingValue::Instances],
        reason: "party row count",
    },
    CountryPoliticsBindingSpec {
        path_pattern: "countrypoliticsview.parties_grid.political_party_info_entry[*].name",
        values: &[
            CountryPoliticsBindingValue::Text,
            CountryPoliticsBindingValue::TextColor,
        ],
        reason: "party row name text",
    },
    CountryPoliticsBindingSpec {
        path_pattern: "countrypoliticsview.parties_grid.political_party_info_entry[*].leading_pol_party_bg",
        values: &[CountryPoliticsBindingValue::Visibility],
        reason: "vanilla row background sprite stays unmodified; party score is engine-provided, not a profile overlay",
    },
    CountryPoliticsBindingSpec {
        path_pattern: "countrypoliticsview.parties_grid.political_party_info_entry[*].color_block",
        values: &[CountryPoliticsBindingValue::Tint],
        reason: "party ideology color swatch",
    },
    CountryPoliticsBindingSpec {
        path_pattern: "countrypoliticsview.idea_categories_grid",
        values: &[CountryPoliticsBindingValue::Instances],
        reason: "three vanilla idea-category rows",
    },
    CountryPoliticsBindingSpec {
        path_pattern: "countrypoliticsview.idea_categories_grid.country_politics_idea_category_entry[*].name",
        values: &[CountryPoliticsBindingValue::Text],
        reason: "idea-category row title",
    },
    CountryPoliticsBindingSpec {
        path_pattern: "countrypoliticsview.idea_categories_grid.country_politics_idea_category_entry[*].category_icon",
        values: &[CountryPoliticsBindingValue::Sprite],
        reason: "idea-category frame index on vanilla category icon sprite",
    },
    CountryPoliticsBindingSpec {
        path_pattern: "countrypoliticsview.idea_categories_grid.country_politics_idea_category_entry[*].ideas_grid",
        values: &[CountryPoliticsBindingValue::Instances],
        reason: "idea slot count for laws and advisor rows",
    },
    CountryPoliticsBindingSpec {
        path_pattern: "countrypoliticsview.idea_categories_grid.country_politics_idea_category_entry[*].political_idea_entry[*].idea_alert_glow",
        values: &[CountryPoliticsBindingValue::Visibility],
        reason: "hide unsupported alert glow over dynamic law/spirit slots",
    },
    CountryPoliticsBindingSpec {
        path_pattern: "countrypoliticsview.idea_categories_grid.country_politics_idea_category_entry[*].political_idea_entry[*].idea_traits",
        values: &[CountryPoliticsBindingValue::Visibility],
        reason: "hide unsupported trait strip over dynamic law/spirit slots",
    },
    CountryPoliticsBindingSpec {
        path_pattern: "countrypoliticsview.idea_categories_grid.country_politics_idea_category_entry[0].political_idea_entry[*].add_idea_button",
        values: &[
            CountryPoliticsBindingValue::Sprite,
            CountryPoliticsBindingValue::Tooltip,
            CountryPoliticsBindingValue::Command,
        ],
        reason: "law slot icon, tooltip and detail command",
    },
    CountryPoliticsBindingSpec {
        path_pattern: "countrypoliticsview.idea_categories_grid.country_politics_idea_category_entry[1..].political_idea_entry[*].add_idea_button",
        values: &[CountryPoliticsBindingValue::Tooltip],
        reason: "advisor rows are visible but game data is not connected yet",
    },
    CountryPoliticsBindingSpec {
        path_pattern: "countrypoliticsview.close_button",
        values: &[CountryPoliticsBindingValue::Tooltip, CountryPoliticsBindingValue::Command],
        reason: "vanilla close button requests animated panel close",
    },
    CountryPoliticsBindingSpec {
        path_pattern: "countrypoliticsview.election_button",
        values: &[CountryPoliticsBindingValue::Visibility, CountryPoliticsBindingValue::Command],
        reason: "SPR 1936 election button opens hemicycle panel",
    },
    CountryPoliticsBindingSpec {
        path_pattern: "countrypoliticsview.election_hemicycle",
        values: &[CountryPoliticsBindingValue::Visibility],
        reason: "SPR 1936 election hemicycle seats visualization",
    },
];

impl crate::vanilla_gui::VanillaPanelProfile for CountryPoliticsProfile {
    type Data = PoliticsData;
    type Command = DecisionCommand;

    fn profile_id(&self) -> &'static str {
        COUNTRY_POLITICS_PROFILE_ID
    }

    fn root_template(&self) -> &'static str {
        COUNTRY_POLITICS_ROOT
    }

    fn required_gui_files(&self) -> &'static [&'static str] {
        &[COUNTRY_POLITICS_GUI_FILE]
    }

    fn template_instances(&self) -> &'static [crate::vanilla_gui::VanillaTemplateInstance] {
        &[
            crate::vanilla_gui::VanillaTemplateInstance {
                template_name: COUNTRY_POLITICS_IDEA_CATEGORY_TEMPLATE,
                count: 3,
            },
            crate::vanilla_gui::VanillaTemplateInstance {
                template_name: "political_idea_entry",
                count: 6,
            },
            crate::vanilla_gui::VanillaTemplateInstance {
                template_name: COUNTRY_POLITICS_PARTY_TEMPLATE,
                count: 4,
            },
        ]
    }

    fn bind_node(
        &self,
        node_path: &crate::vanilla_gui::GuiNodePath,
        data: &Self::Data,
    ) -> crate::vanilla_gui::GuiBinding {
        bind_country_politics_dynamic_node(node_path, data)
    }

    fn handle_action(
        &self,
        action: crate::vanilla_gui::GuiAction,
        data: &Self::Data,
    ) -> Option<Self::Command> {
        if action.kind != crate::vanilla_gui::GuiActionKind::Click {
            return None;
        }
        let path = action.node_path.to_string();
        if path.contains("active_goal") {
            return Some(DecisionCommand::OpenFocusTree);
        }
        if path.contains("election_button") {
            return Some(DecisionCommand::ToggleElectionPanel);
        }
        data.law_slots.iter().find_map(|slot| {
            let key = law_panel::law_category_key(slot.category);
            path.contains(key).then(|| {
                DecisionCommand::Panel(PanelCommand::OpenDetail(ActiveDetailPanel::Law {
                    category: key.to_owned(),
                    law_id: None,
                }))
            })
        })
    }
}

fn bind_country_politics_dynamic_node(
    node_path: &crate::vanilla_gui::GuiNodePath,
    data: &PoliticsData,
) -> crate::vanilla_gui::GuiBinding {
    let path = node_path.to_string();
    let name = node_path.0.last().map(String::as_str).unwrap_or_default();
    if country_politics_hidden_node(&path, name) {
        return crate::vanilla_gui::GuiBinding::default().visible(false);
    }

    let category_kind = idea_category_kind_from_path(&path);
    let slot_context = idea_slot_context_from_path(&path);
    let spirit_slot = spirit_slot_index_from_path(&path);
    let party_row = party_row_index_from_path(&path);

    bind_country_politics_header_node(name, data)
        .or_else(|| bind_country_politics_focus_node(&path, name, data))
        .or_else(|| bind_country_politics_party_node(&path, name, party_row, data))
        .or_else(|| bind_country_politics_spirit_node(name, spirit_slot, data))
        .or_else(|| bind_country_politics_idea_category_node(&path, name, category_kind, data))
        .or_else(|| bind_country_politics_idea_slot_node(name, slot_context, spirit_slot, data))
        .or_else(|| bind_country_politics_command_node(name))
        .unwrap_or_default()
}

fn bind_country_politics_header_node(
    name: &str,
    data: &PoliticsData,
) -> Option<crate::vanilla_gui::GuiBinding> {
    match name {
        "political_title" => Some(crate::vanilla_gui::GuiBinding::default().text(tr("politics"))),
        "leader" => Some(
            crate::vanilla_gui::GuiBinding::default()
                .sprite(
                    data.leader_portrait_key
                        .as_deref()
                        .unwrap_or("GFX_leader_unknown"),
                )
                .tooltip(if data.leader_name.is_empty() {
                    tr("leader_unknown").to_owned()
                } else {
                    data.leader_name.clone()
                }),
        ),
        "leader_name" => Some(crate::vanilla_gui::GuiBinding::default().text(
            if data.leader_name.is_empty() {
                tr("leader_unknown").to_owned()
            } else {
                data.leader_name.clone()
            },
        )),
        "election_button" => {
            if data.election_hemicycle_seats.is_empty() {
                Some(crate::vanilla_gui::GuiBinding::default().visible(false))
            } else {
                Some(
                    crate::vanilla_gui::GuiBinding::default()
                        .visible(true)
                        .tooltip("查看1936选举议会")
                        .click("open_election_panel"),
                )
            }
        }
        "election_hemicycle" => {
            if !data.show_election_panel || data.election_hemicycle_seats.is_empty() {
                Some(crate::vanilla_gui::GuiBinding::default().visible(false))
            } else {
                let mut binding = crate::vanilla_gui::GuiBinding::default().visible(true);
                binding.hemicycle_seats = data.election_hemicycle_seats.clone();
                Some(binding)
            }
        }
        _ => None,
    }
}

fn bind_country_politics_focus_node(
    path: &str,
    name: &str,
    data: &PoliticsData,
) -> Option<crate::vanilla_gui::GuiBinding> {
    let has_focus = data.current_focus_name.is_some();
    match name {
        "active_goal" | "add_national_goal_button" => Some(
            crate::vanilla_gui::GuiBinding::default()
                .tooltip(
                    data.current_focus_name
                        .as_deref()
                        .unwrap_or("点击打开国策树"),
                )
                .click("open_focus_tree"),
        ),
        "title" if path.contains("active_goal") => Some(
            crate::vanilla_gui::GuiBinding::default()
                .text(data.current_focus_name.as_deref().unwrap_or("未选择国策")),
        ),
        "continuous_glow"
        | "continuous_small_glow"
        | "nat_spirit_glow_overlay"
        | "pol_power_icon"
        | "goal_icon"
        | "progress"
        | "progress_frame"
        | "focus_cost"
        | "drop_continuous_focus_button"
        | "drop_focus_button"
            if path.contains("active_goal") && !has_focus =>
        {
            Some(crate::vanilla_gui::GuiBinding::default().visible(false))
        }
        "focus_cost" => Some(
            crate::vanilla_gui::GuiBinding::default().text(
                data.current_focus_cost_days
                    .map(|days| format!("{days}天"))
                    .unwrap_or_default(),
            ),
        ),
        "progress" if path.contains("active_goal") => {
            Some(crate::vanilla_gui::GuiBinding::default().progress(focus_progress(data)))
        }
        _ => None,
    }
}

fn bind_country_politics_party_node(
    path: &str,
    name: &str,
    party_row: Option<usize>,
    data: &PoliticsData,
) -> Option<crate::vanilla_gui::GuiBinding> {
    match name {
        "parties_grid" => Some(
            crate::vanilla_gui::GuiBinding::default()
                .instances(normalized_party_popularity(data).len()),
        ),
        "leading_pol_party_bg" if party_row.is_some() => {
            Some(crate::vanilla_gui::GuiBinding::default().visible(false))
        }
        "color_block" if party_row.is_some() => {
            let idx = party_row.unwrap();
            let popularity = normalized_party_popularity(data);
            let color = popularity
                .get(idx)
                .map(|(key, _)| ideology_color(key))
                .unwrap_or_else(|| Color32::from_rgb(0x66, 0x66, 0x60));
            Some(crate::vanilla_gui::GuiBinding::default().tint(color))
        }
        "name" if party_row.is_some() => {
            let idx = party_row.unwrap();
            let popularity = normalized_party_popularity(data);
            Some(
                crate::vanilla_gui::GuiBinding::default()
                    .text(
                        popularity
                            .get(idx)
                            .map(|(key, popularity)| {
                                party_name_with_popularity(data, key, *popularity)
                            })
                            .unwrap_or_default(),
                    )
                    .text_color(vanilla_text()),
            )
        }
        "ideology" if path.contains("ruling_party_info") => {
            Some(crate::vanilla_gui::GuiBinding::default().text(ideology_label(&data.ruling_party)))
        }
        "elections" if path.contains("ruling_party_info") => {
            Some(crate::vanilla_gui::GuiBinding::default().text(tr("no_elections")))
        }
        "political_pie_chart" | "chart" => {
            let mut binding = crate::vanilla_gui::GuiBinding::default();
            binding.pie_segments = party_pie_segments(data);
            Some(binding)
        }
        _ => None,
    }
}

fn bind_country_politics_spirit_node(
    name: &str,
    spirit_slot: Option<usize>,
    data: &PoliticsData,
) -> Option<crate::vanilla_gui::GuiBinding> {
    match name {
        "national_spirit" => {
            Some(crate::vanilla_gui::GuiBinding::default().tooltip(tr("national_spirits")))
        }
        "spirit_grid" => Some(
            crate::vanilla_gui::GuiBinding::default().instances(national_spirit_ideas(data).len()),
        ),
        "add_idea_button" if spirit_slot.is_some() => {
            let idx = spirit_slot.unwrap();
            let spirits = national_spirit_ideas(data);
            let idea = spirits.get(idx).copied();
            Some(
                idea.and_then(idea_icon_gfx)
                    .map(|sprite| {
                        let tooltip = idea
                            .map(|idea| idea.name.clone())
                            .unwrap_or_else(|| tr("national_spirits").to_owned());
                        crate::vanilla_gui::GuiBinding::default()
                            .sprite(sprite)
                            .tooltip(tooltip)
                    })
                    .unwrap_or_else(|| {
                        crate::vanilla_gui::GuiBinding::default().tooltip(tr("national_spirits"))
                    }),
            )
        }
        _ => None,
    }
}

fn national_spirit_ideas(data: &PoliticsData) -> Vec<&IdeaEntry> {
    data.ideas
        .iter()
        .filter(|idea| is_national_spirit_category(&idea.category))
        .collect()
}

fn is_national_spirit_category(category: &str) -> bool {
    matches!(category, "country" | "national_spirit")
}

fn bind_country_politics_idea_category_node(
    path: &str,
    name: &str,
    category_kind: Option<IdeaCategoryRowKind>,
    data: &PoliticsData,
) -> Option<crate::vanilla_gui::GuiBinding> {
    match name {
        "idea_categories_grid" => Some(crate::vanilla_gui::GuiBinding::default().instances(3)),
        "name" if category_kind.is_some() => Some(
            crate::vanilla_gui::GuiBinding::default()
                .text(category_kind.unwrap().localized_title()),
        ),
        "category_icon" if category_kind.is_some() => Some(
            crate::vanilla_gui::GuiBinding::default()
                .sprite("GFX_idea_categories")
                .frame(category_kind.unwrap().category_frame()),
        ),
        "ideas_grid" if category_kind.is_some() => {
            Some(crate::vanilla_gui::GuiBinding::default().instances(
                politics_idea_category_slot_count(category_kind.unwrap(), data),
            ))
        }
        "ideas_grid" if path.contains("idea_categories_grid") => {
            Some(crate::vanilla_gui::GuiBinding::default().instances(data.law_slots.len()))
        }
        _ => None,
    }
}

fn bind_country_politics_idea_slot_node(
    name: &str,
    slot_context: Option<(IdeaCategoryRowKind, usize)>,
    spirit_slot: Option<usize>,
    data: &PoliticsData,
) -> Option<crate::vanilla_gui::GuiBinding> {
    match name {
        "idea_alert_glow" | "idea_traits" if slot_context.is_some() || spirit_slot.is_some() => {
            Some(crate::vanilla_gui::GuiBinding::default().visible(false))
        }
        "add_idea_button" if slot_context.is_some() => {
            let (kind, idx) = slot_context.unwrap();
            Some(match kind {
                IdeaCategoryRowKind::GovernmentLaws => data
                    .law_slots
                    .get(idx)
                    .map(|law| {
                        crate::vanilla_gui::GuiBinding::default()
                            .sprite(politics_law_idea_sprite(law))
                            .tooltip(politics_law_tooltip(law))
                            .click(format!("law:{}", law_panel::law_category_key(law.category)))
                    })
                    .unwrap_or_default(),
                IdeaCategoryRowKind::ResearchProduction | IdeaCategoryRowKind::MilitaryStaff => {
                    crate::vanilla_gui::GuiBinding::default().tooltip(format!(
                        "{}\n{}",
                        kind.localized_title(),
                        "顾问槽位尚未接入"
                    ))
                }
            })
        }
        _ => None,
    }
}

fn bind_country_politics_command_node(name: &str) -> Option<crate::vanilla_gui::GuiBinding> {
    match name {
        "close_button" => Some(
            crate::vanilla_gui::GuiBinding::default()
                .tooltip(tr("panel_close_hint"))
                .click("close"),
        ),
        _ => None,
    }
}

fn country_politics_hidden_node(path: &str, name: &str) -> bool {
    matches!(
        name,
        "autonomy_progress_view"
            | "subjects_button_container"
            | "show_subjects_button"
            | "show_exiles_collaborations_button"
            | "manage_occupied_button"
            | "power_balance_button"
            | "faction"
            | "no_faction"
            | "in_faction"
            | "original_in_faction"
            | "rules"
    ) || path.contains("power_balance")
        || path.contains("exile")
        || path.contains("occupied")
        || path.contains("subjects")
}

fn idea_category_kind_from_path(path: &str) -> Option<IdeaCategoryRowKind> {
    let marker = "country_politics_idea_category_entry[";
    let start = path.find(marker)? + marker.len();
    let end = path[start..].find(']')? + start;
    match path[start..end].parse::<usize>().ok()? {
        0 => Some(IdeaCategoryRowKind::GovernmentLaws),
        1 => Some(IdeaCategoryRowKind::ResearchProduction),
        2 => Some(IdeaCategoryRowKind::MilitaryStaff),
        _ => None,
    }
}

fn idea_slot_context_from_path(path: &str) -> Option<(IdeaCategoryRowKind, usize)> {
    let kind = idea_category_kind_from_path(path)?;
    let marker = "political_idea_entry[";
    let start = path.find(marker)? + marker.len();
    let end = path[start..].find(']')? + start;
    Some((kind, path[start..end].parse::<usize>().ok()?))
}

fn spirit_slot_index_from_path(path: &str) -> Option<usize> {
    if !path.contains("spirit_grid") {
        return None;
    }
    let marker = "political_idea_entry[";
    let start = path.find(marker)? + marker.len();
    let end = path[start..].find(']')? + start;
    path[start..end].parse::<usize>().ok()
}

fn party_row_index_from_path(path: &str) -> Option<usize> {
    if !path.contains("parties_grid") {
        return None;
    }
    let marker = "political_party_info_entry[";
    let start = path.find(marker)? + marker.len();
    let end = path[start..].find(']')? + start;
    path[start..end].parse::<usize>().ok()
}

fn ideology_color(key: &str) -> Color32 {
    hoi4_ideology_color(key)
}

fn hoi4_ideology_color(key: &str) -> Color32 {
    match canonical_ideology_key(key).unwrap_or(key) {
        "fascism" => Color32::from_rgb(150, 75, 0),
        "democratic" => Color32::from_rgb(0, 0, 255),
        "communism" => Color32::from_rgb(255, 0, 0),
        "neutrality" => Color32::from_rgb(124, 124, 124),
        _ => Color32::from_rgb(0x66, 0x66, 0x60),
    }
}

fn ideology_label(key: &str) -> &'static str {
    match canonical_ideology_key(key).unwrap_or(key) {
        "fascism" => tr("fascism"),
        "democratic" => tr("democratic"),
        "communism" => tr("communism"),
        "neutrality" => tr("neutrality"),
        _ => tr("unknown"),
    }
}

fn canonical_ideology_key(key: &str) -> Option<&'static str> {
    match key.trim().to_ascii_lowercase().replace('-', "_").as_str() {
        "fascism" | "fascist" => Some("fascism"),
        "democratic" | "democracy" | "democrat" => Some("democratic"),
        "communism" | "communist" => Some("communism"),
        "neutrality" | "neutral" | "non_aligned" | "nonaligned" => Some("neutrality"),
        _ => None,
    }
}

fn normalized_party_popularity(data: &PoliticsData) -> Vec<(String, f32)> {
    let mut totals = [
        ("fascism", 0.0_f32),
        ("democratic", 0.0_f32),
        ("communism", 0.0_f32),
        ("neutrality", 0.0_f32),
    ];
    for (key, value) in &data.party_popularity {
        let Some(canonical) = canonical_ideology_key(key) else {
            continue;
        };
        if let Some((_, total)) = totals
            .iter_mut()
            .find(|(ideology, _)| *ideology == canonical)
        {
            *total += value.max(0.0);
        }
    }
    let mut popularity = totals
        .into_iter()
        .filter_map(|(ideology, value)| {
            (value > 0.0).then(|| (ideology.to_owned(), value.clamp(0.0, 1.0)))
        })
        .collect::<Vec<_>>();
    if popularity.is_empty() && !data.party_popularity.is_empty() {
        if let Some(ruling) = canonical_ideology_key(&data.ruling_party) {
            popularity.push((ruling.to_owned(), 1.0));
        }
    }
    popularity
}

fn party_pie_segments(data: &PoliticsData) -> Vec<(f32, Color32)> {
    let mut segments = normalized_party_popularity(data)
        .iter()
        .map(|(key, value)| (value.max(0.0), ideology_color(key)))
        .filter(|(value, _)| *value > 0.0)
        .collect::<Vec<_>>();
    if !segments.is_empty() || data.party_popularity.is_empty() {
        return segments;
    }
    if let Some(ruling) = canonical_ideology_key(&data.ruling_party) {
        segments.push((1.0, ideology_color(ruling)));
    }
    segments
}

fn party_name_for_ideology<'a>(data: &'a PoliticsData, ideology: &str) -> Cow<'a, str> {
    if let Some((_, name)) = data
        .party_names
        .iter()
        .find(|(key, name)| key == ideology && !name.trim().is_empty())
    {
        return Cow::Borrowed(name.as_str());
    }
    if ideology == data.ruling_party && !data.party_full_name.trim().is_empty() {
        return Cow::Borrowed(data.party_full_name.as_str());
    }
    Cow::Borrowed(ideology_label(ideology))
}

fn party_name_with_popularity(data: &PoliticsData, ideology: &str, popularity: f32) -> String {
    let party_name = party_name_for_ideology(data, ideology);
    let percent = (popularity.clamp(0.0, 1.0) * 100.0).round() as i32;
    format!("{} {}%", party_name.as_ref(), percent)
}

/// F.2：单个决议在面板上的运行时状态（caller 在快照里填好）。
#[derive(Debug, Clone)]
pub struct DecisionEntry {
    pub id: String,
    pub name: String,
    pub description: String,
    pub icon: String,
    pub effect_preview: String,
    pub category: DecisionCategory,
    pub mechanic_kind: DecisionMechanicKind,
    pub cost_political_power: f32,
    /// `true` = 可见条件满足，决议出现在面板里。
    pub visible: bool,
    /// `true` = 可用条件满足且政治力量足够 + 不在冷却 + 不进行中 + 不一次性触发过。
    pub clickable: bool,
    /// `Some(remaining_days)` = 进行中（mission 决议）。
    pub mission_remaining: Option<u32>,
    /// `Some(total_days)` = 进行中的总长度（用于 progress bar）。
    pub mission_total: Option<u32>,
    /// `Some(remaining_days)` = 在冷却。
    pub cooldown_remaining: Option<u32>,
    /// `true` = fire_only_once 已触发过，永久禁用。
    pub already_fired: bool,
}

impl DecisionEntry {
    pub fn from_def(d: &Decision) -> Self {
        Self {
            id: d.id.clone(),
            name: d.name.clone(),
            description: d.description.clone(),
            icon: d.icon.clone(),
            effect_preview: decision_effect_preview(d),
            category: d.category,
            mechanic_kind: d.mechanic_kind,
            cost_political_power: d.cost_political_power,
            visible: true,
            clickable: true,
            mission_remaining: None,
            mission_total: None,
            cooldown_remaining: None,
            already_fired: false,
        }
    }
}

fn decision_effect_preview(d: &Decision) -> String {
    let mut summaries = d
        .on_complete
        .iter()
        .chain(d.on_activation.iter())
        .filter_map(decision_effect_summary)
        .collect::<Vec<_>>();
    if summaries.len() > 5 {
        summaries.truncate(5);
        summaries.push("...".to_owned());
    }
    summaries.join("  ·  ")
}

fn decision_effect_summary(effect: &Effect) -> Option<String> {
    match effect {
        Effect::ClampVariable { .. } | Effect::TriggerEvent(_) => None,
        _ => effect.effect_summary(),
    }
}

/// 政治面板每帧所需数据。
pub struct PoliticsData {
    pub ruling_party: String,
    pub party_popularity: Vec<(String, f32)>, // (ideology_key, 0..1)
    pub ideas: Vec<IdeaEntry>,
    /// 当前玩家 PP（用于面板顶部 hint 显示）。
    pub political_power: f32,
    pub stability: f32,
    pub war_support: f32,
    pub focus_available: bool,
    pub current_focus_name: Option<String>,
    pub current_focus_progress: f32,
    pub current_focus_cost_days: Option<u32>,

    // ─── J.1：国家元首 + 党派全称（顶部摘要区） ──────────────────
    /// 国家 tag（"GER" 等）。仅用于 IconBank 兜底。
    pub country_tag: String,
    /// 元首显示名（已按 `data.character_names` 解过 loc，例 "Adolf Hitler"）；
    /// 若元首不存在则为空串。
    pub leader_name: String,
    /// 元首肖像 GFX 名（例 `GFX_portrait_GER_adolf_hitler`）；`None` 时
    /// 由 PoliticsPanel 用国旗占位图兜底。
    pub leader_portrait_key: Option<String>,
    /// 党派完整名称（已解过 `<TAG>_<ideology>_party_long` loc，例 "Nationalsozialistische Deutsche Arbeiterpartei"）。
    /// 若本地化缺失则回退为 `ideology_label(ruling_party)`。
    pub party_full_name: String,
    pub party_names: Vec<(String, String)>,
    pub government_posts: Vec<GovernmentPostEntry>,
    pub law_slots: Vec<PoliticsLawEntry>,
    pub election_hemicycle_seats: Vec<crate::vanilla_gui::binding::HemicycleSeat>,
    pub show_election_panel: bool,
}

impl PoliticsData {
    /// 测试 / 无数据时的空构造（只填 4 项 C.2 字段，decisions 留空）。
    pub fn legacy(
        ruling_party: String,
        party_popularity: Vec<(String, f32)>,
        ideas: Vec<IdeaEntry>,
    ) -> Self {
        Self {
            ruling_party,
            party_popularity,
            ideas,
            political_power: 0.0,
            stability: 0.5,
            war_support: 0.0,
            focus_available: false,
            current_focus_name: None,
            current_focus_progress: 0.0,
            current_focus_cost_days: None,
            country_tag: String::new(),
            leader_name: String::new(),
            leader_portrait_key: None,
            party_full_name: String::new(),
            party_names: Vec::new(),
            government_posts: Vec::new(),
            law_slots: Vec::new(),
            election_hemicycle_seats: Vec::new(),
            show_election_panel: false,
        }
    }
}

/// 政治面板中的一条国家精神。
#[derive(Debug, Clone)]
pub struct IdeaEntry {
    pub key: String,
    pub name: String,
    pub category: String,
    pub picture: Option<String>,
    pub modifiers: Vec<(String, f32)>,
}

#[derive(Debug, Clone)]
pub struct GovernmentPostEntry {
    pub office: String,
    pub name: String,
    pub detail: String,
}

#[derive(Debug, Clone)]
pub struct PoliticsLawEntry {
    pub category: LawCategory,
    pub current_id: String,
    pub current_name: String,
    pub cooldown_days: u16,
    pub pending: Option<(String, u16)>,
    pub is_locked: bool,
}

/// F.2：面板回写命令。
#[derive(Debug, Clone, PartialEq)]
pub enum DecisionCommand {
    /// 玩家点了某个决议的执行按钮。
    Activate(String),
    OpenFocusTree,
    ToggleElectionPanel,
    Panel(PanelCommand),
}

/// 政治面板（无状态）。
///
/// 返回 `(close, commands)`：
/// - `close = true` 用户点关闭按钮
/// - `commands` 用户本帧点击的所有决议命令
pub struct PoliticsPanel;

impl PoliticsPanel {
    pub fn show(
        ctx: &egui::Context,
        data: &PoliticsData,
        icon_bank: &mut crate::icons::IconBank,
    ) -> (bool, Vec<DecisionCommand>) {
        vanilla_show_politics(ctx, data, icon_bank)
    }
}

fn vanilla_show_politics(
    ctx: &egui::Context,
    data: &PoliticsData,
    icon_bank: &mut crate::icons::IconBank,
) -> (bool, Vec<DecisionCommand>) {
    use crate::v9::paint;
    use crate::vanilla_gui::VanillaPanelProfile;

    let profile = CountryPoliticsProfile;
    icon_bank.add_politics_search_dirs();
    if !data.country_tag.is_empty() {
        icon_bank.add_leader_dirs([data.country_tag.as_str()]);
    }
    let diag_id = egui::Id::new("politics_panel_sprite_diag_logged");
    let already_logged = ctx
        .data_mut(|d| d.get_persisted::<bool>(diag_id))
        .unwrap_or(false);
    if !already_logged {
        let (loaded, missing) = icon_bank.diagnose_sprites(POLITICS_PANEL_SPRITES.iter().copied());
        if let Some(context) = politics_vanilla_gui_context() {
            let profile_report =
                context.profile_report(&crate::vanilla_gui::COUNTRY_POLITICS_DESCRIPTOR);
            println!(
                "[ui][politics] gui_loaded=true gfx_tokens={}/{} sprites_loaded={} sprites_missing={}",
                profile_report.gfx_hits.hits, profile_report.gfx_hits.requested, loaded, missing
            );
            if !profile_report.gfx_hits.missing.is_empty() {
                println!(
                    "[ui][politics] missing_gfx_tokens={:?}",
                    profile_report.gfx_hits.missing
                );
            }
            for sprite in POLITICS_PANEL_SPRITES {
                if icon_bank.get_or_load(sprite).is_some() {
                    continue;
                }
                let report = icon_bank.diagnose_sprite(sprite);
                let gfx_source = context
                    .gfx_index
                    .get(sprite)
                    .and_then(|resource| resource.source.as_ref())
                    .map(|path| path.display().to_string())
                    .unwrap_or_else(|| "<unmapped>".to_owned());
                println!(
                    "[ui][politics] missing_sprite sprite={} gfx_source={} textureFile={:?} tried={:?} reason={:?}",
                    report.sprite_name,
                    gfx_source,
                    report.texture_file,
                    report.attempted_paths,
                    report.failure_reason
                );
            }
        } else {
            println!(
                "[ui][politics] gui_loaded=false sprites_loaded={loaded} sprites_missing={missing}"
            );
        }
        ctx.data_mut(|d| d.insert_persisted(diag_id, true));
    }
    let screen = ctx.screen_rect();
    let viewport = crate::vanilla_gui::GuiRect::new(
        screen.left(),
        screen.top(),
        screen.width(),
        screen.height(),
    );
    let vanilla_context = politics_vanilla_gui_context();
    let vanilla_root = vanilla_context.and_then(|context| {
        context.root_template(COUNTRY_POLITICS_GUI_FILE, profile.root_template())
    });
    let (Some(context), Some(root)) = (vanilla_context, vanilla_root) else {
        return vanilla_politics_runtime_unavailable_panel(ctx);
    };
    let root_layout = crate::vanilla_gui::compute_layout_tree(
        root,
        &crate::vanilla_gui::LayoutOptions::new(viewport).shown_position(true),
    );
    let spec = crate::vanilla_gui::AnimationSpec::from_node(root);
    let update = crate::vanilla_gui::update_panel_animation(ctx, profile.profile_id(), spec);

    if update.close_finished {
        crate::vanilla_gui::clear_panel_close_request(ctx, profile.profile_id());
        return (true, Vec::new());
    }
    if !update.visible {
        return (false, Vec::new());
    }

    let offset = crate::vanilla_gui::GuiPoint {
        x: update.position.x - spec.shown_position.x,
        y: update.position.y - spec.shown_position.y,
    };
    let root_layout = if offset.x.abs() > f32::EPSILON || offset.y.abs() > f32::EPSILON {
        offset_politics_layout_tree(&root_layout, offset)
    } else {
        root_layout
    };

    let mut close = false;
    let mut output = Vec::new();
    egui::Area::new(egui::Id::new("politics_panel_vanilla_1936"))
        .order(egui::Order::Foreground)
        .fixed_pos(screen.min)
        .show(ctx, |ui| {
            let outer: Rect = root_layout.rect.into();
            ui.interact(
                outer.intersect(screen),
                ui.id().with("politics_panel_drag_region"),
                Sense::click_and_drag(),
            );
            paint::paint_shadow(ui.painter(), outer, crate::v9::Elevation::E2, 1.0);
            let bindings = crate::vanilla_gui::bind_profile_tree(&profile, root, data);
            let renderer = crate::vanilla_gui::VanillaGuiRenderer::new(&context.gfx_index);
            let mut stats = renderer.paint_tree(ui, root, &root_layout, &bindings, icon_bank);
            if politics_close_requested_from_render_stats(&stats) {
                close = true;
            }
            output.extend(politics_commands_from_render_stats(&profile, data, &stats));
            let bridge_stats = paint_politics_template_instance_bridge(
                ui,
                &renderer,
                context,
                &profile,
                data,
                root,
                &root_layout,
                icon_bank,
                &mut output,
            );
            if politics_close_requested_from_render_stats(&bridge_stats) {
                close = true;
            }
            stats.merge(bridge_stats);
            log_politics_render_stats(ctx, &stats, icon_bank);
        });

    if close {
        request_country_politics_close(ctx);
    }

    (false, output)
}

fn offset_politics_layout_tree(
    layout: &crate::vanilla_gui::LayoutNode,
    offset: crate::vanilla_gui::GuiPoint,
) -> crate::vanilla_gui::LayoutNode {
    let mut next = layout.clone();
    offset_politics_layout_tree_in_place(&mut next, offset);
    next
}

fn offset_politics_layout_tree_in_place(
    layout: &mut crate::vanilla_gui::LayoutNode,
    offset: crate::vanilla_gui::GuiPoint,
) {
    layout.rect.x += offset.x;
    layout.rect.y += offset.y;
    layout.clip_rect.x += offset.x;
    layout.clip_rect.y += offset.y;
    layout.rects.translate(offset);
    for child in &mut layout.children {
        offset_politics_layout_tree_in_place(child, offset);
    }
}

fn vanilla_politics_runtime_unavailable_panel(ctx: &egui::Context) -> (bool, Vec<DecisionCommand>) {
    let screen = ctx.screen_rect();
    let panel = Rect::from_min_size(
        Pos2::new(screen.left() + 16.0, screen.top() + 92.0),
        Vec2::new(620.0_f32.min((screen.width() - 32.0).max(280.0)), 232.0),
    );
    let diagnostics = crate::vanilla_gui::vanilla_profile_diagnostics_markdown(
        None,
        &crate::vanilla_gui::COUNTRY_POLITICS_DESCRIPTOR,
    );
    let reason = diagnostics
        .lines()
        .find(|line| line.starts_with("- ["))
        .unwrap_or("- [hoi4_path] vanilla runtime unavailable")
        .trim_start_matches("- ")
        .to_owned();
    let mut close = false;
    egui::Area::new(egui::Id::new("countrypoliticsview_runtime_unavailable"))
        .order(egui::Order::Foreground)
        .fixed_pos(panel.min)
        .show(ctx, |ui| {
            let local = Rect::from_min_size(Pos2::ZERO, panel.size());
            let response = ui.allocate_rect(local, Sense::click_and_drag());
            let painter = ui.painter();
            painter.rect_filled(local, 1.0, Color32::from_rgb(0x0a, 0x0d, 0x0c));
            painter.rect_stroke(
                local,
                1.0,
                egui::Stroke::new(1.0, Color32::from_rgb(0x6e, 0x5a, 0x35)),
                egui::StrokeKind::Inside,
            );
            painter.text(
                Pos2::new(local.left() + 16.0, local.top() + 16.0),
                egui::Align2::LEFT_TOP,
                "Vanilla politics runtime unavailable",
                crate::v9::TextRole::Heading.font_id(),
                VanillaIron::TEXT,
            );
            painter.text(
                Pos2::new(local.left() + 16.0, local.top() + 54.0),
                egui::Align2::LEFT_TOP,
                reason,
                crate::v9::TextRole::Body.font_id(),
                VanillaIron::MUTED,
            );
            painter.text(
                Pos2::new(local.left() + 16.0, local.top() + 92.0),
                egui::Align2::LEFT_TOP,
                format!("required: {}", COUNTRY_POLITICS_GUI_FILE),
                crate::v9::TextRole::Caption.font_id(),
                VanillaIron::MUTED,
            );
            painter.text(
                Pos2::new(local.left() + 16.0, local.top() + 124.0),
                egui::Align2::LEFT_TOP,
                "No V9 or legacy UI fallback is used for this panel.",
                crate::v9::TextRole::Caption.font_id(),
                VanillaIron::MUTED,
            );
            let close_rect = Rect::from_min_size(
                Pos2::new(local.right() - 38.0, local.top() + 10.0),
                Vec2::splat(26.0),
            );
            if ui
                .put(close_rect, egui::Button::new("X"))
                .on_hover_text(tr("panel_close_hint"))
                .clicked()
            {
                close = true;
            }
            response.on_hover_cursor(egui::CursorIcon::Grab);
        });
    (close, Vec::new())
}

fn log_politics_render_stats(
    ctx: &egui::Context,
    stats: &crate::vanilla_gui::RenderStats,
    icon_bank: &crate::icons::IconBank,
) {
    let id = egui::Id::new("politics_panel_render_stats_logged");
    let already_logged = ctx
        .data_mut(|d| d.get_persisted::<bool>(id))
        .unwrap_or(false);
    if already_logged {
        return;
    }
    println!(
        "[ui][politics] render nodes={}/{} sprites={} fallback={} text={} buttons={} progress={} pie={} icon_missing_cache={} fallback_labels={:?}",
        stats.nodes_painted,
        stats.nodes_seen,
        stats.sprites_painted,
        stats.fallback_painted,
        stats.text_painted,
        stats.buttons,
        stats.progress_bars,
        stats.pie_charts,
        icon_bank.missing_count(),
        stats.fallback_labels
    );
    ctx.data_mut(|d| d.insert_persisted(id, true));
}

fn politics_close_requested_from_render_stats(stats: &crate::vanilla_gui::RenderStats) -> bool {
    stats
        .clicked_commands
        .iter()
        .any(|command| command == "close")
}

fn politics_commands_from_render_stats(
    profile: &CountryPoliticsProfile,
    data: &PoliticsData,
    stats: &crate::vanilla_gui::RenderStats,
) -> Vec<DecisionCommand> {
    stats
        .clicked_commands
        .iter()
        .filter_map(|command| match command.as_str() {
            "open_focus_tree" => profile.handle_action(
                crate::vanilla_gui::GuiAction {
                    node_path: crate::vanilla_gui::GuiNodePath::root(COUNTRY_POLITICS_ROOT)
                        .child("active_goal"),
                    kind: crate::vanilla_gui::GuiActionKind::Click,
                },
                data,
            ),
            key if key.starts_with("law:") => key.strip_prefix("law:").map(|category| {
                DecisionCommand::Panel(PanelCommand::OpenDetail(ActiveDetailPanel::Law {
                    category: category.to_owned(),
                    law_id: None,
                }))
            }),
            "close" => None,
            _ => None,
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn paint_politics_template_instance_bridge(
    ui: &mut egui::Ui,
    renderer: &crate::vanilla_gui::VanillaGuiRenderer<'_>,
    context: &VanillaPoliticsGuiContext,
    profile: &CountryPoliticsProfile,
    data: &PoliticsData,
    root: &crate::vanilla_gui::GuiNode,
    root_layout: &crate::vanilla_gui::LayoutNode,
    icon_bank: &mut crate::icons::IconBank,
    cmds: &mut Vec<DecisionCommand>,
) -> crate::vanilla_gui::RenderStats {
    // Bridge only: the generic runtime still does not expand vanilla grid/template
    // instances by itself. These calls place real vanilla templates at vanilla
    // grid slots, then let CountryPoliticsProfile provide text/sprite/progress/
    // visibility/command bindings. Do not add hand-painted panel chrome here.
    let mut stats = crate::vanilla_gui::RenderStats::default();
    stats.merge(paint_politics_party_template_instances(
        ui,
        renderer,
        context,
        profile,
        data,
        root,
        root_layout,
        icon_bank,
        cmds,
    ));
    stats.merge(paint_politics_national_spirit_template_instances(
        ui,
        renderer,
        context,
        profile,
        data,
        root_layout,
        icon_bank,
        cmds,
    ));
    stats.merge(paint_politics_idea_category_template_instances(
        ui,
        renderer,
        context,
        profile,
        data,
        root,
        root_layout,
        icon_bank,
        cmds,
    ));
    stats
}

#[allow(clippy::too_many_arguments)]
fn paint_politics_party_template_instances(
    ui: &mut egui::Ui,
    renderer: &crate::vanilla_gui::VanillaGuiRenderer<'_>,
    context: &VanillaPoliticsGuiContext,
    profile: &CountryPoliticsProfile,
    data: &PoliticsData,
    root: &crate::vanilla_gui::GuiNode,
    root_layout: &crate::vanilla_gui::LayoutNode,
    icon_bank: &mut crate::icons::IconBank,
    cmds: &mut Vec<DecisionCommand>,
) -> crate::vanilla_gui::RenderStats {
    let Some(grid_node) = root.find_node_by_name("parties_grid") else {
        return crate::vanilla_gui::RenderStats::default();
    };
    let Some(grid_layout) = root_layout.find_by_name("parties_grid") else {
        return crate::vanilla_gui::RenderStats::default();
    };
    let Some(row_template) =
        context.root_template(COUNTRY_POLITICS_GUI_FILE, COUNTRY_POLITICS_PARTY_TEMPLATE)
    else {
        return crate::vanilla_gui::RenderStats::default();
    };

    let count = normalized_party_popularity(data).len().clamp(1, 4);
    let instancer =
        crate::vanilla_gui::TemplateInstancer::new(COUNTRY_POLITICS_PARTY_TEMPLATE, row_template);
    let total_stats = instancer.paint_profile_grid_instances_with_options(
        ui,
        renderer,
        profile,
        data,
        grid_node,
        grid_layout,
        count,
        crate::vanilla_gui::TemplateInstanceOptions::default().template_size(true),
        icon_bank,
    );
    cmds.extend(politics_commands_from_render_stats(
        profile,
        data,
        &total_stats,
    ));
    total_stats
}

fn snap_politics_rect(ui: &egui::Ui, rect: Rect) -> Rect {
    let pixels_per_point = ui.ctx().pixels_per_point().max(1.0);
    Rect::from_min_max(
        Pos2::new(
            (rect.min.x * pixels_per_point).round() / pixels_per_point,
            (rect.min.y * pixels_per_point).round() / pixels_per_point,
        ),
        Pos2::new(
            (rect.max.x * pixels_per_point).round() / pixels_per_point,
            (rect.max.y * pixels_per_point).round() / pixels_per_point,
        ),
    )
}

#[allow(clippy::too_many_arguments)]
fn paint_politics_national_spirit_template_instances(
    ui: &mut egui::Ui,
    renderer: &crate::vanilla_gui::VanillaGuiRenderer<'_>,
    context: &VanillaPoliticsGuiContext,
    profile: &CountryPoliticsProfile,
    data: &PoliticsData,
    root_layout: &crate::vanilla_gui::LayoutNode,
    icon_bank: &mut crate::icons::IconBank,
    cmds: &mut Vec<DecisionCommand>,
) -> crate::vanilla_gui::RenderStats {
    let Some(grid_layout) = root_layout.find_by_name("spirit_grid") else {
        return crate::vanilla_gui::RenderStats::default();
    };
    let Some(slot_template) =
        context.root_template(COUNTRY_POLITICS_GUI_FILE, "political_idea_entry")
    else {
        return crate::vanilla_gui::RenderStats::default();
    };
    let count = national_spirit_ideas(data).len().clamp(1, 6);
    let mut total_stats = crate::vanilla_gui::RenderStats::default();
    for idx in 0..count {
        let slot = crate::vanilla_gui::GuiRect::new(
            grid_layout.rect.x + idx as f32 * 59.0,
            grid_layout.rect.y,
            59.0,
            68.0,
        );
        let path = grid_layout
            .path
            .clone()
            .child(format!("political_idea_entry[{idx}]"));
        let slot_layout = crate::vanilla_gui::compute_layout_tree_with_path(
            slot_template,
            &crate::vanilla_gui::LayoutOptions::new(slot),
            path.clone(),
        );
        let bindings =
            crate::vanilla_gui::bind_profile_tree_with_path(profile, slot_template, data, path);
        let stats = renderer.paint_tree(ui, slot_template, &slot_layout, &bindings, icon_bank);
        cmds.extend(politics_commands_from_render_stats(profile, data, &stats));
        total_stats.merge(stats);
    }
    total_stats
}

#[allow(clippy::too_many_arguments)]
fn paint_politics_idea_category_template_instances(
    ui: &mut egui::Ui,
    renderer: &crate::vanilla_gui::VanillaGuiRenderer<'_>,
    context: &VanillaPoliticsGuiContext,
    profile: &CountryPoliticsProfile,
    data: &PoliticsData,
    root: &crate::vanilla_gui::GuiNode,
    root_layout: &crate::vanilla_gui::LayoutNode,
    icon_bank: &mut crate::icons::IconBank,
    cmds: &mut Vec<DecisionCommand>,
) -> crate::vanilla_gui::RenderStats {
    let Some(grid_node) = root.find_node_by_name("idea_categories_grid") else {
        return crate::vanilla_gui::RenderStats::default();
    };
    let Some(grid_layout) = root_layout.find_by_name("idea_categories_grid") else {
        return crate::vanilla_gui::RenderStats::default();
    };
    let Some(template) = context.root_template(
        COUNTRY_POLITICS_GUI_FILE,
        COUNTRY_POLITICS_IDEA_CATEGORY_TEMPLATE,
    ) else {
        return crate::vanilla_gui::RenderStats::default();
    };

    let rows = [
        IdeaCategoryRowKind::GovernmentLaws,
        IdeaCategoryRowKind::ResearchProduction,
        IdeaCategoryRowKind::MilitaryStaff,
    ];
    let mut total_stats = crate::vanilla_gui::RenderStats::default();
    for (idx, slot) in crate::vanilla_gui::grid_slots(grid_node, grid_layout.rect, rows.len())
        .into_iter()
        .enumerate()
    {
        let Some(kind) = rows.get(idx).copied() else {
            continue;
        };
        let path = crate::vanilla_gui::GuiNodePath::root(COUNTRY_POLITICS_ROOT)
            .child("idea_categories_grid")
            .child(format!("{COUNTRY_POLITICS_IDEA_CATEGORY_TEMPLATE}[{idx}]"));
        let row_slot = crate::vanilla_gui::GuiRect::new(
            slot.x,
            slot.y,
            vanilla_idea_category_row_width(),
            vanilla_idea_category_row_height(),
        );
        let row_layout = crate::vanilla_gui::compute_layout_tree_with_path(
            template,
            &crate::vanilla_gui::LayoutOptions::new(row_slot),
            path.clone(),
        );
        let row_bindings =
            crate::vanilla_gui::bind_profile_tree_with_path(profile, template, data, path);
        let stats = renderer.paint_tree(ui, template, &row_layout, &row_bindings, icon_bank);
        cmds.extend(politics_commands_from_render_stats(profile, data, &stats));
        total_stats.merge(stats);
        total_stats.merge(paint_politics_idea_category_slot_templates(
            ui,
            renderer,
            context,
            profile,
            data,
            template,
            &row_layout,
            kind,
            icon_bank,
            cmds,
        ));
    }
    total_stats
}

#[allow(clippy::too_many_arguments)]
fn paint_politics_idea_category_slot_templates(
    ui: &mut egui::Ui,
    renderer: &crate::vanilla_gui::VanillaGuiRenderer<'_>,
    context: &VanillaPoliticsGuiContext,
    profile: &CountryPoliticsProfile,
    data: &PoliticsData,
    row_template: &crate::vanilla_gui::GuiNode,
    row_layout: &crate::vanilla_gui::LayoutNode,
    kind: IdeaCategoryRowKind,
    icon_bank: &mut crate::icons::IconBank,
    cmds: &mut Vec<DecisionCommand>,
) -> crate::vanilla_gui::RenderStats {
    let Some(grid_node) = row_template.find_node_by_name("ideas_grid") else {
        return crate::vanilla_gui::RenderStats::default();
    };
    let Some(grid_layout) = row_layout.find_by_name("ideas_grid") else {
        return crate::vanilla_gui::RenderStats::default();
    };
    let Some(slot_template) =
        context.root_template(COUNTRY_POLITICS_GUI_FILE, "political_idea_entry")
    else {
        return crate::vanilla_gui::RenderStats::default();
    };
    let count = politics_idea_category_slot_count(kind, data);
    let mut total_stats = crate::vanilla_gui::RenderStats::default();
    for (idx, slot) in crate::vanilla_gui::grid_slots(grid_node, grid_layout.rect, count)
        .into_iter()
        .enumerate()
    {
        let path = row_layout
            .path
            .clone()
            .child("ideas_grid")
            .child(format!("political_idea_entry[{idx}]"));
        let slot_layout = crate::vanilla_gui::compute_layout_tree_with_path(
            slot_template,
            &crate::vanilla_gui::LayoutOptions::new(slot),
            path.clone(),
        );
        let bindings =
            crate::vanilla_gui::bind_profile_tree_with_path(profile, slot_template, data, path);
        let stats = renderer.paint_tree(ui, slot_template, &slot_layout, &bindings, icon_bank);
        cmds.extend(politics_commands_from_render_stats(profile, data, &stats));
        total_stats.merge(stats);
    }
    total_stats
}

fn politics_idea_category_slot_count(kind: IdeaCategoryRowKind, data: &PoliticsData) -> usize {
    match kind {
        IdeaCategoryRowKind::GovernmentLaws => data.law_slots.len().clamp(1, 7),
        IdeaCategoryRowKind::ResearchProduction => 5,
        IdeaCategoryRowKind::MilitaryStaff => 6,
    }
}

fn vanilla_idea_category_row_width() -> f32 {
    vanilla_idea_category_template()
        .and_then(|template| template.block("size"))
        .and_then(|size| size.get("width"))
        .and_then(crate::vanilla_gui::GuiValueExt::as_lossy_f32)
        .unwrap_or(550.0)
}

pub fn request_country_politics_close(ctx: &egui::Context) {
    crate::vanilla_gui::request_panel_close(ctx, COUNTRY_POLITICS_PROFILE_ID);
}

fn vanilla_politics_body(
    ui: &mut egui::Ui,
    outer: Rect,
    _body_clip: Rect,
    data: &PoliticsData,
    icon_bank: &mut crate::icons::IconBank,
    cmds: &mut Vec<DecisionCommand>,
    root_layout: Option<&crate::vanilla_gui::LayoutNode>,
) {
    let leader_rect = vanilla_layout_rect(
        root_layout,
        outer,
        "leader",
        vanilla_panel_rect(outer, 18.0, 58.0, 136.0, 196.0),
    );
    draw_vanilla_portrait(ui, leader_rect, data, icon_bank);
    vanilla_leader_nameplate(ui, leader_rect, data);

    let focus_rect = vanilla_layout_rect(
        root_layout,
        outer,
        "active_goal",
        vanilla_panel_rect(outer, 179.0, 48.0, 330.0, 82.0),
    );
    let focus_rect = Rect::from_min_size(
        focus_rect.min,
        Vec2::new(focus_rect.width().max(330.0), focus_rect.height().max(82.0)),
    );
    vanilla_focus_selector_at(ui, focus_rect, data, cmds);

    let spirit_title = vanilla_panel_rect(outer, 191.0, 151.0, 240.0, 20.0);
    ui.painter().text(
        spirit_title.left_top(),
        egui::Align2::LEFT_TOP,
        tr("national_spirits"),
        crate::v9::TextRole::Caption.font_id(),
        vanilla_muted(),
    );
    let spirit_grid = vanilla_layout_rect(
        root_layout,
        outer,
        "spirit_grid",
        vanilla_panel_rect(outer, 181.0, 171.0, 354.0, 68.0),
    );
    vanilla_spirit_grid_at(ui, spirit_grid, data, icon_bank);

    let party_rect = vanilla_layout_rect(
        root_layout,
        outer,
        "ruling_party_info",
        vanilla_panel_rect(outer, 88.0, 305.0, 421.0, 104.0),
    );
    let party_rect = Rect::from_min_size(
        party_rect.min,
        Vec2::new((outer.right() - party_rect.left() - 14.0).max(260.0), 104.0),
    );
    vanilla_ideology_summary_at(ui, party_rect, data);

    let ideas = vanilla_layout_rect(
        root_layout,
        outer,
        "ideas",
        vanilla_panel_rect(outer, 0.0, 425.0, outer.width(), outer.height() - 425.0),
    );
    let ideas_clip = Rect::from_min_max(
        Pos2::new(outer.left(), ideas.top()),
        Pos2::new(outer.right(), outer.bottom()),
    )
    .intersect(outer);
    if ideas_clip.is_positive() {
        ui.painter()
            .rect_filled(ideas_clip, 0.0, Color32::from_black_alpha(150));
        paint_vanilla_border(ui.painter(), ideas_clip);

        let grid = vanilla_layout_rect(
            root_layout,
            outer,
            "idea_categories_grid",
            vanilla_panel_rect(
                outer,
                17.0,
                425.0,
                (outer.width() - 34.0).max(320.0),
                vanilla_idea_categories_height(),
            ),
        );
        let categories = Rect::from_min_size(
            Pos2::new(grid.left(), grid.top()),
            Vec2::new(
                (outer.right() - grid.left() - 12.0).max(320.0),
                vanilla_idea_categories_height(),
            ),
        );
        ui.allocate_ui_at_rect(ideas_clip, |ui| {
            ui.set_clip_rect(ideas_clip);
            vanilla_idea_categories_at(ui, data, icon_bank, cmds, categories);
        });
    }
}

fn vanilla_panel_rect(outer: Rect, x: f32, y: f32, width: f32, height: f32) -> Rect {
    Rect::from_min_size(
        Pos2::new(outer.left() + x, outer.top() + y),
        Vec2::new(width, height),
    )
}

fn vanilla_layout_rect(
    root_layout: Option<&crate::vanilla_gui::LayoutNode>,
    outer: Rect,
    name: &str,
    fallback: Rect,
) -> Rect {
    let Some(root) = root_layout else {
        return fallback;
    };
    let Some(node) = root.find_by_name(name) else {
        return fallback;
    };
    let width = if node.rect.width >= 1.0 {
        node.rect.width
    } else {
        fallback.width()
    };
    let height = if node.rect.height >= 1.0 {
        node.rect.height
    } else {
        fallback.height()
    };
    Rect::from_min_size(
        Pos2::new(
            outer.left() + node.rect.x - root.rect.x,
            outer.top() + node.rect.y - root.rect.y,
        ),
        Vec2::new(width, height),
    )
}

fn vanilla_spirit_grid_at(
    ui: &mut egui::Ui,
    rect: Rect,
    data: &PoliticsData,
    icon_bank: &mut crate::icons::IconBank,
) {
    vanilla_row_frame(ui, rect);
    let slot_w = (rect.width() / 6.0).clamp(42.0, 59.0);
    let slot_h = rect.height().clamp(52.0, 68.0);
    let spirits = national_spirit_ideas(data);
    if spirits.is_empty() {
        let slot = Rect::from_min_size(rect.left_top(), Vec2::new(slot_w, slot_h));
        vanilla_slot(ui, slot, Color32::from_rgb(0x34, 0x35, 0x30));
        ui.painter().text(
            slot.center(),
            egui::Align2::CENTER_CENTER,
            "?",
            crate::v9::TextRole::Subheading.font_id(),
            vanilla_muted(),
        );
        return;
    }

    for (idx, idea) in spirits.into_iter().take(6).enumerate() {
        let slot = Rect::from_min_size(
            Pos2::new(rect.left() + idx as f32 * slot_w, rect.top()),
            Vec2::new(slot_w, slot_h),
        );
        vanilla_slot(ui, slot, vanilla_edge());
        if let Some(handle) = idea_icon_gfx(idea).and_then(|gfx| icon_bank.get_or_load(&gfx)) {
            ui.put(
                slot.shrink(5.0),
                egui::Image::from_texture(handle).fit_to_exact_size(slot.shrink(5.0).size()),
            );
        } else {
            let letter = idea
                .name
                .chars()
                .find(|c| !c.is_whitespace())
                .unwrap_or('?');
            ui.painter().text(
                slot.center(),
                egui::Align2::CENTER_CENTER,
                letter.to_string(),
                crate::v9::TextRole::Subheading.font_id(),
                vanilla_gold(),
            );
        }
        ui.interact(
            slot,
            ui.id().with(("vanilla_spirit_grid", &idea.key)),
            Sense::hover(),
        )
        .on_hover_ui(|ui| render_idea_tooltip(ui, idea));
    }
}

#[deprecated(note = "Debug fallback only; PoliticsPanel::show defaults to vanilla_show_politics.")]
fn v9_show_politics(
    ctx: &egui::Context,
    data: &PoliticsData,
    icon_bank: &mut crate::icons::IconBank,
) -> (bool, Vec<DecisionCommand>) {
    use crate::v9::{
        composites::side_rail::{SIDE_RAIL_PANEL_LEFT, SIDE_RAIL_TOP_OFFSET},
        paint,
        primitives::{Button, ButtonSize, ButtonVariant},
        tokens::{palette, spacing, Elevation, TextRole},
    };

    let accent = v9_ideology_color(&data.ruling_party);

    let screen = ctx.screen_rect();
    let left_gap = if screen.width() >= 980.0 {
        SIDE_RAIL_PANEL_LEFT
    } else {
        8.0
    };
    let top_gap = if screen.height() >= 680.0 {
        SIDE_RAIL_TOP_OFFSET
    } else {
        72.0
    };
    let panel_w = 820.0_f32.min((screen.width() - left_gap - 8.0).max(420.0));
    let available_h = (screen.height() - top_gap - 8.0).max(360.0);
    let panel_h = available_h.min(820.0);
    let panel_pos = Pos2::new(screen.left() + left_gap, screen.top() + top_gap);
    let panel_size = Vec2::new(panel_w, panel_h);

    let mut close = false;
    let mut output = Vec::new();
    egui::Area::new(egui::Id::new("politics_panel_hoi4_rebuild"))
        .order(egui::Order::Foreground)
        .fixed_pos(panel_pos)
        .show(ctx, |ui| {
            let (outer, _) = ui.allocate_exact_size(panel_size, Sense::click_and_drag());
            paint::paint_shadow(ui.painter(), outer, Elevation::E3, 2.0);
            paint_politics_shell(ui, outer, accent);

            let inner = outer.shrink2(Vec2::new(spacing::S5, spacing::S4));
            let header = Rect::from_min_size(inner.min, Vec2::new(inner.width(), 66.0));
            let tab = Rect::from_min_size(
                Pos2::new(inner.left(), header.bottom() + spacing::S4),
                Vec2::new(inner.width(), 36.0),
            );
            let body = Rect::from_min_max(
                Pos2::new(inner.left(), tab.bottom() + spacing::S4),
                Pos2::new(inner.right(), inner.bottom() - spacing::S2),
            );

            paint::paint_recessed_panel(ui.painter(), header, 1.0);
            ui.painter().rect_filled(
                Rect::from_min_max(
                    header.left_top(),
                    Pos2::new(header.left() + 4.0, header.bottom()),
                ),
                0.0,
                accent,
            );
            ui.painter().text(
                Pos2::new(header.left() + spacing::S5, header.top() + spacing::S3),
                egui::Align2::LEFT_TOP,
                tr("politics"),
                TextRole::Display.font_id(),
                palette::GOLD_HOT,
            );
            ui.painter().text(
                Pos2::new(header.left() + spacing::S5, header.bottom() - spacing::S2),
                egui::Align2::LEFT_BOTTOM,
                if data.country_tag.is_empty() {
                    tr("country")
                } else {
                    data.country_tag.as_str()
                },
                TextRole::Caption.font_id(),
                palette::PARCHMENT_DIM,
            );

            let close_rect = Rect::from_min_size(
                Pos2::new(header.right() - 30.0, header.top() + spacing::S3),
                Vec2::splat(24.0),
            );
            if Button::new("X")
                .size(ButtonSize::Sm)
                .variant(ButtonVariant::Ghost)
                .show_at(ui, close_rect)
                .on_hover_text(tr("panel_close_hint"))
                .clicked()
            {
                close = true;
            }

            draw_politics_tab_strip(ui, tab, "政权总览 / 意识形态 / 国家精神 / 政府制度", accent);
            let mut cmds = Vec::new();
            v9_politics_body(ui, body, data, icon_bank, &mut cmds);
            output = cmds;
        });
    (close, output)
}

fn v9_politics_body(
    ui: &mut egui::Ui,
    rect: Rect,
    data: &PoliticsData,
    icon_bank: &mut crate::icons::IconBank,
    cmds: &mut Vec<DecisionCommand>,
) {
    ui.allocate_ui_at_rect(rect, |ui| {
        ui.set_min_size(rect.size());
        egui::ScrollArea::vertical()
            .id_salt("politics_vanilla_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let width = ui.available_width().max(360.0);
                let overview_h = vanilla_overview_card_height(width);
                let categories_h = vanilla_idea_categories_height();
                let total_h = overview_h + VANILLA_CARD_GAP + categories_h;
                let (canvas, _) = ui.allocate_exact_size(Vec2::new(width, total_h), Sense::hover());
                let overview_rect =
                    Rect::from_min_size(canvas.left_top(), Vec2::new(width, overview_h));
                let categories_rect = Rect::from_min_size(
                    Pos2::new(canvas.left(), overview_rect.bottom() + VANILLA_CARD_GAP),
                    Vec2::new(width, categories_h),
                );

                vanilla_overview_card_at(ui, data, icon_bank, cmds, overview_rect);
                vanilla_idea_categories_at(ui, data, icon_bank, cmds, categories_rect);
            });
    });
}

fn paint_politics_shell(ui: &mut egui::Ui, rect: Rect, accent: Color32) {
    use crate::v9::{paint, tokens::palette};

    ui.painter().rect_filled(rect, 1.0, palette::SOOT_BLACK);
    paint::paint_vertical_gradient_mesh(
        ui.painter(),
        rect.shrink(2.0),
        Color32::from_rgba_premultiplied(0x18, 0x1b, 0x17, 245),
        Color32::from_black_alpha(252),
    );
    ui.painter().rect_stroke(
        rect,
        1.0,
        egui::Stroke::new(2.0, palette::EDGE_DARK),
        egui::epaint::StrokeKind::Inside,
    );
    ui.painter().rect_stroke(
        rect.shrink(2.0),
        1.0,
        egui::Stroke::new(1.0, palette::BRASS_DARK),
        egui::epaint::StrokeKind::Inside,
    );
    ui.painter().rect_stroke(
        rect.shrink(6.0),
        0.0,
        egui::Stroke::new(1.0, Color32::from_black_alpha(230)),
        egui::epaint::StrokeKind::Inside,
    );
    ui.painter().hline(
        (rect.left() + 22.0)..=(rect.right() - 22.0),
        rect.top() + 9.0,
        egui::Stroke::new(1.0, accent),
    );
    ui.painter().hline(
        (rect.left() + 22.0)..=(rect.right() - 22.0),
        rect.bottom() - 9.0,
        egui::Stroke::new(1.0, palette::BRASS_DARK),
    );
}

fn draw_politics_tab_strip(ui: &mut egui::Ui, rect: Rect, label: &str, accent: Color32) {
    use crate::v9::{
        paint,
        tokens::{palette, spacing, TextRole},
    };

    paint::paint_recessed_panel(ui.painter(), rect, 1.0);
    ui.painter().rect_filled(
        Rect::from_min_max(rect.left_top(), Pos2::new(rect.left() + 4.0, rect.bottom())),
        0.0,
        accent,
    );
    ui.painter().hline(
        (rect.left() + spacing::S4)..=(rect.right() - spacing::S4),
        rect.top() + 1.0,
        egui::Stroke::new(1.0, palette::BRASS_DARK),
    );
    ui.painter().text(
        Pos2::new(rect.left() + spacing::S5, rect.center().y),
        egui::Align2::LEFT_CENTER,
        label,
        TextRole::Subheading.font_id(),
        accent,
    );
}

fn vanilla_politics_shell(
    ui: &mut egui::Ui,
    rect: Rect,
    accent: Color32,
    icon_bank: &mut crate::icons::IconBank,
) {
    if let Some(handle) = icon_bank.get_or_load("GFX_pol_view_bg") {
        ui.put(
            rect,
            egui::Image::from_texture(handle).fit_to_exact_size(rect.size()),
        );
        ui.painter()
            .rect_filled(rect, 0.0, Color32::from_black_alpha(82));
    } else if let Some(handle) = icon_bank.get_or_load("GFX_tiled_plain_bg") {
        ui.put(
            rect,
            egui::Image::from_texture(handle).fit_to_exact_size(rect.size()),
        );
        ui.painter()
            .rect_filled(rect, 0.0, Color32::from_black_alpha(118));
    } else {
        VanillaIron::paint_panel(ui, rect, accent);
        return;
    }

    let header_rect = Rect::from_min_size(rect.left_top(), Vec2::new(rect.width(), 48.0));
    if let Some(handle) = icon_bank.get_or_load("GFX_header_bg") {
        ui.put(
            header_rect,
            egui::Image::from_texture(handle).fit_to_exact_size(header_rect.size()),
        );
    }
    VanillaIron::paint_border(ui.painter(), rect);
    ui.painter().rect_stroke(
        rect.shrink(2.0),
        egui::epaint::CornerRadius::same(1),
        egui::Stroke::new(1.0, accent),
        egui::epaint::StrokeKind::Inside,
    );
}

fn paint_vanilla_border(painter: &egui::Painter, rect: Rect) {
    VanillaIron::paint_border(painter, rect);
}

fn vanilla_close_button(ui: &mut egui::Ui, rect: Rect) -> egui::Response {
    VanillaIron::close_button(ui, rect, ui.id().with("politics_vanilla_close"))
}

fn vanilla_black() -> Color32 {
    VanillaIron::BLACK
}

fn vanilla_card() -> Color32 {
    VanillaIron::CARD
}

fn vanilla_edge() -> Color32 {
    VanillaIron::EDGE
}

fn vanilla_text() -> Color32 {
    VanillaIron::TEXT
}

fn vanilla_muted() -> Color32 {
    VanillaIron::MUTED
}

fn v9_card(ui: &mut egui::Ui, height: f32, add_contents: impl FnOnce(&mut egui::Ui, Rect)) {
    let width = ui.available_width().max(360.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, height), Sense::hover());
    v9_card_at(ui, rect, add_contents);
    ui.add_space(VANILLA_CARD_GAP);
}

fn v9_card_at(ui: &mut egui::Ui, rect: Rect, add_contents: impl FnOnce(&mut egui::Ui, Rect)) {
    paint_politics_card_frame(ui, rect);
    let inner = rect.shrink2(Vec2::new(9.0, 7.0));
    add_contents(ui, inner);
}

fn paint_politics_card_frame(ui: &mut egui::Ui, rect: Rect) {
    let painter = ui.painter();
    painter.rect_filled(rect, 1.0, Color32::from_rgb(0x08, 0x09, 0x08));
    crate::v9::paint::paint_vertical_gradient_mesh(
        painter,
        rect.shrink(2.0),
        Color32::from_rgba_premultiplied(0x19, 0x1b, 0x17, 238),
        Color32::from_rgba_premultiplied(0x03, 0x04, 0x03, 252),
    );
    crate::v9::paint::paint_horizontal_gradient_mesh(
        painter,
        rect.shrink(2.0),
        Color32::from_black_alpha(120),
        Color32::from_white_alpha(3),
    );
    crate::v9::paint::paint_plate_grain(painter, rect.shrink(4.0), 3.0, 2);
    paint_vanilla_border(painter, rect);
    painter.rect_stroke(
        rect.shrink(3.0),
        egui::epaint::CornerRadius::same(0),
        egui::Stroke::new(1.0, Color32::from_black_alpha(190)),
        egui::epaint::StrokeKind::Inside,
    );
    painter.hline(
        (rect.left() + 8.0)..=(rect.right() - 8.0),
        rect.top() + 3.0,
        egui::Stroke::new(1.0, Color32::from_white_alpha(12)),
    );
    painter.hline(
        (rect.left() + 8.0)..=(rect.right() - 8.0),
        rect.bottom() - 3.0,
        egui::Stroke::new(1.0, Color32::from_black_alpha(230)),
    );
}

fn vanilla_leader_card_height() -> f32 {
    238.0
}

fn vanilla_ideas_card_height(_data: &PoliticsData) -> f32 {
    122.0
}

fn vanilla_ideology_card_height(data: &PoliticsData) -> f32 {
    (76.0 + normalized_party_popularity(data).len().max(1) as f32 * 24.0).max(206.0)
}

fn vanilla_government_card_height(data: &PoliticsData) -> f32 {
    let rows = data.government_posts.len().max(1);
    (48.0 + rows as f32 * 43.0).max(238.0)
}

#[deprecated(note = "Gate 8 replaced the standalone laws card with idea category rows.")]
fn vanilla_law_systems_card_height(data: &PoliticsData, width: f32) -> f32 {
    let rows = data.law_slots.len().max(1);
    let columns = vanilla_law_columns(width);
    let visible_rows = (rows + columns - 1) / columns;
    (48.0 + visible_rows as f32 * 54.0 + visible_rows.saturating_sub(1) as f32 * 7.0).max(272.0)
}

fn vanilla_law_columns(width: f32) -> usize {
    if width >= 520.0 {
        2
    } else {
        1
    }
}

fn vanilla_idea_categories_height() -> f32 {
    vanilla_idea_category_row_height() + vanilla_idea_category_row_step() * 2.0
}

fn vanilla_idea_category_template() -> Option<&'static crate::vanilla_gui::GuiNode> {
    politics_vanilla_gui_context().and_then(|context| {
        context.root_template(
            COUNTRY_POLITICS_GUI_FILE,
            COUNTRY_POLITICS_IDEA_CATEGORY_TEMPLATE,
        )
    })
}

fn vanilla_idea_category_row_height() -> f32 {
    vanilla_idea_category_template()
        .and_then(|node| node.block("size"))
        .and_then(|size| size.get("height"))
        .and_then(crate::vanilla_gui::GuiValueExt::as_lossy_f32)
        .unwrap_or(VANILLA_IDEA_CATEGORY_ROW_H)
}

fn vanilla_idea_category_row_step() -> f32 {
    let row_h = vanilla_idea_category_row_height();
    let parent_stride = politics_vanilla_gui_context()
        .and_then(|context| context.document(COUNTRY_POLITICS_GUI_FILE))
        .and_then(|document| document.find_node_by_name("idea_categories_grid"))
        .and_then(|grid| grid.block("slotsize"))
        .and_then(|slotsize| slotsize.get("height"))
        .and_then(crate::vanilla_gui::GuiValueExt::as_lossy_f32)
        .unwrap_or(row_h + VANILLA_IDEA_CATEGORY_GAP);
    let row = Rect::from_min_size(Pos2::ZERO, Vec2::new(550.0, row_h));
    let slot_extent = vanilla_idea_category_slot_rects(row, 1)
        .into_iter()
        .map(|slot| slot.bottom() - row.top())
        .fold(row_h, f32::max);

    parent_stride.max(slot_extent).max(row_h)
}

fn vanilla_idea_category_slot_rects(row: Rect, count: usize) -> Vec<Rect> {
    let Some(template) = vanilla_idea_category_template() else {
        return fallback_idea_category_slot_rects(row, count);
    };
    let Some(grid) = template.find_node_by_name("ideas_grid") else {
        return fallback_idea_category_slot_rects(row, count);
    };
    let layout = crate::vanilla_gui::compute_layout_tree(
        template,
        &crate::vanilla_gui::LayoutOptions::new(crate::vanilla_gui::GuiRect::new(
            row.left(),
            row.top(),
            row.width(),
            row.height(),
        )),
    );
    let Some(grid_layout) = layout.find_by_name("ideas_grid") else {
        return fallback_idea_category_slot_rects(row, count);
    };
    crate::vanilla_gui::grid_slots(grid, grid_layout.rect, count)
        .into_iter()
        .map(Into::into)
        .collect()
}

fn fallback_idea_category_slot_rects(row: Rect, count: usize) -> Vec<Rect> {
    let content = Rect::from_min_max(
        Pos2::new(row.left() + 92.0, row.top() + 30.0),
        Pos2::new(row.right() - 8.0, row.bottom() - 8.0),
    );
    let gap = 6.0;
    let slot_count = count.max(1);
    let slot_w = vanilla_slot_width_for_count(content.width(), slot_count, gap, 66.0);
    (0..count)
        .map(|idx| {
            Rect::from_min_size(
                Pos2::new(content.left() + idx as f32 * (slot_w + gap), content.top()),
                Vec2::new(slot_w, 54.0),
            )
        })
        .collect()
}

fn vanilla_slot_width_for_count(
    available_width: f32,
    count: usize,
    gap: f32,
    max_width: f32,
) -> f32 {
    if count == 0 {
        return max_width.max(1.0);
    }
    let gaps = gap * count.saturating_sub(1) as f32;
    ((available_width - gaps).max(count as f32) / count as f32)
        .min(max_width)
        .max(1.0)
}

fn vanilla_overview_card_height(width: f32) -> f32 {
    let inner_width = width - 18.0;
    if inner_width >= 540.0 {
        432.0
    } else {
        520.0
    }
}

fn vanilla_overview_card(
    ui: &mut egui::Ui,
    data: &PoliticsData,
    icon_bank: &mut crate::icons::IconBank,
    cmds: &mut Vec<DecisionCommand>,
    height: f32,
) {
    v9_card(ui, height, |ui, inner| {
        vanilla_overview_card_contents(ui, inner, data, icon_bank, cmds);
    });
}

fn vanilla_overview_card_at(
    ui: &mut egui::Ui,
    data: &PoliticsData,
    icon_bank: &mut crate::icons::IconBank,
    cmds: &mut Vec<DecisionCommand>,
    rect: Rect,
) {
    v9_card_at(ui, rect, |ui, inner| {
        vanilla_overview_card_contents(ui, inner, data, icon_bank, cmds);
    });
}

fn vanilla_overview_card_contents(
    ui: &mut egui::Ui,
    inner: Rect,
    data: &PoliticsData,
    icon_bank: &mut crate::icons::IconBank,
    cmds: &mut Vec<DecisionCommand>,
) {
    vanilla_section_title(ui, inner, "\u{56fd}\u{5bb6}\u{6982}\u{89c8}");
    let content = Rect::from_min_max(
        Pos2::new(inner.left(), inner.top() + 34.0),
        inner.right_bottom(),
    );
    let link_rect = Rect::from_min_max(
        Pos2::new(content.left(), content.bottom() - 36.0),
        content.right_bottom(),
    );
    let main = Rect::from_min_max(
        content.left_top(),
        Pos2::new(content.right(), link_rect.top() - 8.0),
    );

    if main.width() >= 540.0 {
        let portrait_w = 148.0;
        let portrait_h = 190.0_f32.min(main.height() - 116.0).max(154.0);
        let portrait_rect = Rect::from_min_size(
            main.left_top() + Vec2::new(2.0, 2.0),
            Vec2::new(portrait_w, portrait_h),
        );
        draw_vanilla_portrait(ui, portrait_rect, data, icon_bank);
        vanilla_leader_nameplate(ui, portrait_rect, data);

        let right = Rect::from_min_max(
            Pos2::new(portrait_rect.right() + 10.0, main.top() + 2.0),
            main.right_bottom(),
        );
        let focus_rect = Rect::from_min_size(right.left_top(), Vec2::new(right.width(), 58.0));
        vanilla_focus_selector_at(ui, focus_rect, data, cmds);

        let ideas_rect = Rect::from_min_size(
            Pos2::new(right.left(), focus_rect.bottom() + 8.0),
            Vec2::new(right.width(), 76.0),
        );
        vanilla_idea_strip_at(ui, ideas_rect, data, icon_bank);

        let gov_rect = Rect::from_min_max(
            Pos2::new(portrait_rect.left(), portrait_rect.bottom() + 40.0),
            Pos2::new(portrait_rect.right(), main.bottom()),
        );
        let ideology_rect = Rect::from_min_max(
            Pos2::new(right.left(), ideas_rect.bottom() + 8.0),
            right.right_bottom(),
        );
        vanilla_government_mini_at(ui, gov_rect, data);
        vanilla_ideology_summary_at(ui, ideology_rect, data);
    } else {
        let portrait_rect = Rect::from_min_size(
            main.left_top() + Vec2::new(2.0, 2.0),
            Vec2::new(118.0, 166.0),
        );
        draw_vanilla_portrait(ui, portrait_rect, data, icon_bank);
        vanilla_leader_nameplate(ui, portrait_rect, data);

        let right = Rect::from_min_max(
            Pos2::new(portrait_rect.right() + 10.0, main.top() + 2.0),
            Pos2::new(main.right(), portrait_rect.bottom()),
        );
        let focus_rect = Rect::from_min_size(right.left_top(), Vec2::new(right.width(), 58.0));
        vanilla_focus_selector_at(ui, focus_rect, data, cmds);
        let ideology_rect = Rect::from_min_max(
            Pos2::new(right.left(), focus_rect.bottom() + 8.0),
            right.right_bottom(),
        );
        vanilla_ideology_summary_at(ui, ideology_rect, data);

        let ideas_rect = Rect::from_min_size(
            Pos2::new(main.left(), portrait_rect.bottom() + 34.0),
            Vec2::new(main.width(), 76.0),
        );
        vanilla_idea_strip_at(ui, ideas_rect, data, icon_bank);
        let gov_rect = Rect::from_min_max(
            Pos2::new(main.left(), ideas_rect.bottom() + 8.0),
            main.right_bottom(),
        );
        vanilla_government_mini_at(ui, gov_rect, data);
    }
    vanilla_politics_link_strip_at(ui, link_rect, cmds);
}

fn vanilla_politics_link_strip_at(ui: &mut egui::Ui, rect: Rect, cmds: &mut Vec<DecisionCommand>) {
    let gap = 8.0;
    let button_w = (rect.width() - gap * 2.0) / 3.0;
    let buttons = [
        ("\u{5360}\u{9886}\u{5730}\u{533a}", None),
        ("\u{9635}\u{8425}\u{56fd}", None),
        (
            "\u{5916}\u{4ea4}\u{4e8b}\u{52a1}",
            Some(ActivePrimaryPanel::Diplomacy),
        ),
    ];

    for (idx, (label, panel)) in buttons.iter().enumerate() {
        let button = Rect::from_min_size(
            Pos2::new(rect.left() + idx as f32 * (button_w + gap), rect.top()),
            Vec2::new(button_w, rect.height()),
        );
        let response = ui.interact(button, ui.id().with(("politics_link", idx)), Sense::click());
        let enabled = panel.is_some();
        let fill = if response.hovered() && enabled {
            Color32::from_rgb(0x2a, 0x2e, 0x29)
        } else {
            Color32::from_rgb(0x23, 0x26, 0x22)
        };
        ui.painter().rect_filled(button, 1.0, fill);
        paint_vanilla_border(ui.painter(), button);
        ui.painter().text(
            button.center(),
            egui::Align2::CENTER_CENTER,
            *label,
            fit_text_font(
                *label,
                crate::v9::TextRole::Body.font_id(),
                button.width() - 12.0,
            ),
            if enabled {
                vanilla_text()
            } else {
                vanilla_muted()
            },
        );
        if response.clicked() {
            if let Some(panel) = panel {
                cmds.push(DecisionCommand::Panel(PanelCommand::OpenPrimary(*panel)));
            }
        }
        if response.hovered() && enabled {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
    }
}

fn vanilla_leader_nameplate(ui: &mut egui::Ui, portrait_rect: Rect, data: &PoliticsData) {
    let leader = if data.leader_name.is_empty() {
        tr("leader_unknown")
    } else {
        data.leader_name.as_str()
    };
    let party = if data.party_full_name.is_empty() {
        ideology_label(&data.ruling_party)
    } else {
        data.party_full_name.as_str()
    };
    let name_rect = Rect::from_min_size(
        Pos2::new(portrait_rect.left(), portrait_rect.bottom() + 6.0),
        Vec2::new(portrait_rect.width(), 28.0),
    );
    ui.painter().text(
        Pos2::new(name_rect.center().x, name_rect.top()),
        egui::Align2::CENTER_TOP,
        leader,
        fit_text_font(
            leader,
            crate::v9::TextRole::Body.font_id(),
            name_rect.width(),
        ),
        vanilla_text(),
    );
    ui.painter().text(
        Pos2::new(name_rect.center().x, name_rect.top() + 16.0),
        egui::Align2::CENTER_TOP,
        party,
        fit_text_font(
            party,
            crate::v9::TextRole::Caption.font_id(),
            name_rect.width(),
        ),
        v9_ideology_color(&data.ruling_party),
    );
}

fn vanilla_focus_selector_at(
    ui: &mut egui::Ui,
    rect: Rect,
    data: &PoliticsData,
    cmds: &mut Vec<DecisionCommand>,
) {
    let sense = if data.focus_available {
        Sense::click()
    } else {
        Sense::hover()
    };
    let response = ui.interact(rect, ui.id().with("vanilla_focus_selector"), sense);
    let fill = if response.hovered() && data.focus_available {
        Color32::from_rgb(0x1c, 0x27, 0x25)
    } else {
        Color32::from_rgb(0x08, 0x13, 0x14)
    };
    ui.painter().rect_filled(rect, 1.0, fill);
    paint_vanilla_border(ui.painter(), rect);

    let progress = focus_progress(data);
    let label = if let Some(name) = data.current_focus_name.as_deref() {
        name
    } else if data.focus_available {
        "\u{9009}\u{62e9}\u{4e00}\u{4e2a}\u{56fd}\u{7b56}"
    } else {
        "\u{56fd}\u{7b56}\u{6811}\u{672a}\u{63a5}\u{5165}"
    };
    let title = if data.current_focus_name.is_some() {
        "\u{5f53}\u{524d}\u{56fd}\u{7b56}"
    } else {
        "\u{56fd}\u{7b56}"
    };
    ui.painter().text(
        Pos2::new(rect.left() + 12.0, rect.top() + 8.0),
        egui::Align2::LEFT_TOP,
        title,
        crate::v9::TextRole::Caption.font_id(),
        vanilla_muted(),
    );
    ui.painter().text(
        Pos2::new(rect.center().x, rect.center().y + 1.0),
        egui::Align2::CENTER_CENTER,
        label,
        fit_text_font(
            label,
            crate::v9::TextRole::Subheading.font_id(),
            rect.width() - 76.0,
        ),
        vanilla_text(),
    );

    if data.current_focus_name.is_some() {
        let bar_bg = Rect::from_min_max(
            Pos2::new(rect.left() + 12.0, rect.bottom() - 9.0),
            Pos2::new(rect.right() - 12.0, rect.bottom() - 5.0),
        );
        ui.painter()
            .rect_filled(bar_bg, 0.0, Color32::from_black_alpha(180));
        let fill = Rect::from_min_max(
            bar_bg.left_top(),
            Pos2::new(bar_bg.left() + bar_bg.width() * progress, bar_bg.bottom()),
        );
        ui.painter().rect_filled(fill, 0.0, vanilla_gold());
    }

    if response.clicked() && data.focus_available {
        cmds.push(DecisionCommand::OpenFocusTree);
    }
    if response.hovered() && data.focus_available {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
}

fn vanilla_idea_strip_at(
    ui: &mut egui::Ui,
    rect: Rect,
    data: &PoliticsData,
    icon_bank: &mut crate::icons::IconBank,
) {
    vanilla_row_frame(ui, rect);
    ui.painter().text(
        Pos2::new(rect.left() + 8.0, rect.top() + 7.0),
        egui::Align2::LEFT_TOP,
        tr("national_spirits"),
        crate::v9::TextRole::Caption.font_id(),
        vanilla_muted(),
    );

    let slot_size = 42.0;
    let gap = 7.0;
    let first_x = rect.left() + 8.0;
    let slot_top = rect.top() + 27.0;
    let max_slots = (((rect.width() - 16.0 + gap) / (slot_size + gap)).floor() as usize).max(1);

    let spirits = national_spirit_ideas(data);
    if spirits.is_empty() {
        let slot = Rect::from_min_size(Pos2::new(first_x, slot_top), Vec2::splat(slot_size));
        vanilla_slot(ui, slot, Color32::from_rgb(0x34, 0x35, 0x30));
        ui.painter().text(
            Pos2::new(slot.right() + 10.0, slot.center().y),
            egui::Align2::LEFT_CENTER,
            tr("idea_none"),
            crate::v9::TextRole::Body.font_id(),
            vanilla_muted(),
        );
        return;
    }

    let visible = if spirits.len() > max_slots && max_slots > 1 {
        max_slots - 1
    } else {
        spirits.len().min(max_slots)
    };
    for (idx, idea) in spirits.iter().take(visible).enumerate() {
        let slot = Rect::from_min_size(
            Pos2::new(first_x + idx as f32 * (slot_size + gap), slot_top),
            Vec2::splat(slot_size),
        );
        vanilla_slot(ui, slot, vanilla_edge());
        if let Some(handle) = idea_icon_gfx(idea).and_then(|gfx| icon_bank.get_or_load(&gfx)) {
            ui.put(
                slot.shrink(4.0),
                egui::Image::from_texture(handle).fit_to_exact_size(slot.shrink(4.0).size()),
            );
        } else {
            let letter = idea
                .name
                .chars()
                .find(|c| !c.is_whitespace())
                .unwrap_or('?');
            ui.painter().text(
                slot.center(),
                egui::Align2::CENTER_CENTER,
                letter.to_string(),
                crate::v9::TextRole::Subheading.font_id(),
                vanilla_gold(),
            );
        }
        ui.interact(
            slot,
            ui.id().with(("vanilla_overview_idea", &idea.key)),
            Sense::hover(),
        )
        .on_hover_ui(|ui| render_idea_tooltip(ui, idea));
    }

    if spirits.len() > visible {
        let extra = spirits.len() - visible;
        let text = format!("+{extra}");
        let pos = Pos2::new(rect.right() - 8.0, slot_top + slot_size * 0.5);
        ui.painter().text(
            pos,
            egui::Align2::RIGHT_CENTER,
            text,
            crate::v9::TextRole::Body.font_id(),
            vanilla_muted(),
        );
    }
}

fn vanilla_government_mini_at(ui: &mut egui::Ui, rect: Rect, data: &PoliticsData) {
    vanilla_row_frame(ui, rect);
    ui.painter().text(
        Pos2::new(rect.left() + 8.0, rect.top() + 7.0),
        egui::Align2::LEFT_TOP,
        "\u{653f}\u{5e9c}",
        crate::v9::TextRole::Caption.font_id(),
        vanilla_muted(),
    );

    if data.government_posts.is_empty() {
        vanilla_empty_text(
            ui,
            rect,
            "\u{6682}\u{65e0}\u{653f}\u{5e9c}\u{804c}\u{4f4d}\u{6570}\u{636e}",
        );
        return;
    }

    let top = rect.top() + 25.0;
    let row_gap = 3.0;
    let min_row_h = 22.0;
    let available_h = (rect.bottom() - top - 5.0).max(0.0);
    if available_h < min_row_h {
        return;
    }
    let max_rows_by_height = ((available_h + row_gap) / (min_row_h + row_gap))
        .floor()
        .max(1.0) as usize;
    let rows = data.government_posts.len().min(3).min(max_rows_by_height);
    let row_h = ((available_h - row_gap * rows.saturating_sub(1) as f32) / rows as f32)
        .clamp(min_row_h, 30.0);
    for (idx, post) in data.government_posts.iter().take(rows).enumerate() {
        let row = Rect::from_min_size(
            Pos2::new(rect.left() + 7.0, top + idx as f32 * (row_h + row_gap)),
            Vec2::new(rect.width() - 14.0, row_h),
        );
        vanilla_government_mini_row(ui, row, post, idx == 0, &data.ruling_party);
    }
}

fn vanilla_government_mini_row(
    ui: &mut egui::Ui,
    rect: Rect,
    post: &GovernmentPostEntry,
    primary: bool,
    ruling_party: &str,
) {
    ui.painter()
        .rect_filled(rect, 1.0, Color32::from_rgb(0x0c, 0x0f, 0x0e));
    ui.painter().rect_stroke(
        rect,
        egui::epaint::CornerRadius::same(1),
        egui::Stroke::new(1.0, Color32::from_black_alpha(210)),
        egui::epaint::StrokeKind::Inside,
    );
    let accent = if primary {
        v9_ideology_color(ruling_party)
    } else {
        vanilla_edge()
    };
    let icon = Rect::from_min_size(rect.left_top() + Vec2::new(4.0, 4.0), Vec2::splat(20.0));
    vanilla_slot(ui, icon, accent);
    draw_panel_svg_icon(ui, icon.shrink(4.0), PanelSvgIcon::Person, vanilla_muted());
    let text_left = icon.right() + 7.0;
    ui.painter().text(
        Pos2::new(text_left, rect.top() + 2.0),
        egui::Align2::LEFT_TOP,
        post.office.as_str(),
        fit_text_font(
            post.office.as_str(),
            crate::v9::TextRole::Caption.font_id(),
            rect.width() * 0.42,
        ),
        vanilla_muted(),
    );
    ui.painter().text(
        Pos2::new(text_left, rect.top() + 15.0),
        egui::Align2::LEFT_TOP,
        post.name.as_str(),
        fit_text_font(
            post.name.as_str(),
            crate::v9::TextRole::Caption.font_id(),
            if rect.width() >= 170.0 && !post.detail.is_empty() {
                rect.width() * 0.50
            } else {
                rect.right() - text_left - 8.0
            },
        ),
        vanilla_text(),
    );
    if rect.width() >= 170.0 && !post.detail.is_empty() {
        ui.painter().text(
            Pos2::new(rect.right() - 6.0, rect.top() + 8.0),
            egui::Align2::RIGHT_TOP,
            post.detail.as_str(),
            fit_text_font(
                post.detail.as_str(),
                crate::v9::TextRole::Caption.font_id(),
                rect.width() * 0.34,
            ),
            accent,
        );
    }
}

fn vanilla_ideology_summary_at(ui: &mut egui::Ui, rect: Rect, data: &PoliticsData) {
    vanilla_row_frame(ui, rect);
    ui.painter().text(
        Pos2::new(rect.left() + 8.0, rect.top() + 7.0),
        egui::Align2::LEFT_TOP,
        "\u{610f}\u{8bc6}\u{5f62}\u{6001}",
        crate::v9::TextRole::Caption.font_id(),
        vanilla_muted(),
    );
    let support = ruling_party_support(data);
    let label = ideology_label(&data.ruling_party);
    let accent = v9_ideology_color(&data.ruling_party);
    let party = if data.party_full_name.is_empty() {
        label
    } else {
        data.party_full_name.as_str()
    };
    let popularity = normalized_party_popularity(data);
    let compact = rect.height() < 160.0;
    if compact && rect.width() >= 250.0 {
        ui.painter().text(
            Pos2::new(rect.left() + 14.0, rect.top() + 28.0),
            egui::Align2::LEFT_TOP,
            party,
            fit_text_font(
                party,
                crate::v9::TextRole::Body.font_id(),
                rect.width() - 28.0,
            ),
            accent,
        );
        let donut = Rect::from_center_size(
            Pos2::new(rect.left() + 52.0, rect.top() + 76.0),
            Vec2::splat((rect.height() - 50.0).clamp(42.0, 72.0)),
        );
        draw_vanilla_ideology_donut(ui, donut, &popularity);
        let list_left = donut.right() + 13.0;
        let list_top = rect.top() + 52.0;
        for (idx, (key, pop)) in popularity.iter().take(4).enumerate() {
            let row = Rect::from_min_size(
                Pos2::new(list_left, list_top + idx as f32 * 15.0),
                Vec2::new((rect.right() - list_left - 9.0).max(80.0), 13.0),
            );
            let color = v9_ideology_color(key);
            ui.painter().rect_filled(
                Rect::from_min_size(row.left_top() + Vec2::new(0.0, 2.0), Vec2::new(16.0, 8.0)),
                0.0,
                color,
            );
            ui.painter().text(
                Pos2::new(row.left() + 22.0, row.top() - 1.0),
                egui::Align2::LEFT_TOP,
                ideology_label(key),
                fit_text_font(
                    ideology_label(key),
                    crate::v9::TextRole::Caption.font_id(),
                    row.width() - 58.0,
                ),
                vanilla_text(),
            );
            ui.painter().text(
                row.right_top() + Vec2::new(0.0, -1.0),
                egui::Align2::RIGHT_TOP,
                format!("{:.0}%", pop * 100.0),
                crate::v9::TextRole::Caption.font_id(),
                color,
            );
        }
        ui.painter().text(
            Pos2::new(rect.right() - 9.0, rect.bottom() - 9.0),
            egui::Align2::RIGHT_BOTTOM,
            "Faction: N/A",
            crate::v9::TextRole::Small.font_id(),
            vanilla_muted(),
        );
        return;
    }
    if !compact {
        ui.painter().text(
            Pos2::new(rect.center().x, rect.top() + 28.0),
            egui::Align2::CENTER_TOP,
            party,
            fit_text_font(
                party,
                crate::v9::TextRole::Body.font_id(),
                rect.width() - 16.0,
            ),
            accent,
        );
    }
    let donut_top = if compact {
        rect.top() + 28.0
    } else {
        rect.top() + 52.0
    };
    let bottom_reserved = if compact { 32.0 } else { 48.0 };
    let max_donut_h = (rect.bottom() - bottom_reserved - donut_top).max(24.0);
    let donut_size = max_donut_h.min((rect.width() - 22.0).max(24.0)).min(148.0);
    let donut = Rect::from_center_size(
        Pos2::new(rect.center().x, donut_top + donut_size * 0.5),
        Vec2::splat(donut_size),
    );
    draw_vanilla_ideology_donut(ui, donut, &popularity);
    ui.painter().text(
        Pos2::new(rect.center().x, rect.bottom() - 24.0),
        egui::Align2::CENTER_CENTER,
        label,
        fit_text_font(
            label,
            crate::v9::TextRole::Body.font_id(),
            rect.width() - 12.0,
        ),
        accent,
    );
    ui.painter().text(
        Pos2::new(rect.center().x, rect.bottom() - 9.0),
        egui::Align2::CENTER_CENTER,
        format!("{:.0}% \u{652f}\u{6301}", support * 100.0),
        crate::v9::TextRole::Caption.font_id(),
        vanilla_text(),
    );
}

fn focus_progress(data: &PoliticsData) -> f32 {
    data.current_focus_cost_days
        .filter(|days| *days > 0)
        .map(|days| (data.current_focus_progress / days as f32).clamp(0.0, 1.0))
        .unwrap_or(0.0)
}

fn vanilla_leader_card(
    ui: &mut egui::Ui,
    data: &PoliticsData,
    icon_bank: &mut crate::icons::IconBank,
    cmds: &mut Vec<DecisionCommand>,
    height: f32,
) {
    v9_card(ui, height, |ui, inner| {
        vanilla_section_title(ui, inner, "\u{6267}\u{653f}\u{515a}");

        let content = Rect::from_min_max(
            Pos2::new(inner.left(), inner.top() + 34.0),
            inner.right_bottom(),
        );
        let portrait_rect = Rect::from_min_size(
            content.left_top() + Vec2::new(2.0, 4.0),
            Vec2::new(102.0, 148.0),
        );
        draw_vanilla_portrait(ui, portrait_rect, data, icon_bank);

        let text_left = portrait_rect.right() + 18.0;
        let text_rect = Rect::from_min_max(
            Pos2::new(text_left, portrait_rect.top() + 2.0),
            Pos2::new(content.right() - 4.0, portrait_rect.bottom()),
        );
        let leader = if data.leader_name.is_empty() {
            tr("leader_unknown").to_owned()
        } else {
            data.leader_name.clone()
        };
        let party = if data.party_full_name.is_empty() {
            ideology_label(&data.ruling_party).to_owned()
        } else {
            data.party_full_name.clone()
        };
        let painter = ui.painter().with_clip_rect(text_rect);
        painter.text(
            text_rect.left_top(),
            egui::Align2::LEFT_TOP,
            leader.as_str(),
            fit_text_font(
                leader.as_str(),
                crate::v9::TextRole::Heading.font_id(),
                text_rect.width(),
            ),
            vanilla_gold(),
        );
        painter.text(
            Pos2::new(text_rect.left(), text_rect.top() + 30.0),
            egui::Align2::LEFT_TOP,
            party.as_str(),
            fit_text_font(
                party.as_str(),
                crate::v9::TextRole::Body.font_id(),
                text_rect.width(),
            ),
            v9_ideology_color(&data.ruling_party),
        );
        vanilla_info_line(
            ui,
            Pos2::new(text_rect.left(), text_rect.top() + 70.0),
            "\u{610f}\u{8bc6}\u{5f62}\u{6001}:",
            ideology_label(&data.ruling_party),
            v9_ideology_color(&data.ruling_party),
        );
        vanilla_info_line(
            ui,
            Pos2::new(text_rect.left(), text_rect.top() + 96.0),
            "\u{6267}\u{653f}\u{5730}\u{4f4d}:",
            "\u{6267}\u{653f}\u{515a}",
            vanilla_gold(),
        );
        vanilla_info_line(
            ui,
            Pos2::new(text_rect.left(), text_rect.top() + 122.0),
            "\u{4e0b}\u{4e00}\u{6b21}\u{9009}\u{4e3e}:",
            "1940\u{5e74}1\u{6708}",
            vanilla_text(),
        );

        if data.focus_available {
            let button = Rect::from_min_size(
                Pos2::new(content.right() - 126.0, content.bottom() - 34.0),
                Vec2::new(118.0, 28.0),
            );
            if vanilla_action_button(ui, button, "\u{56fd}\u{7b56}") {
                cmds.push(DecisionCommand::OpenFocusTree);
            }
        }
    });
}

fn vanilla_ideas_card(
    ui: &mut egui::Ui,
    data: &PoliticsData,
    icon_bank: &mut crate::icons::IconBank,
    height: f32,
) {
    v9_card(ui, height, |ui, inner| {
        vanilla_section_title(ui, inner, tr("national_spirits"));
        let content = Rect::from_min_max(
            Pos2::new(inner.left(), inner.top() + 34.0),
            inner.right_bottom(),
        );
        let spirits = national_spirit_ideas(data);
        if spirits.is_empty() {
            let slot = Rect::from_min_size(
                Pos2::new(content.left() + 12.0, content.top() + 8.0),
                Vec2::splat(50.0),
            );
            vanilla_slot(ui, slot, Color32::from_rgb(0x34, 0x35, 0x30));
            ui.painter().text(
                slot.center(),
                egui::Align2::CENTER_CENTER,
                "?",
                crate::v9::TextRole::Heading.font_id(),
                Color32::from_rgb(0x4c, 0x4b, 0x43),
            );
            ui.painter().text(
                Pos2::new(slot.right() + 14.0, slot.center().y),
                egui::Align2::LEFT_CENTER,
                tr("idea_none"),
                crate::v9::TextRole::Body.font_id(),
                vanilla_muted(),
            );
            return;
        }

        let slot = Vec2::splat(54.0);
        let gap = 8.0;
        let cols = (((content.width() + gap) / (slot.x + gap)).floor() as usize).clamp(1, 7);
        for (idx, idea) in spirits.iter().enumerate() {
            let col = idx % cols;
            let row = idx / cols;
            let rect = Rect::from_min_size(
                content.left_top()
                    + Vec2::new(col as f32 * (slot.x + gap), row as f32 * (slot.y + gap)),
                slot,
            );
            vanilla_slot(ui, rect, vanilla_edge());
            if let Some(handle) = idea_icon_gfx(idea).and_then(|gfx| icon_bank.get_or_load(&gfx)) {
                ui.put(
                    rect.shrink(5.0),
                    egui::Image::from_texture(handle).fit_to_exact_size(rect.shrink(5.0).size()),
                );
            } else {
                let letter = idea
                    .name
                    .chars()
                    .find(|c| !c.is_whitespace())
                    .unwrap_or('?');
                ui.painter().text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    letter.to_string(),
                    crate::v9::TextRole::Heading.font_id(),
                    vanilla_gold(),
                );
            }
            ui.interact(
                rect,
                ui.id().with(("vanilla_idea", &idea.key)),
                Sense::hover(),
            )
            .on_hover_ui(|ui| render_idea_tooltip(ui, idea));
        }
    });
}

fn vanilla_ideology_card(ui: &mut egui::Ui, data: &PoliticsData, height: f32) {
    v9_card(ui, height, |ui, inner| {
        let popularity = normalized_party_popularity(data);
        vanilla_section_title(ui, inner, "\u{610f}\u{8bc6}\u{5f62}\u{6001}");
        let content = Rect::from_min_max(
            Pos2::new(inner.left(), inner.top() + 40.0),
            inner.right_bottom(),
        );
        let donut = Rect::from_min_size(
            Pos2::new(content.left() + 26.0, content.top() + 10.0),
            Vec2::splat(128.0),
        );
        draw_vanilla_ideology_donut(ui, donut, &popularity);

        let legend = Rect::from_min_max(
            Pos2::new(donut.right() + 26.0, content.top() + 4.0),
            content.right_bottom(),
        );
        let mut y = legend.top();
        for (key, pop) in &popularity {
            let color = v9_ideology_color(key);
            let row =
                Rect::from_min_size(Pos2::new(legend.left(), y), Vec2::new(legend.width(), 24.0));
            ui.painter()
                .circle_filled(Pos2::new(row.left() + 7.0, row.center().y), 5.0, color);
            ui.painter().text(
                Pos2::new(row.left() + 20.0, row.center().y),
                egui::Align2::LEFT_CENTER,
                ideology_label(key),
                crate::v9::TextRole::Body.font_id(),
                vanilla_text(),
            );
            ui.painter().text(
                Pos2::new(row.right(), row.center().y),
                egui::Align2::RIGHT_CENTER,
                format!("{:.0}%", pop * 100.0),
                crate::v9::TextRole::Body.font_id(),
                vanilla_text(),
            );
            y += 28.0;
        }
    });
}

fn vanilla_government_card(ui: &mut egui::Ui, data: &PoliticsData, height: f32) {
    v9_card(ui, height, |ui, inner| {
        vanilla_section_title(ui, inner, "\u{653f}\u{5e9c}");
        if data.government_posts.is_empty() {
            vanilla_empty_text(
                ui,
                inner,
                "\u{6682}\u{65e0}\u{653f}\u{5e9c}\u{804c}\u{4f4d}\u{6570}\u{636e}",
            );
            return;
        }
        let mut y = inner.top() + 36.0;
        for (idx, post) in data.government_posts.iter().enumerate() {
            let rect =
                Rect::from_min_size(Pos2::new(inner.left(), y), Vec2::new(inner.width(), 37.0));
            let accent = if idx == 0 {
                v9_ideology_color(&data.ruling_party)
            } else {
                vanilla_edge()
            };
            vanilla_person_row(ui, rect, &post.office, &post.name, &post.detail, accent);
            y += 43.0;
        }
    });
}

#[deprecated(note = "Gate 8 replaced the standalone laws card with idea category rows.")]
fn vanilla_law_systems_card(
    ui: &mut egui::Ui,
    data: &PoliticsData,
    cmds: &mut Vec<DecisionCommand>,
    height: f32,
) {
    v9_card(ui, height, |ui, inner| {
        vanilla_law_systems_card_contents(ui, inner, data, cmds);
    });
}

#[deprecated(note = "Gate 8 replaced the standalone laws card with idea category rows.")]
fn vanilla_law_systems_card_at(
    ui: &mut egui::Ui,
    data: &PoliticsData,
    cmds: &mut Vec<DecisionCommand>,
    rect: Rect,
) {
    v9_card_at(ui, rect, |ui, inner| {
        vanilla_law_systems_card_contents(ui, inner, data, cmds);
    });
}

fn vanilla_law_systems_card_contents(
    ui: &mut egui::Ui,
    inner: Rect,
    data: &PoliticsData,
    cmds: &mut Vec<DecisionCommand>,
) {
    vanilla_section_title(ui, inner, "Government / Laws");
    if data.law_slots.is_empty() {
        vanilla_empty_text(
            ui,
            inner,
            "\u{6682}\u{65e0}\u{653f}\u{7b56}\u{6570}\u{636e}",
        );
        return;
    }
    let content = Rect::from_min_max(
        Pos2::new(inner.left(), inner.top() + 36.0),
        inner.right_bottom(),
    );
    let columns = vanilla_law_columns(inner.width());
    let gap = 8.0;
    let row_h = 47.0;
    let row_w = if columns > 1 {
        (content.width() - gap) * 0.5
    } else {
        content.width()
    };
    for (idx, slot) in data.law_slots.iter().enumerate() {
        let col = idx % columns;
        let row = idx / columns;
        let rect = Rect::from_min_size(
            Pos2::new(
                content.left() + col as f32 * (row_w + gap),
                content.top() + row as f32 * (row_h + 7.0),
            ),
            Vec2::new(row_w, row_h),
        );
        let response = ui.interact(
            rect,
            ui.id().with((
                "politics_law_detail",
                law_panel::law_category_key(slot.category),
            )),
            Sense::click(),
        );
        if response.clicked() {
            cmds.push(DecisionCommand::Panel(PanelCommand::OpenDetail(
                ActiveDetailPanel::Law {
                    category: law_panel::law_category_key(slot.category).to_owned(),
                    law_id: None,
                },
            )));
        }
        response.on_hover_text(politics_law_tooltip(slot));
        let status = politics_law_status_text(slot);
        vanilla_law_row(ui, rect, slot, &status);
    }
}

fn vanilla_idea_categories_at(
    ui: &mut egui::Ui,
    data: &PoliticsData,
    icon_bank: &mut crate::icons::IconBank,
    cmds: &mut Vec<DecisionCommand>,
    rect: Rect,
) {
    let mut y = rect.top();
    let rows = [
        IdeaCategoryRowKind::GovernmentLaws,
        IdeaCategoryRowKind::ResearchProduction,
        IdeaCategoryRowKind::MilitaryStaff,
    ];
    let row_h = vanilla_idea_category_row_height();
    let row_step = vanilla_idea_category_row_step();
    for kind in rows {
        let row = Rect::from_min_size(Pos2::new(rect.left(), y), Vec2::new(rect.width(), row_h));
        vanilla_idea_category_row(ui, row, kind, data, icon_bank, cmds);
        y += row_step;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IdeaCategoryRowKind {
    GovernmentLaws,
    ResearchProduction,
    MilitaryStaff,
}

impl IdeaCategoryRowKind {
    fn title(self) -> &'static str {
        match self {
            Self::GovernmentLaws => "Government / Laws",
            Self::ResearchProduction => "Research & Production",
            Self::MilitaryStaff => "Military Staff",
        }
    }

    fn localized_title(self) -> &'static str {
        match self {
            Self::GovernmentLaws => tr("government_laws"),
            Self::ResearchProduction => tr("research_production"),
            Self::MilitaryStaff => tr("military_staff"),
        }
    }

    fn category_frame(self) -> u32 {
        match self {
            Self::GovernmentLaws => 5,
            Self::ResearchProduction => 2,
            Self::MilitaryStaff => 3,
        }
    }

    fn glyph(self) -> &'static str {
        match self {
            Self::GovernmentLaws => "G",
            Self::ResearchProduction => "R",
            Self::MilitaryStaff => "M",
        }
    }
}

fn vanilla_idea_category_row(
    ui: &mut egui::Ui,
    rect: Rect,
    kind: IdeaCategoryRowKind,
    data: &PoliticsData,
    icon_bank: &mut crate::icons::IconBank,
    cmds: &mut Vec<DecisionCommand>,
) {
    paint_vanilla_idea_category_frame(ui, rect, kind, icon_bank);
    let content = Rect::from_min_max(
        Pos2::new(rect.left() + 92.0, rect.top() + 30.0),
        Pos2::new(rect.right() - 8.0, rect.bottom() - 8.0),
    );

    match kind {
        IdeaCategoryRowKind::GovernmentLaws => {
            vanilla_category_law_slots(ui, rect, data, cmds);
        }
        IdeaCategoryRowKind::ResearchProduction | IdeaCategoryRowKind::MilitaryStaff => {
            vanilla_category_advisor_slots(ui, content);
        }
    }
}

fn paint_vanilla_idea_category_frame(
    ui: &mut egui::Ui,
    rect: Rect,
    kind: IdeaCategoryRowKind,
    icon_bank: &mut crate::icons::IconBank,
) {
    ui.painter()
        .rect_filled(rect, 1.0, Color32::from_rgb(0x08, 0x09, 0x08));
    crate::v9::paint::paint_vertical_gradient_mesh(
        ui.painter(),
        rect.shrink(1.0),
        Color32::from_rgba_premultiplied(0x20, 0x22, 0x1d, 230),
        Color32::from_rgba_premultiplied(0x04, 0x05, 0x04, 252),
    );
    paint_vanilla_border(ui.painter(), rect);

    let header = Rect::from_min_size(rect.left_top(), Vec2::new(rect.width(), 25.0));
    if let Some(handle) = icon_bank.get_or_load("GFX_category_header") {
        ui.put(
            header,
            egui::Image::from_texture(handle).fit_to_exact_size(header.size()),
        );
        ui.painter()
            .rect_filled(header, 0.0, Color32::from_black_alpha(70));
    } else {
        VanillaIron::section_title_at(ui, header, "");
    }
    ui.painter().text(
        Pos2::new(header.left() + 92.0, header.center().y),
        egui::Align2::LEFT_CENTER,
        kind.title(),
        crate::v9::TextRole::Subheading.font_id(),
        VanillaIron::BRASS_BRIGHT,
    );

    let icon_rect = Rect::from_min_size(
        Pos2::new(rect.left() + 13.0, rect.top() + 31.0),
        Vec2::new(64.0, 56.0),
    );
    vanilla_slot(ui, icon_rect, vanilla_edge());
    if let Some(handle) = icon_bank.get_or_load("GFX_idea_categories") {
        ui.put(
            icon_rect.shrink(4.0),
            egui::Image::from_texture(handle).fit_to_exact_size(icon_rect.shrink(4.0).size()),
        );
        ui.painter()
            .rect_filled(icon_rect.shrink(4.0), 0.0, Color32::from_black_alpha(105));
    }
    ui.painter().text(
        icon_rect.center(),
        egui::Align2::CENTER_CENTER,
        kind.glyph(),
        crate::v9::TextRole::Heading.font_id(),
        vanilla_gold(),
    );
}

fn vanilla_category_spirit_slots(
    ui: &mut egui::Ui,
    rect: Rect,
    data: &PoliticsData,
    icon_bank: &mut crate::icons::IconBank,
) {
    let max_slots = 7usize;
    let spirits = national_spirit_ideas(data);
    if spirits.is_empty() {
        let empty = vanilla_idea_category_slot_rects(rect, 1)
            .into_iter()
            .next()
            .unwrap_or_else(|| Rect::from_min_size(rect.left_top(), Vec2::new(80.0, 64.0)));
        vanilla_slot(ui, empty, Color32::from_rgb(0x34, 0x35, 0x30));
        ui.painter().text(
            Pos2::new(empty.right() + 10.0, empty.center().y),
            egui::Align2::LEFT_CENTER,
            tr("idea_none"),
            crate::v9::TextRole::Body.font_id(),
            vanilla_muted(),
        );
        return;
    }

    let visible = spirits.len().min(max_slots);
    for (idea, slot_rect) in spirits
        .iter()
        .take(visible)
        .zip(vanilla_idea_category_slot_rects(rect, visible))
    {
        vanilla_slot(ui, slot_rect, vanilla_edge());
        if let Some(handle) = idea_icon_gfx(idea).and_then(|gfx| icon_bank.get_or_load(&gfx)) {
            ui.put(
                slot_rect.shrink(5.0),
                egui::Image::from_texture(handle).fit_to_exact_size(slot_rect.shrink(5.0).size()),
            );
        } else {
            let letter = idea
                .name
                .chars()
                .find(|c| !c.is_whitespace())
                .unwrap_or('?');
            ui.painter().text(
                slot_rect.center(),
                egui::Align2::CENTER_CENTER,
                letter.to_string(),
                crate::v9::TextRole::Subheading.font_id(),
                vanilla_gold(),
            );
        }
        ui.interact(
            slot_rect,
            ui.id().with(("vanilla_category_idea", &idea.key)),
            Sense::hover(),
        )
        .on_hover_ui(|ui| render_idea_tooltip(ui, idea));
    }

    if spirits.len() > visible {
        ui.painter().text(
            Pos2::new(rect.right() - 8.0, rect.bottom() - 32.0),
            egui::Align2::RIGHT_CENTER,
            format!("+{}", spirits.len() - visible),
            crate::v9::TextRole::Body.font_id(),
            vanilla_muted(),
        );
    }
}

fn vanilla_category_law_slots(
    ui: &mut egui::Ui,
    rect: Rect,
    data: &PoliticsData,
    cmds: &mut Vec<DecisionCommand>,
) {
    if data.law_slots.is_empty() {
        vanilla_empty_text(ui, rect, "No law data");
        return;
    }
    let slot_count = data.law_slots.len().min(7);
    for (law, slot_rect) in data
        .law_slots
        .iter()
        .take(slot_count)
        .zip(vanilla_idea_category_slot_rects(rect, slot_count))
    {
        let response = ui.interact(
            slot_rect,
            ui.id().with((
                "politics_law_detail_category_row",
                law_panel::law_category_key(law.category),
            )),
            Sense::click(),
        );
        if response.clicked() {
            cmds.push(politics_law_detail_command(law));
        }
        response.on_hover_text(politics_law_tooltip(law));
        vanilla_law_idea_slot(ui, slot_rect, law);
    }
}

fn politics_law_detail_command(law: &PoliticsLawEntry) -> DecisionCommand {
    DecisionCommand::Panel(PanelCommand::OpenDetail(ActiveDetailPanel::Law {
        category: law_panel::law_category_key(law.category).to_owned(),
        law_id: None,
    }))
}

fn vanilla_law_idea_slot(ui: &mut egui::Ui, rect: Rect, law: &PoliticsLawEntry) {
    vanilla_slot(ui, rect, politics_law_status_color(law));
    let icon_size = (rect.width() * 0.38).clamp(14.0, 25.0);
    let icon = Rect::from_min_size(
        rect.left_top() + Vec2::new(6.0, 5.0),
        Vec2::splat(icon_size),
    );
    draw_panel_svg_icon(
        ui,
        icon.shrink(4.0),
        PanelSvgIcon::from_law(&law.category),
        Color32::from_rgb(0xb7, 0xb1, 0x9a),
    );
    ui.painter().text(
        Pos2::new(rect.right() - 7.0, rect.top() + 7.0),
        egui::Align2::RIGHT_TOP,
        politics_law_category_glyph(&law.category),
        fit_text_font(
            politics_law_category_glyph(&law.category),
            crate::v9::TextRole::Caption.font_id(),
            (rect.width() - icon_size - 14.0).max(8.0),
        ),
        vanilla_muted(),
    );
    ui.painter().text(
        Pos2::new(rect.center().x, rect.bottom() - 16.0),
        egui::Align2::CENTER_CENTER,
        law.current_name.as_str(),
        fit_text_font(
            law.current_name.as_str(),
            crate::v9::TextRole::Caption.font_id(),
            rect.width() - 8.0,
        ),
        vanilla_text(),
    );
    let status = if law.is_locked {
        "LOCK"
    } else if law.pending.is_some() {
        "PEND"
    } else if law.cooldown_days > 0 {
        "CD"
    } else {
        "CUR"
    };
    ui.painter().text(
        Pos2::new(rect.right() - 5.0, rect.bottom() - 5.0),
        egui::Align2::RIGHT_BOTTOM,
        status,
        crate::v9::TextRole::Small.font_id(),
        politics_law_status_color(law),
    );
}

fn vanilla_category_advisor_slots(ui: &mut egui::Ui, rect: Rect) {
    let gap = 9.0;
    let slot_w = vanilla_slot_width_for_count(rect.width(), 5, gap, 52.0);
    let slot = Vec2::new(slot_w, 52.0);
    for idx in 0..5 {
        let slot_rect = Rect::from_min_size(
            Pos2::new(rect.left() + idx as f32 * (slot.x + gap), rect.top()),
            slot,
        );
        vanilla_slot(ui, slot_rect, Color32::from_rgb(0x34, 0x35, 0x30));
        let icon_pad = (slot_rect.width().min(slot_rect.height()) * 0.25).clamp(5.0, 13.0);
        draw_panel_svg_icon(
            ui,
            slot_rect.shrink(icon_pad),
            PanelSvgIcon::Person,
            vanilla_muted(),
        );
        ui.interact(
            slot_rect,
            ui.id().with(("politics_advisor_placeholder", idx)),
            Sense::hover(),
        )
        .on_hover_text("顾问席位暂未接入。");
    }
}

fn vanilla_section_title(ui: &mut egui::Ui, inner: Rect, title: &str) {
    let title_rect = Rect::from_min_size(inner.left_top(), Vec2::new(inner.width(), 26.0));
    VanillaIron::section_title_at(ui, title_rect, title);
}

fn vanilla_info_line(ui: &mut egui::Ui, pos: Pos2, label: &str, value: &str, value_color: Color32) {
    ui.painter().text(
        pos,
        egui::Align2::LEFT_TOP,
        label,
        crate::v9::TextRole::Body.font_id(),
        vanilla_muted(),
    );
    ui.painter().text(
        pos + Vec2::new(92.0, 0.0),
        egui::Align2::LEFT_TOP,
        value,
        crate::v9::TextRole::Body.font_id(),
        value_color,
    );
}

fn draw_vanilla_portrait(
    ui: &mut egui::Ui,
    rect: Rect,
    data: &PoliticsData,
    icon_bank: &mut crate::icons::IconBank,
) {
    vanilla_slot(ui, rect, v9_ideology_color(&data.ruling_party));
    let mut drawn = false;
    let image_rect = rect.shrink(2.0);
    if let Some(gfx) = data.leader_portrait_key.as_deref() {
        if let Some(handle) = icon_bank.get_or_load(gfx) {
            ui.put(
                image_rect,
                egui::Image::from_texture(handle).fit_to_exact_size(image_rect.size()),
            );
            drawn = true;
        }
    }
    if !drawn && !data.country_tag.is_empty() {
        let gfx = format!("GFX_flag_{}", data.country_tag);
        if let Some(handle) = icon_bank.get_or_load(&gfx) {
            ui.put(
                image_rect,
                egui::Image::from_texture(handle).fit_to_exact_size(image_rect.size()),
            );
            drawn = true;
        }
    }
    if !drawn {
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            if data.country_tag.is_empty() {
                "?"
            } else {
                data.country_tag.as_str()
            },
            crate::v9::TextRole::Heading.font_id(),
            v9_ideology_color(&data.ruling_party),
        );
    }
    ui.painter().rect_stroke(
        image_rect,
        egui::epaint::CornerRadius::same(0),
        egui::Stroke::new(1.0, Color32::from_black_alpha(230)),
        egui::epaint::StrokeKind::Inside,
    );
    ui.painter().rect_stroke(
        rect,
        egui::epaint::CornerRadius::same(1),
        egui::Stroke::new(1.0, v9_ideology_color(&data.ruling_party)),
        egui::epaint::StrokeKind::Inside,
    );
}

fn vanilla_person_row(
    ui: &mut egui::Ui,
    rect: Rect,
    office: &str,
    name: &str,
    detail: &str,
    accent: Color32,
) {
    vanilla_row_frame(ui, rect);
    let icon = Rect::from_min_size(rect.left_top() + Vec2::new(7.0, 5.0), Vec2::splat(27.0));
    vanilla_slot(ui, icon, accent);
    draw_panel_svg_icon(ui, icon.shrink(5.0), PanelSvgIcon::Person, vanilla_muted());
    let text_left = icon.right() + 10.0;
    let action = Rect::from_min_size(
        Pos2::new(rect.right() - 34.0, rect.top() + 4.0),
        Vec2::splat(29.0),
    );
    vanilla_icon_button(ui, action, PanelSvgIcon::Government, accent);
    ui.painter().text(
        Pos2::new(text_left, rect.top() + 3.0),
        egui::Align2::LEFT_TOP,
        office,
        crate::v9::TextRole::Caption.font_id(),
        vanilla_muted(),
    );
    ui.painter().text(
        Pos2::new(text_left, rect.top() + 18.0),
        egui::Align2::LEFT_TOP,
        name,
        fit_text_font(
            name,
            crate::v9::TextRole::Body.font_id(),
            action.left() - text_left - 58.0,
        ),
        vanilla_text(),
    );
    if !detail.is_empty() {
        ui.painter().text(
            Pos2::new(action.left() - 7.0, rect.center().y),
            egui::Align2::RIGHT_CENTER,
            detail,
            crate::v9::TextRole::Caption.font_id(),
            accent,
        );
    }
}

fn vanilla_law_row(ui: &mut egui::Ui, rect: Rect, slot: &PoliticsLawEntry, status: &str) {
    vanilla_row_frame(ui, rect);
    let icon = Rect::from_min_size(rect.left_top() + Vec2::new(7.0, 5.0), Vec2::splat(27.0));
    vanilla_slot(ui, icon, Color32::from_rgb(0x4d, 0x45, 0x35));
    draw_panel_svg_icon(
        ui,
        icon.shrink(5.0),
        PanelSvgIcon::from_law(&slot.category),
        Color32::from_rgb(0xb7, 0xb1, 0x9a),
    );
    let text_left = icon.right() + 10.0;
    let status_rect = Rect::from_min_size(
        Pos2::new(rect.right() - 73.0, rect.top() + 7.0),
        Vec2::new(66.0, 23.0),
    );
    ui.painter().text(
        Pos2::new(text_left, rect.top() + 3.0),
        egui::Align2::LEFT_TOP,
        politics_law_category_label(&slot.category),
        crate::v9::TextRole::Caption.font_id(),
        vanilla_muted(),
    );
    ui.painter().text(
        Pos2::new(text_left, rect.top() + 18.0),
        egui::Align2::LEFT_TOP,
        slot.current_name.as_str(),
        fit_text_font(
            slot.current_name.as_str(),
            crate::v9::TextRole::Body.font_id(),
            status_rect.left() - text_left - 8.0,
        ),
        vanilla_text(),
    );
    vanilla_status_pill(ui, status_rect, status, politics_law_status_color(slot));
}

fn vanilla_row_frame(ui: &mut egui::Ui, rect: Rect) {
    ui.painter()
        .rect_filled(rect, 1.0, Color32::from_rgb(0x12, 0x14, 0x12));
    crate::v9::paint::paint_vertical_gradient_mesh(
        ui.painter(),
        rect.shrink(1.0),
        Color32::from_rgba_premultiplied(0x25, 0x27, 0x22, 150),
        Color32::from_rgba_premultiplied(0x08, 0x09, 0x08, 235),
    );
    paint_vanilla_border(ui.painter(), rect);
    ui.painter().hline(
        (rect.left() + 4.0)..=(rect.right() - 4.0),
        rect.top() + 1.0,
        egui::Stroke::new(1.0, Color32::from_white_alpha(6)),
    );
}

fn vanilla_slot(ui: &mut egui::Ui, rect: Rect, accent: Color32) {
    let fill = Color32::from_rgb(
        0x10u8.saturating_add(accent.r() / 8),
        0x11u8.saturating_add(accent.g() / 8),
        0x0fu8.saturating_add(accent.b() / 8),
    );
    ui.painter().rect_filled(rect, 1.0, fill);
    crate::v9::paint::paint_vertical_gradient_mesh(
        ui.painter(),
        rect.shrink(1.0),
        Color32::from_rgba_premultiplied(accent.r(), accent.g(), accent.b(), 28),
        Color32::from_black_alpha(135),
    );
    ui.painter().rect_stroke(
        rect,
        egui::epaint::CornerRadius::same(1),
        egui::Stroke::new(1.0, accent),
        egui::epaint::StrokeKind::Inside,
    );
    ui.painter().rect_stroke(
        rect.shrink(3.0),
        egui::epaint::CornerRadius::same(0),
        egui::Stroke::new(1.0, Color32::from_black_alpha(210)),
        egui::epaint::StrokeKind::Inside,
    );
}

fn vanilla_icon_button(ui: &mut egui::Ui, rect: Rect, icon: PanelSvgIcon, accent: Color32) {
    ui.painter()
        .rect_filled(rect, 1.0, Color32::from_rgb(0x18, 0x19, 0x15));
    crate::v9::paint::paint_vertical_gradient_mesh(
        ui.painter(),
        rect.shrink(1.0),
        Color32::from_white_alpha(12),
        Color32::from_black_alpha(165),
    );
    paint_vanilla_border(ui.painter(), rect);
    draw_panel_svg_icon(ui, rect.shrink(6.0), icon, accent);
}

fn vanilla_status_pill(ui: &mut egui::Ui, rect: Rect, text: &str, color: Color32) {
    ui.painter()
        .rect_filled(rect, 1.0, Color32::from_rgb(0x0a, 0x0b, 0x09));
    ui.painter().rect_stroke(
        rect,
        egui::epaint::CornerRadius::same(1),
        egui::Stroke::new(1.0, Color32::from_black_alpha(220)),
        egui::epaint::StrokeKind::Inside,
    );
    ui.painter().rect_stroke(
        rect.shrink(1.0),
        egui::epaint::CornerRadius::same(0),
        egui::Stroke::new(1.0, color),
        egui::epaint::StrokeKind::Inside,
    );
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        text,
        fit_text_font(
            text,
            crate::v9::TextRole::Caption.font_id(),
            rect.width() - 6.0,
        ),
        color,
    );
}

#[derive(Clone, Copy)]
enum PanelSvgIcon {
    Person,
    Government,
    Helmet,
    Factory,
    Globe,
    Coin,
    Scales,
    Microphone,
}

impl PanelSvgIcon {
    fn from_law(category: &LawCategory) -> Self {
        match category {
            LawCategory::Conscription => Self::Helmet,
            LawCategory::Economy => Self::Factory,
            LawCategory::Trade => Self::Globe,
            LawCategory::Taxation => Self::Coin,
            LawCategory::CivilRights => Self::Scales,
            LawCategory::InformationControl => Self::Microphone,
        }
    }
}

fn draw_panel_svg_icon(ui: &mut egui::Ui, rect: Rect, icon: PanelSvgIcon, color: Color32) {
    let stroke = egui::Stroke::new(1.7, color);
    let thin = egui::Stroke::new(1.25, color);
    let p = |x: f32, y: f32| -> Pos2 {
        Pos2::new(
            rect.left() + rect.width() * x / 24.0,
            rect.top() + rect.height() * y / 24.0,
        )
    };
    let rr =
        |x: f32, y: f32, w: f32, h: f32| -> Rect { Rect::from_min_max(p(x, y), p(x + w, y + h)) };
    let painter = ui.painter();

    match icon {
        PanelSvgIcon::Person => {
            painter.circle_stroke(p(12.0, 7.5), rect.width() * 3.0 / 24.0, stroke);
            panel_svg_polyline(
                painter,
                &[
                    p(5.0, 21.0),
                    p(7.0, 15.0),
                    p(12.0, 12.5),
                    p(17.0, 15.0),
                    p(19.0, 21.0),
                ],
                stroke,
            );
            painter.line_segment([p(7.0, 21.0), p(17.0, 21.0)], thin);
        }
        PanelSvgIcon::Government => {
            panel_svg_polyline(painter, &[p(3.0, 9.0), p(12.0, 4.5), p(21.0, 9.0)], stroke);
            painter.line_segment([p(5.0, 10.0), p(19.0, 10.0)], stroke);
            for x in [7.0, 11.0, 15.0] {
                painter.line_segment([p(x, 11.0), p(x, 18.0)], thin);
            }
            painter.line_segment([p(5.0, 19.0), p(19.0, 19.0)], stroke);
            painter.line_segment([p(3.5, 21.0), p(20.5, 21.0)], thin);
        }
        PanelSvgIcon::Helmet => {
            panel_svg_polyline(
                painter,
                &[
                    p(4.0, 13.0),
                    p(6.0, 8.0),
                    p(12.0, 5.5),
                    p(18.0, 8.0),
                    p(20.0, 13.0),
                ],
                stroke,
            );
            painter.line_segment([p(4.0, 13.0), p(20.0, 13.0)], stroke);
            painter.line_segment([p(7.0, 16.5), p(17.0, 16.5)], thin);
            painter.line_segment([p(9.0, 20.0), p(15.0, 20.0)], thin);
        }
        PanelSvgIcon::Factory => {
            painter.rect_stroke(
                rr(4.0, 11.0, 16.0, 9.0),
                egui::epaint::CornerRadius::same(1),
                stroke,
                egui::epaint::StrokeKind::Inside,
            );
            panel_svg_polyline(
                painter,
                &[
                    p(4.0, 11.0),
                    p(8.0, 8.0),
                    p(12.0, 11.0),
                    p(16.0, 8.0),
                    p(20.0, 11.0),
                ],
                stroke,
            );
            painter.rect_stroke(
                rr(6.0, 5.0, 3.0, 6.0),
                egui::epaint::CornerRadius::same(0),
                thin,
                egui::epaint::StrokeKind::Inside,
            );
            for x in [8.0, 12.0, 16.0] {
                painter.line_segment([p(x, 15.0), p(x, 20.0)], thin);
            }
        }
        PanelSvgIcon::Globe => {
            painter.circle_stroke(p(12.0, 12.0), rect.width() * 7.0 / 24.0, stroke);
            painter.line_segment([p(5.0, 12.0), p(19.0, 12.0)], thin);
            painter.line_segment([p(12.0, 5.0), p(12.0, 19.0)], thin);
            painter.circle_stroke(p(12.0, 12.0), rect.width() * 3.8 / 24.0, thin);
        }
        PanelSvgIcon::Coin => {
            painter.circle_stroke(p(12.0, 12.0), rect.width() * 7.0 / 24.0, stroke);
            painter.line_segment([p(12.0, 6.0), p(12.0, 18.0)], thin);
            panel_svg_polyline(
                painter,
                &[
                    p(15.5, 8.5),
                    p(10.0, 8.5),
                    p(8.5, 10.5),
                    p(10.0, 12.0),
                    p(14.0, 12.0),
                    p(15.5, 13.5),
                    p(14.0, 15.5),
                    p(8.5, 15.5),
                ],
                thin,
            );
        }
        PanelSvgIcon::Scales => {
            painter.line_segment([p(12.0, 5.0), p(12.0, 20.0)], stroke);
            painter.line_segment([p(6.0, 8.0), p(18.0, 8.0)], stroke);
            painter.line_segment([p(8.0, 8.0), p(5.0, 15.0)], thin);
            painter.line_segment([p(8.0, 8.0), p(11.0, 15.0)], thin);
            painter.line_segment([p(16.0, 8.0), p(13.0, 15.0)], thin);
            painter.line_segment([p(16.0, 8.0), p(19.0, 15.0)], thin);
            panel_svg_polyline(
                painter,
                &[
                    p(4.5, 15.0),
                    p(11.5, 15.0),
                    p(10.0, 17.0),
                    p(6.0, 17.0),
                    p(4.5, 15.0),
                ],
                thin,
            );
            panel_svg_polyline(
                painter,
                &[
                    p(12.5, 15.0),
                    p(19.5, 15.0),
                    p(18.0, 17.0),
                    p(14.0, 17.0),
                    p(12.5, 15.0),
                ],
                thin,
            );
            painter.line_segment([p(8.0, 21.0), p(16.0, 21.0)], stroke);
        }
        PanelSvgIcon::Microphone => {
            painter.rect_stroke(
                rr(9.0, 4.5, 6.0, 10.0),
                egui::epaint::CornerRadius::same(3),
                stroke,
                egui::epaint::StrokeKind::Inside,
            );
            painter.line_segment([p(12.0, 14.5), p(12.0, 20.0)], stroke);
            painter.line_segment([p(8.0, 20.0), p(16.0, 20.0)], stroke);
            panel_svg_polyline(
                painter,
                &[
                    p(6.5, 11.0),
                    p(6.5, 14.0),
                    p(9.0, 17.0),
                    p(12.0, 17.5),
                    p(15.0, 17.0),
                    p(17.5, 14.0),
                    p(17.5, 11.0),
                ],
                thin,
            );
            painter.line_segment([p(10.5, 7.0), p(13.5, 7.0)], thin);
            painter.line_segment([p(10.5, 10.0), p(13.5, 10.0)], thin);
        }
    }
}

fn panel_svg_polyline(painter: &egui::Painter, points: &[Pos2], stroke: egui::Stroke) {
    painter.add(egui::Shape::line(points.to_vec(), stroke));
}

fn vanilla_action_button(ui: &mut egui::Ui, rect: Rect, label: &str) -> bool {
    let response = ui.interact(
        rect,
        ui.id().with(("vanilla_action", label)),
        Sense::click(),
    );
    ui.painter().rect_filled(
        rect,
        1.0,
        if response.hovered() {
            Color32::from_rgb(0x31, 0x32, 0x2a)
        } else {
            Color32::from_rgb(0x21, 0x22, 0x1c)
        },
    );
    paint_vanilla_border(ui.painter(), rect);
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        label,
        crate::v9::TextRole::Body.font_id(),
        vanilla_text(),
    );
    response.clicked()
}

fn draw_vanilla_ideology_donut(ui: &mut egui::Ui, rect: Rect, popularity: &[(String, f32)]) {
    let center = rect.center();
    let radius = rect.width().min(rect.height()) * 0.5;
    let total: f32 = popularity.iter().map(|(_, pop)| pop.max(0.0)).sum();
    if total <= f32::EPSILON {
        ui.painter()
            .circle_filled(center, radius, Color32::from_rgb(0x3d, 0x3d, 0x38));
        ui.painter()
            .circle_filled(center, radius * 0.56, vanilla_black());
        return;
    }
    let mut start = -std::f32::consts::FRAC_PI_2;
    for (key, pop) in popularity {
        let frac = (pop.max(0.0) / total).clamp(0.0, 1.0);
        if frac <= 0.0 {
            continue;
        }
        let sweep = frac * std::f32::consts::TAU;
        let steps = ((sweep / std::f32::consts::TAU) * 80.0).ceil().max(4.0) as usize;
        let mut points = Vec::with_capacity(steps + 2);
        points.push(center);
        for i in 0..=steps {
            let t = start + sweep * (i as f32 / steps as f32);
            points.push(Pos2::new(
                center.x + t.cos() * radius,
                center.y + t.sin() * radius,
            ));
        }
        ui.painter().add(egui::Shape::convex_polygon(
            points,
            v9_ideology_color(key),
            egui::Stroke::NONE,
        ));
        start += sweep;
    }
    ui.painter()
        .circle_filled(center, radius * 0.56, vanilla_black());
    ui.painter().circle_stroke(
        center,
        radius,
        egui::Stroke::new(2.0, Color32::from_black_alpha(220)),
    );
    ui.painter().circle_stroke(
        center,
        radius * 0.56,
        egui::Stroke::new(1.0, vanilla_edge()),
    );
}

fn vanilla_empty_text(ui: &mut egui::Ui, inner: Rect, text: &str) {
    ui.painter().text(
        inner.center(),
        egui::Align2::CENTER_CENTER,
        text,
        crate::v9::TextRole::Body.font_id(),
        vanilla_muted(),
    );
}

fn fit_text_font(text: &str, font: egui::FontId, max_width: f32) -> egui::FontId {
    crate::v9::text::fit_font_to_width(text, font, max_width, 0.60)
}

fn vanilla_gold() -> Color32 {
    VanillaIron::BRASS_BRIGHT
}

fn v9_leader_card(
    ui: &mut egui::Ui,
    data: &PoliticsData,
    icon_bank: &mut crate::icons::IconBank,
    cmds: &mut Vec<DecisionCommand>,
) {
    use crate::v9::{
        layout::{GridLayout, Track},
        primitives::{draw_progress_bar, Button, ButtonSize, ButtonVariant, PortraitFrame},
        tokens::{palette, spacing, TextRole},
    };
    v9_card(ui, 170.0, |ui, inner| {
        let grid = GridLayout::new(
            vec![Track::Fr(1.0)],
            vec![Track::Fixed(118.0), Track::Fr(1.0)],
        )
        .with_gutter(spacing::S5, 0.0);
        let cells = grid.measure(inner.shrink(2.0));
        let portrait_rect = GridLayout::cell(&cells, 0, 0).shrink2(Vec2::new(2.0, 10.0));
        let portrait = data
            .leader_portrait_key
            .as_deref()
            .and_then(|gfx| icon_bank.get_or_load(gfx));
        PortraitFrame::new(if data.country_tag.is_empty() {
            "?"
        } else {
            &data.country_tag
        })
        .accent(v9_ideology_color(&data.ruling_party))
        .show_at(ui, portrait_rect, portrait);

        let detail = GridLayout::cell(&cells, 0, 1).shrink(2.0);
        let leader = if data.leader_name.is_empty() {
            tr("leader_unknown").to_owned()
        } else {
            data.leader_name.clone()
        };
        let party = if data.party_full_name.is_empty() {
            ideology_label(&data.ruling_party).to_owned()
        } else {
            data.party_full_name.clone()
        };
        let leader_clip = Rect::from_min_max(
            detail.left_top(),
            Pos2::new(detail.right(), detail.top() + 56.0),
        );
        let leader_painter = ui.painter().with_clip_rect(leader_clip);
        let mut leader_font = TextRole::Display.font_id();
        let leader_w = leader.chars().count() as f32 * leader_font.size * 0.56;
        if leader_w > detail.width() {
            leader_font.size *= (detail.width() / leader_w).clamp(0.78, 1.0);
        }
        leader_painter.text(
            Pos2::new(detail.left(), detail.top() + 4.0),
            egui::Align2::LEFT_TOP,
            leader,
            leader_font,
            palette::GOLD_HOT,
        );
        let mut party_font = TextRole::Subheading.font_id();
        let party_w = party.chars().count() as f32 * party_font.size * 0.56;
        if party_w > detail.width() {
            party_font.size *= (detail.width() / party_w).clamp(0.72, 1.0);
        }
        leader_painter.text(
            Pos2::new(detail.left(), detail.top() + 32.0),
            egui::Align2::LEFT_TOP,
            party,
            party_font,
            v9_ideology_color(&data.ruling_party),
        );
        v9_badge(
            ui,
            Rect::from_min_size(
                Pos2::new(detail.left(), detail.top() + 58.0),
                Vec2::new(152.0, 24.0),
            ),
            ideology_label(&data.ruling_party),
            v9_ideology_color(&data.ruling_party),
        );
        v9_badge(
            ui,
            Rect::from_min_size(
                Pos2::new(detail.left() + 162.0, detail.top() + 58.0),
                Vec2::new(92.0, 24.0),
            ),
            if data.country_tag.is_empty() {
                "TAG"
            } else {
                &data.country_tag
            },
            palette::BRASS_BRIGHT,
        );

        let focus_label = data.current_focus_name.as_deref().unwrap_or("未选择国策");
        let focus_clip = Rect::from_min_max(
            Pos2::new(detail.left(), detail.top() + 90.0),
            Pos2::new(detail.right() - 132.0, detail.top() + 114.0),
        );
        ui.painter().with_clip_rect(focus_clip).text(
            Pos2::new(detail.left(), detail.top() + 96.0),
            egui::Align2::LEFT_TOP,
            focus_label,
            TextRole::Body.font_id(),
            palette::PARCHMENT,
        );
        let progress = data
            .current_focus_cost_days
            .filter(|days| *days > 0)
            .map(|days| (data.current_focus_progress / days as f32).clamp(0.0, 1.0))
            .unwrap_or(0.0);
        let bar = Rect::from_min_size(
            Pos2::new(detail.left(), detail.top() + 118.0),
            Vec2::new((detail.width() - 138.0).max(120.0), 10.0),
        );
        draw_progress_bar(ui, bar, progress, palette::GOLD);
        let btn_rect = Rect::from_min_size(
            Pos2::new(detail.right() - 124.0, detail.top() + 106.0),
            Vec2::new(120.0, 30.0),
        );
        if Button::new("国策")
            .size(ButtonSize::Md)
            .variant(ButtonVariant::Secondary)
            .enabled(data.focus_available)
            .show_at(ui, btn_rect)
            .clicked()
        {
            cmds.push(DecisionCommand::OpenFocusTree);
        }
    });
}

fn v9_ideology_card(ui: &mut egui::Ui, data: &PoliticsData) {
    use crate::v9::tokens::{palette, spacing, TextRole};
    let popularity = normalized_party_popularity(data);
    let height = (92.0 + popularity.len().max(1) as f32 * 25.0).max(188.0);
    v9_card(ui, height, |ui, inner| {
        ui.painter().text(
            Pos2::new(inner.left(), inner.top()),
            egui::Align2::LEFT_TOP,
            tr("party_popularity"),
            TextRole::Heading.font_id(),
            palette::BRASS_BRIGHT,
        );
        let content = Rect::from_min_max(
            Pos2::new(inner.left(), inner.top() + 34.0),
            inner.right_bottom(),
        );
        let donut_size = content
            .height()
            .min(content.width() * 0.42)
            .clamp(108.0, 148.0);
        let donut_rect = Rect::from_min_size(
            Pos2::new(
                content.left(),
                content.top() + (content.height() - donut_size) * 0.5,
            ),
            Vec2::splat(donut_size),
        );
        draw_v9_ideology_donut(ui, donut_rect, &popularity);

        let legend_left = donut_rect.right() + spacing::S5;
        let legend = Rect::from_min_max(
            Pos2::new(legend_left, content.top()),
            content.right_bottom(),
        );
        let mut y = legend.top();
        for (key, pop) in &popularity {
            let color = v9_ideology_color(key);
            let row =
                Rect::from_min_size(Pos2::new(legend.left(), y), Vec2::new(legend.width(), 22.0));
            ui.painter().rect_filled(
                Rect::from_min_size(
                    Pos2::new(row.left(), row.center().y - 5.0),
                    Vec2::splat(10.0),
                ),
                1.0,
                color,
            );
            let text_clip = Rect::from_min_max(
                Pos2::new(row.left() + 16.0, row.top()),
                Pos2::new(row.right() - 48.0, row.bottom()),
            );
            ui.painter().text(
                Pos2::new(text_clip.left(), row.center().y),
                egui::Align2::LEFT_CENTER,
                ideology_label(key),
                TextRole::Body.font_id(),
                palette::PARCHMENT,
            );
            ui.painter().text(
                Pos2::new(row.right(), row.center().y),
                egui::Align2::RIGHT_CENTER,
                format!("{:.0}%", pop * 100.0),
                TextRole::Numeric.font_id(),
                color,
            );
            let bar = Rect::from_min_size(
                Pos2::new(row.left() + 16.0, row.bottom() - 3.0),
                Vec2::new((row.width() - 64.0).max(32.0), 3.0),
            );
            ui.painter().rect_filled(bar, 1.0, palette::SOOT_BLACK);
            ui.painter().rect_filled(
                Rect::from_min_size(
                    bar.min,
                    Vec2::new(bar.width() * pop.clamp(0.0, 1.0), bar.height()),
                ),
                1.0,
                color,
            );
            y += spacing::S7;
        }
    });
}

fn draw_v9_ideology_donut(ui: &mut egui::Ui, rect: Rect, popularity: &[(String, f32)]) {
    use crate::v9::tokens::palette;

    let center = rect.center();
    let radius = rect.width().min(rect.height()) * 0.5;
    let total: f32 = popularity.iter().map(|(_, pop)| pop.max(0.0)).sum();
    if total <= f32::EPSILON {
        ui.painter().circle_filled(center, radius, palette::IRON);
        ui.painter()
            .circle_filled(center, radius * 0.55, palette::SOOT_BLACK);
        ui.painter()
            .circle_stroke(center, radius, egui::Stroke::new(1.0, palette::EDGE_DARK));
        return;
    }

    let mut start = -std::f32::consts::FRAC_PI_2;
    for (key, pop) in popularity {
        let frac = (pop.max(0.0) / total).clamp(0.0, 1.0);
        if frac <= 0.0 {
            continue;
        }
        let sweep = frac * std::f32::consts::TAU;
        let steps = ((sweep / std::f32::consts::TAU) * 80.0).ceil().max(4.0) as usize;
        let mut points = Vec::with_capacity(steps + 2);
        points.push(center);
        for i in 0..=steps {
            let t = start + sweep * (i as f32 / steps as f32);
            points.push(Pos2::new(
                center.x + t.cos() * radius,
                center.y + t.sin() * radius,
            ));
        }
        ui.painter().add(egui::Shape::convex_polygon(
            points,
            v9_ideology_color(key),
            egui::Stroke::new(0.0, Color32::TRANSPARENT),
        ));
        start += sweep;
    }
    ui.painter()
        .circle_filled(center, radius * 0.55, palette::SOOT_BLACK);
    ui.painter()
        .circle_stroke(center, radius, egui::Stroke::new(2.0, palette::EDGE_DARK));
    ui.painter().circle_stroke(
        center,
        radius * 0.55,
        egui::Stroke::new(1.0, palette::BRASS_DARK),
    );
}

fn v9_ideas_card(ui: &mut egui::Ui, data: &PoliticsData, icon_bank: &mut crate::icons::IconBank) {
    use crate::v9::{
        primitives::PortraitFrame,
        tokens::{palette, spacing, TextRole},
    };
    let content_w = (ui.available_width().max(360.0) - 32.0).max(IDEA_SLOT_SIZE);
    let cols = (((content_w + spacing::S4) / (54.0 + spacing::S4)).floor() as usize).clamp(1, 8);
    let spirits = national_spirit_ideas(data);
    let rows = spirits.len().max(1).div_ceil(cols);
    let height = 54.0 + rows as f32 * 64.0;
    v9_card(ui, height, |ui, inner| {
        ui.painter().text(
            Pos2::new(inner.left(), inner.top()),
            egui::Align2::LEFT_TOP,
            tr("national_spirits"),
            TextRole::Heading.font_id(),
            palette::BRASS_BRIGHT,
        );
        if spirits.is_empty() {
            ui.painter().text(
                inner.center(),
                egui::Align2::CENTER_CENTER,
                tr("idea_none"),
                TextRole::Body.font_id(),
                palette::MUTED,
            );
            return;
        }
        let slot = Vec2::splat(54.0);
        let gap = spacing::S4;
        let start = Pos2::new(inner.left(), inner.top() + 34.0);
        for (idx, idea) in spirits.iter().enumerate() {
            let col = idx % cols;
            let row = idx / cols;
            let rect = Rect::from_min_size(
                Pos2::new(
                    start.x + col as f32 * (slot.x + gap),
                    start.y + row as f32 * (slot.y + gap),
                ),
                slot,
            );
            let texture = idea_icon_gfx(idea).and_then(|gfx| icon_bank.get_or_load(&gfx));
            PortraitFrame::new(&idea.name)
                .accent(palette::BRASS_BRIGHT)
                .show_at(ui, rect, texture);
            let response = ui.interact(rect, ui.id().with(("v9_idea", &idea.key)), Sense::hover());
            response.on_hover_ui(|ui| render_idea_tooltip(ui, idea));
        }
    });
}

fn v9_government_card(ui: &mut egui::Ui, data: &PoliticsData) {
    use crate::v9::tokens::{palette, spacing, TextRole};

    let rows = data.government_posts.len().max(1);
    let height = 58.0 + rows as f32 * 48.0;
    v9_card(ui, height, |ui, inner| {
        ui.painter().text(
            Pos2::new(inner.left(), inner.top()),
            egui::Align2::LEFT_TOP,
            "政府",
            TextRole::Heading.font_id(),
            palette::BRASS_BRIGHT,
        );
        if data.government_posts.is_empty() {
            draw_v9_empty_text(ui, inner, "暂无政府职位数据");
            return;
        }
        let mut y = inner.top() + 34.0;
        for (idx, post) in data.government_posts.iter().enumerate() {
            let row =
                Rect::from_min_size(Pos2::new(inner.left(), y), Vec2::new(inner.width(), 40.0));
            let accent = if idx == 0 {
                v9_ideology_color(&data.ruling_party)
            } else {
                palette::BRASS_BRIGHT
            };
            draw_v9_detail_row(ui, row, &post.office, &post.name, &post.detail, accent);
            y += 40.0 + spacing::S3;
        }
    });
}

fn v9_law_systems_card(ui: &mut egui::Ui, data: &PoliticsData) {
    use crate::v9::tokens::{palette, spacing, TextRole};

    let rows = data.law_slots.len().max(1);
    let height = 58.0 + rows as f32 * 48.0;
    v9_card(ui, height, |ui, inner| {
        ui.painter().text(
            Pos2::new(inner.left(), inner.top()),
            egui::Align2::LEFT_TOP,
            "法律与制度",
            TextRole::Heading.font_id(),
            palette::BRASS_BRIGHT,
        );
        if data.law_slots.is_empty() {
            draw_v9_empty_text(ui, inner, "暂无法律制度数据");
            return;
        }
        let mut y = inner.top() + 34.0;
        for slot in &data.law_slots {
            let row =
                Rect::from_min_size(Pos2::new(inner.left(), y), Vec2::new(inner.width(), 40.0));
            let accent = politics_law_category_color(&slot.category);
            let status = politics_law_status_text(slot);
            draw_v9_law_row(ui, row, slot, &status, accent);
            y += 40.0 + spacing::S3;
        }
    });
}

fn draw_v9_detail_row(
    ui: &mut egui::Ui,
    rect: Rect,
    label: &str,
    value: &str,
    detail: &str,
    accent: Color32,
) {
    use crate::v9::tokens::{palette, spacing, TextRole};

    crate::v9::paint::paint_bevel(
        ui.painter(),
        rect,
        palette::SOOT_BLACK,
        palette::EDGE_DARK,
        2.0,
    );
    ui.painter().rect_filled(
        Rect::from_min_size(rect.left_top(), Vec2::new(3.0, rect.height())),
        0.0,
        accent,
    );
    let icon = Rect::from_min_size(
        Pos2::new(rect.left() + spacing::S3, rect.top() + spacing::S3),
        Vec2::splat(28.0),
    );
    crate::v9::paint::paint_bevel(ui.painter(), icon, palette::IRON_DARK, accent, 2.0);
    let glyph = label.chars().next().unwrap_or('?').to_string();
    ui.painter().text(
        icon.center(),
        egui::Align2::CENTER_CENTER,
        glyph,
        TextRole::Subheading.font_id(),
        accent,
    );
    let text = Rect::from_min_max(
        Pos2::new(icon.right() + spacing::S4, rect.top() + spacing::S2),
        Pos2::new(rect.right() - spacing::S4, rect.bottom() - spacing::S2),
    );
    let painter = ui.painter().with_clip_rect(text);
    painter.text(
        Pos2::new(text.left(), text.top()),
        egui::Align2::LEFT_TOP,
        label,
        TextRole::Caption.font_id(),
        palette::PARCHMENT_DIM,
    );
    painter.text(
        Pos2::new(text.left(), text.top() + 14.0),
        egui::Align2::LEFT_TOP,
        value,
        TextRole::Body.font_id(),
        palette::PARCHMENT,
    );
    if !detail.is_empty() {
        painter.text(
            Pos2::new(text.right(), text.top() + 14.0),
            egui::Align2::RIGHT_TOP,
            detail,
            TextRole::Caption.font_id(),
            accent,
        );
    }
}

fn draw_v9_law_row(
    ui: &mut egui::Ui,
    rect: Rect,
    slot: &PoliticsLawEntry,
    status: &str,
    accent: Color32,
) {
    use crate::v9::tokens::{palette, spacing, TextRole};

    crate::v9::paint::paint_bevel(
        ui.painter(),
        rect,
        palette::SOOT_BLACK,
        palette::EDGE_DARK,
        2.0,
    );
    ui.painter().rect_filled(
        Rect::from_min_size(rect.left_top(), Vec2::new(3.0, rect.height())),
        0.0,
        accent,
    );
    let icon = Rect::from_min_size(
        Pos2::new(rect.left() + spacing::S3, rect.top() + spacing::S3),
        Vec2::splat(28.0),
    );
    crate::v9::paint::paint_bevel(
        ui.painter(),
        icon,
        palette::IRON_DARK,
        palette::EDGE_DARK,
        2.0,
    );
    draw_panel_svg_icon(
        ui,
        icon.shrink(6.0),
        PanelSvgIcon::from_law(&slot.category),
        palette::PARCHMENT_DIM,
    );
    let text = Rect::from_min_max(
        Pos2::new(icon.right() + spacing::S4, rect.top() + spacing::S2),
        Pos2::new(rect.right() - spacing::S4, rect.bottom() - spacing::S2),
    );
    let painter = ui.painter().with_clip_rect(text);
    painter.text(
        Pos2::new(text.left(), text.top()),
        egui::Align2::LEFT_TOP,
        politics_law_category_label(&slot.category),
        TextRole::Caption.font_id(),
        palette::PARCHMENT_DIM,
    );
    painter.text(
        Pos2::new(text.left(), text.top() + 14.0),
        egui::Align2::LEFT_TOP,
        slot.current_name.as_str(),
        TextRole::Body.font_id(),
        palette::PARCHMENT,
    );
    painter.text(
        Pos2::new(text.right(), text.top() + 14.0),
        egui::Align2::RIGHT_TOP,
        status,
        TextRole::Caption.font_id(),
        politics_law_status_color(slot),
    );
}

fn draw_v9_empty_text(ui: &mut egui::Ui, inner: Rect, text: &str) {
    ui.painter().text(
        inner.center(),
        egui::Align2::CENTER_CENTER,
        text,
        crate::v9::TextRole::Body.font_id(),
        crate::v9::palette::MUTED,
    );
}

fn v9_advisors_card(ui: &mut egui::Ui) {
    use crate::v9::{
        primitives::PortraitFrame,
        tokens::{palette, spacing, TextRole},
    };
    v9_card(ui, 120.0, |ui, inner| {
        ui.painter().text(
            Pos2::new(inner.left(), inner.top()),
            egui::Align2::LEFT_TOP,
            tr("advisors"),
            TextRole::Heading.font_id(),
            palette::BRASS_BRIGHT,
        );
        let slot = Vec2::new(72.0, 72.0);
        let start = Pos2::new(inner.left(), inner.top() + 34.0);
        for idx in 0..5 {
            let rect = Rect::from_min_size(
                Pos2::new(start.x + idx as f32 * (slot.x + spacing::S5), start.y),
                slot,
            );
            PortraitFrame::new("?")
                .accent(palette::MUTED)
                .show_at(ui, rect, None);
        }
    });
}

fn v9_badge(ui: &mut egui::Ui, rect: Rect, text: &str, color: Color32) {
    crate::v9::paint::paint_bevel(
        ui.painter(),
        rect,
        crate::v9::palette::SOOT_BLACK,
        color,
        1.0,
    );
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        text,
        crate::v9::TextRole::Caption.font_id(),
        color,
    );
}

fn v9_ideology_color(key: &str) -> Color32 {
    hoi4_ideology_color(key)
}

fn v9_percent_color(value: f32) -> Color32 {
    if value >= 0.60 {
        crate::v9::palette::GOOD
    } else if value >= 0.35 {
        crate::v9::palette::WARN
    } else {
        crate::v9::palette::BAD
    }
}

fn politics_law_category_label(category: &LawCategory) -> &'static str {
    match category {
        LawCategory::Conscription => tr("v6_law_conscription"),
        LawCategory::Economy => tr("v6_law_economy"),
        LawCategory::Trade => tr("v6_law_trade"),
        LawCategory::Taxation => tr("v6_law_taxation"),
        LawCategory::CivilRights => tr("v6_law_civil_rights"),
        LawCategory::InformationControl => tr("v6_law_information_control"),
    }
}

fn politics_law_category_glyph(category: &LawCategory) -> &'static str {
    match category {
        LawCategory::Conscription => "C",
        LawCategory::Economy => "E",
        LawCategory::Trade => "T",
        LawCategory::Taxation => "$",
        LawCategory::CivilRights => "R",
        LawCategory::InformationControl => "I",
    }
}

fn politics_law_category_color(category: &LawCategory) -> Color32 {
    use crate::v9::palette;
    match category {
        LawCategory::Conscription => palette::IDEO_FASCISM,
        LawCategory::Economy => palette::INFO,
        LawCategory::Trade => palette::GOOD,
        LawCategory::Taxation => palette::GOLD,
        LawCategory::CivilRights => palette::COLD_ATOMIC,
        LawCategory::InformationControl => palette::BAD,
    }
}

fn politics_law_status_text(slot: &PoliticsLawEntry) -> String {
    if slot.is_locked {
        tr("locked").to_owned()
    } else if let Some((target, days)) = &slot.pending {
        format!("{}: {} / {}天", tr("v6_law_pending"), target, days)
    } else if slot.cooldown_days > 0 {
        format!("{} {}天", tr("cooldown"), slot.cooldown_days)
    } else {
        tr("current").to_owned()
    }
}

fn politics_law_status_color(slot: &PoliticsLawEntry) -> Color32 {
    use crate::v9::palette;
    if slot.is_locked {
        palette::BAD
    } else if slot.pending.is_some() || slot.cooldown_days > 0 {
        palette::WARN
    } else {
        palette::GOOD
    }
}

fn politics_law_tooltip(slot: &PoliticsLawEntry) -> String {
    let mut lines = vec![
        format!(
            "{}: {}",
            politics_law_category_label(&slot.category),
            slot.current_name
        ),
        "点击打开法律详情".to_owned(),
    ];
    if slot.is_locked {
        lines.push("状态：锁定".to_owned());
    }
    if let Some((target, days)) = &slot.pending {
        lines.push(format!("切换中：{target}，剩余 {days} 天"));
    }
    if slot.cooldown_days > 0 {
        lines.push(format!("冷却中：剩余 {} 天", slot.cooldown_days));
    }
    lines.join("\n")
}

fn politics_law_idea_sprite(slot: &PoliticsLawEntry) -> &'static str {
    if slot.current_id.is_empty() {
        law_panel::law_panel_law_sprite_from_name(slot.category, &slot.current_name)
    } else {
        law_panel::law_panel_law_sprite(slot.category, &slot.current_id)
    }
}

fn normalize_law_name_for_sprite(value: &str) -> String {
    value
        .chars()
        .flat_map(|c| c.to_lowercase())
        .filter(|c| c.is_ascii_alphanumeric())
        .collect()
}

fn render_politics_summary(ui: &mut egui::Ui, data: &PoliticsData) {
    let ruling_support = ruling_party_support(data);
    ui.add_space(6.0);
    components::summary_strip(
        ui,
        &[
            (
                tr("political_power"),
                format!("{:.0}", data.political_power),
            ),
            (tr("stability"), format!("{:.0}%", data.stability * 100.0)),
            (
                tr("war_support"),
                format!("{:.0}%", data.war_support * 100.0),
            ),
            ("执政支持", format!("{:.0}%", ruling_support * 100.0)),
        ],
    );
    ui.add_space(6.0);
}

fn render_politics_status_banner(ui: &mut egui::Ui, data: &PoliticsData) {
    let ruling_support = ruling_party_support(data);
    let (label, text, color) = if data.stability < 0.35 {
        (
            "政权不稳",
            "稳定度偏低，罢工、激进化和政治事件风险会上升。".to_owned(),
            BAD,
        )
    } else if data.war_support < 0.35 {
        (
            "战争支持不足",
            "战争支持偏低，动员和长期战争承压。".to_owned(),
            WARN,
        )
    } else if ruling_support < 0.40 {
        (
            "执政基础薄弱",
            "执政党支持率不足 40%，意识形态竞争正在削弱政权。".to_owned(),
            WARN,
        )
    } else {
        (
            "政局稳定",
            "当前政治局势可控，继续积累政治力量并观察局势变化。".to_owned(),
            GOOD,
        )
    };

    components::status_banner(ui, color, label, &text);
    ui.add_space(4.0);
}

fn render_leader_overview(
    ui: &mut egui::Ui,
    data: &PoliticsData,
    icon_bank: &mut crate::icons::IconBank,
    cmds: &mut Vec<DecisionCommand>,
) {
    politics_card(ui, "政权总览", |ui| {
        ui.horizontal(|ui| {
            render_leader_portrait(ui, data, icon_bank, Vec2::new(96.0, 96.0));
            ui.add_space(8.0);
            ui.vertical(|ui| {
                let leader_text = if data.leader_name.is_empty() {
                    tr("leader_unknown").to_string()
                } else {
                    data.leader_name.clone()
                };
                let party_text = if data.party_full_name.is_empty() {
                    ideology_label(&data.ruling_party).to_string()
                } else {
                    data.party_full_name.clone()
                };
                let ruling_color = ideology_color(&data.ruling_party);
                ui.label(
                    RichText::new(leader_text)
                        .heading()
                        .color(components::GOLD_BRIGHT),
                );
                ui.label(RichText::new(party_text).strong().color(ruling_color));
                ui.horizontal(|ui| {
                    ideology_chip(ui, &data.ruling_party, ruling_party_support(data));
                    if !data.country_tag.is_empty() {
                        ui.label(
                            RichText::new(&data.country_tag)
                                .small()
                                .color(components::MUTED),
                        );
                    }
                });
                render_current_focus_strip(ui, data, cmds);
                ui.add_space(6.0);
                ui.columns(3, |columns| {
                    metric_tile(
                        &mut columns[0],
                        tr("political_power"),
                        format!("{:.0}", data.political_power),
                        components::GOLD,
                    );
                    metric_tile(
                        &mut columns[1],
                        tr("stability"),
                        format!("{:.0}%", data.stability * 100.0),
                        percent_color(data.stability),
                    );
                    metric_tile(
                        &mut columns[2],
                        tr("war_support"),
                        format!("{:.0}%", data.war_support * 100.0),
                        percent_color(data.war_support),
                    );
                });
            });
        });
    });
}

fn render_current_focus_strip(
    ui: &mut egui::Ui,
    data: &PoliticsData,
    cmds: &mut Vec<DecisionCommand>,
) {
    if !data.focus_available {
        return;
    }
    ui.add_space(6.0);
    let progress = if let Some(cost_days) = data.current_focus_cost_days {
        if cost_days > 0 {
            (data.current_focus_progress / cost_days as f32).clamp(0.0, 1.0)
        } else {
            0.0
        }
    } else {
        0.0
    };
    let focus_label = data.current_focus_name.as_deref().unwrap_or("选择国策");
    let button_text = if data.current_focus_name.is_some() {
        format!("国策：{}  {:.0}%", focus_label, progress * 100.0)
    } else {
        "国策：未选择".to_owned()
    };

    egui::Frame::new()
        .fill(PANEL_CARD_SOFT)
        .stroke(egui::Stroke::new(1.0, components::GOLD))
        .inner_margin(egui::Margin::symmetric(8, 5))
        .show(ui, |ui| {
            ui.set_min_width(250.0);
            let response = ui.add(
                egui::Button::new(
                    RichText::new(button_text)
                        .small()
                        .color(components::GOLD_BRIGHT),
                )
                .min_size(egui::vec2(250.0, 24.0)),
            );
            if response.clicked() {
                cmds.push(DecisionCommand::OpenFocusTree);
            }
            if data.current_focus_name.is_some() {
                ui.add(
                    egui::ProgressBar::new(progress)
                        .fill(components::GOLD)
                        .desired_width(250.0),
                );
            } else {
                ui.label(
                    RichText::new("点击打开国策树")
                        .small()
                        .color(components::MUTED),
                );
            }
        });
}

fn render_ideology_section(ui: &mut egui::Ui, data: &PoliticsData) {
    politics_card(ui, tr("party_popularity"), |ui| {
        let popularity = normalized_party_popularity(data);
        ui.horizontal(|ui| {
            render_ideology_pie(ui, &popularity, 138.0);
            ui.add_space(12.0);
            ui.vertical(|ui| {
                for (key, pop) in &popularity {
                    render_ideology_popularity_row(ui, key, *pop);
                    let bar = egui::ProgressBar::new((*pop).clamp(0.0, 1.0))
                        .fill(ideology_color(key))
                        .desired_width(ui.available_width());
                    ui.add(bar);
                    ui.add_space(3.0);
                }
            });
        });
    });
}

fn render_ideas_section(
    ui: &mut egui::Ui,
    data: &PoliticsData,
    icon_bank: &mut crate::icons::IconBank,
) {
    politics_card(ui, tr("national_spirits"), |ui| {
        let spirits = national_spirit_ideas(data);
        if spirits.is_empty() {
            components::empty_state(ui, tr("idea_none"), "当前国家没有国家精神。 ");
        } else {
            let available_width = ui.available_width().max(IDEA_SLOT_SIZE);
            let spacing = 6.0;
            let columns = ((available_width + spacing) / (IDEA_SLOT_SIZE + spacing))
                .floor()
                .max(1.0) as usize;

            egui::Grid::new("politics_ideas_grid")
                .num_columns(columns)
                .spacing([spacing, spacing])
                .show(ui, |ui| {
                    for (index, idea) in spirits.iter().enumerate() {
                        render_idea_slot(ui, idea, icon_bank);
                        if index % columns == columns - 1 {
                            ui.end_row();
                        }
                    }
                });
        }
    });
}

fn render_idea_slot(ui: &mut egui::Ui, idea: &IdeaEntry, icon_bank: &mut crate::icons::IconBank) {
    let (rect, response) =
        ui.allocate_exact_size(Vec2::splat(IDEA_SLOT_SIZE), egui::Sense::hover());
    if ui.is_rect_visible(rect) {
        let border = if response.hovered() {
            components::GOLD_BRIGHT
        } else {
            STROKE_DARK
        };
        let painter = ui.painter();
        painter.rect_filled(rect, 3.0, Color32::from_rgb(0x16, 0x11, 0x0c));
        painter.rect_stroke(
            rect,
            3.0,
            egui::Stroke::new(1.2, border),
            egui::epaint::StrokeKind::Inside,
        );

        let icon_rect = egui::Rect::from_center_size(rect.center(), Vec2::splat(IDEA_ICON_SIZE));
        let mut icon_drawn = false;
        if let Some(gfx) = idea_icon_gfx(idea) {
            if let Some(handle) = icon_bank.get_or_load(&gfx) {
                ui.put(
                    icon_rect,
                    egui::Image::from_texture(handle)
                        .fit_to_exact_size(Vec2::splat(IDEA_ICON_SIZE)),
                );
                icon_drawn = true;
            }
        }
        if !icon_drawn {
            draw_idea_placeholder(ui, icon_rect, idea);
        }
    }

    response.on_hover_ui(|ui| render_idea_tooltip(ui, idea));
}

fn idea_icon_gfx(idea: &IdeaEntry) -> Option<String> {
    if let Some(picture) = idea.picture.as_deref().filter(|p| !p.is_empty()) {
        Some(if picture.starts_with("GFX_") {
            picture.to_owned()
        } else if picture.starts_with("idea_") {
            format!("GFX_{picture}")
        } else {
            format!("GFX_idea_{picture}")
        })
    } else if !idea.key.is_empty() {
        Some(format!("GFX_idea_{}", idea.key))
    } else {
        None
    }
}

fn draw_idea_placeholder(ui: &mut egui::Ui, rect: egui::Rect, idea: &IdeaEntry) {
    let painter = ui.painter();
    painter.rect_filled(rect, 2.0, PANEL_CARD_SOFT);
    painter.rect_stroke(
        rect,
        2.0,
        egui::Stroke::new(1.0, Color32::from_rgb(0x72, 0x58, 0x34)),
        egui::epaint::StrokeKind::Inside,
    );
    let letter = idea
        .name
        .chars()
        .find(|c| !c.is_whitespace())
        .unwrap_or('?')
        .to_string();
    painter.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        letter,
        egui::FontId::proportional(20.0),
        components::GOLD_BRIGHT,
    );
}

fn render_idea_tooltip(ui: &mut egui::Ui, idea: &IdeaEntry) {
    ui.set_max_width(300.0);
    ui.label(
        RichText::new(&idea.name)
            .strong()
            .color(components::GOLD_BRIGHT),
    );
    let category_key = format!("idea_category_{}", idea.category);
    let category_label = tr(&category_key);
    if category_label != category_key {
        ui.label(
            RichText::new(category_label)
                .small()
                .color(components::MUTED),
        );
    }
    ui.separator();
    if idea.modifiers.is_empty() {
        ui.label(
            RichText::new("暂无数据加成。")
                .small()
                .color(components::MUTED),
        );
    } else {
        ui.label(
            RichText::new(idea_modifier_summary(&idea.modifiers))
                .small()
                .color(Color32::from_rgb(0xd8, 0xc0, 0x8a)),
        );
    }
}

fn idea_modifier_summary(modifiers: &[(String, f32)]) -> String {
    modifiers
        .iter()
        .map(|(key, value)| match key.as_str() {
            "political_power_gain" => format!("政治力量 +{value:.2}/日"),
            "political_power_factor" | "political_power_gain_factor" => {
                format!("政治力量 {:+.0}%", value * 100.0)
            }
            "national_focus_progress" => format!("国策速度 {:+.0}%", value * 100.0),
            "production_speed_buildings_factor" => format!("建造速度 {:+.0}%", value * 100.0),
            "production_factory_max_efficiency_factor" => {
                format!("工厂效率上限 {:+.0}%", value * 100.0)
            }
            "research_speed_factor" => format!("科研速度 {:+.0}%", value * 100.0),
            "consumer_goods_factor" => format!("消费品工厂 {:+.0}%", value * 100.0),
            "army_attack_factor" => format!("陆军攻击 {:+.0}%", value * 100.0),
            "army_defence_factor" => format!("陆军防御 {:+.0}%", value * 100.0),
            "army_organisation_factor" => format!("陆军组织度 {:+.0}%", value * 100.0),
            "army_org_regain" => format!("组织恢复 {:+.0}%", value * 100.0),
            "reinforce_rate" => format!("增援率 {:+.0}%", value * 100.0),
            "front_demand_factor" => format!("前线需求 {:+.0}%", value * 100.0),
            "war_support_factor" => format!("战争支持度 {:+.0}%", value * 100.0),
            "attrition" => format!("损耗 {:+.0}%", value * 100.0),
            _ if value.fract().abs() < f32::EPSILON => format!("{key} {value:+.0}"),
            _ => format!("{key} {value:+.2}"),
        })
        .collect::<Vec<_>>()
        .join(" · ")
}

fn render_advisors_section(ui: &mut egui::Ui) {
    politics_card(ui, tr("advisors"), |ui| {
        ui.horizontal_wrapped(|ui| {
            for i in 0..5 {
                let (rect, response) =
                    ui.allocate_exact_size(Vec2::new(44.0, 44.0), egui::Sense::hover());
                ui.painter().rect_filled(rect, 4.0, PANEL_CARD_SOFT);
                ui.painter().rect_stroke(
                    rect,
                    4.0,
                    egui::Stroke::new(1.0, STROKE_DARK),
                    egui::epaint::StrokeKind::Outside,
                );
                ui.painter().text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    format!("{}", i + 1),
                    egui::FontId::proportional(13.0),
                    components::GOLD,
                );
                response.on_hover_text("顾问数据尚未接入：该槽位当前为空。");
            }
        });
        ui.label(
            RichText::new("顾问槽状态：暂无可任命顾问数据；不会消耗政治力量。")
                .small()
                .color(components::MUTED),
        );
    });
}

fn politics_card(ui: &mut egui::Ui, title: &str, add_contents: impl FnOnce(&mut egui::Ui)) {
    components::section_card(ui, title, add_contents);
}

fn metric_tile(ui: &mut egui::Ui, label: &str, value: String, color: Color32) {
    components::metric_tile(ui, label, value, color);
}

fn render_leader_portrait(
    ui: &mut egui::Ui,
    data: &PoliticsData,
    icon_bank: &mut crate::icons::IconBank,
    portrait_size: Vec2,
) {
    let mut portrait_drawn = false;
    if let Some(gfx) = data.leader_portrait_key.as_deref() {
        if let Some(handle) = icon_bank.get_or_load(gfx) {
            ui.add(
                egui::Image::from_texture(handle)
                    .fit_to_exact_size(portrait_size)
                    .corner_radius(2.0),
            );
            portrait_drawn = true;
        }
    }
    if portrait_drawn {
        return;
    }

    let flag_gfx = format!("GFX_flag_{}", data.country_tag);
    if !data.country_tag.is_empty() {
        if let Some(handle) = icon_bank.get_or_load(&flag_gfx) {
            ui.add(
                egui::Image::from_texture(handle)
                    .fit_to_exact_size(portrait_size)
                    .corner_radius(2.0),
            );
            return;
        }
    }

    let (rect, _) = ui.allocate_exact_size(portrait_size, egui::Sense::hover());
    ui.painter().rect_filled(rect, 2.0, Color32::from_gray(40));
    ui.painter().rect_stroke(
        rect,
        2.0,
        egui::Stroke::new(1.5, components::GOLD),
        egui::epaint::StrokeKind::Outside,
    );
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        if data.country_tag.is_empty() {
            "?"
        } else {
            data.country_tag.as_str()
        },
        egui::FontId::proportional(14.0),
        components::GOLD,
    );
}

fn render_ideology_pie(ui: &mut egui::Ui, popularity: &[(String, f32)], diameter: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(diameter), egui::Sense::hover());
    let center = rect.center();
    let radius = diameter * 0.5;
    let total: f32 = popularity.iter().map(|(_, pop)| pop.max(0.0)).sum();

    if total <= f32::EPSILON {
        ui.painter()
            .circle_filled(center, radius, Color32::from_rgb(70, 70, 70));
        ui.painter().circle_stroke(
            center,
            radius,
            egui::Stroke::new(2.0, Color32::from_rgb(40, 34, 26)),
        );
        return;
    }

    let mut start = -std::f32::consts::FRAC_PI_2;
    for (key, pop) in popularity {
        let frac = (pop.max(0.0) / total).clamp(0.0, 1.0);
        if frac <= 0.0 {
            continue;
        }
        let sweep = frac * std::f32::consts::TAU;
        let steps = ((sweep.abs() / std::f32::consts::TAU) * 72.0)
            .ceil()
            .max(3.0) as usize;
        let mut points = Vec::with_capacity(steps + 2);
        points.push(center);
        for i in 0..=steps {
            let t = start + sweep * (i as f32 / steps as f32);
            points.push(egui::pos2(
                center.x + t.cos() * radius,
                center.y + t.sin() * radius,
            ));
        }
        ui.painter().add(egui::Shape::convex_polygon(
            points,
            ideology_color(key),
            egui::Stroke::new(0.0, Color32::TRANSPARENT),
        ));
        start += sweep;
    }

    ui.painter().circle_stroke(
        center,
        radius,
        egui::Stroke::new(2.0, Color32::from_rgb(28, 22, 16)),
    );
    ui.painter().circle_stroke(
        center,
        radius - 3.0,
        egui::Stroke::new(1.0, Color32::from_black_alpha(120)),
    );
    ui.painter()
        .circle_filled(center, radius * 0.18, Color32::from_rgb(0x20, 0x18, 0x12));
    ui.painter().circle_stroke(
        center,
        radius * 0.18,
        egui::Stroke::new(1.0, components::GOLD),
    );
}

fn ideology_chip(ui: &mut egui::Ui, key: &str, support: f32) {
    let color = ideology_color(key);
    egui::Frame::new()
        .fill(Color32::from_rgba_premultiplied(
            color.r(),
            color.g(),
            color.b(),
            70,
        ))
        .stroke(egui::Stroke::new(1.0, color))
        .inner_margin(egui::Margin::symmetric(7, 3))
        .show(ui, |ui| {
            ui.label(
                RichText::new(format!("{} {:.0}%", ideology_label(key), support * 100.0))
                    .small()
                    .strong()
                    .color(color),
            );
        });
}

fn ruling_party_support(data: &PoliticsData) -> f32 {
    let Some(ruling) = canonical_ideology_key(&data.ruling_party) else {
        return 0.0;
    };
    normalized_party_popularity(data)
        .iter()
        .find(|(key, _)| key == ruling)
        .map(|(_, pop)| *pop)
        .unwrap_or(0.0)
        .clamp(0.0, 1.0)
}

fn percent_color(v: f32) -> Color32 {
    if v >= 0.60 {
        GOOD
    } else if v >= 0.40 {
        WARN
    } else {
        BAD
    }
}

fn render_ideology_popularity_row(ui: &mut egui::Ui, key: &str, pop: f32) {
    let color = ideology_color(key);
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(Vec2::new(22.0, 14.0), egui::Sense::hover());
        ui.painter().rect_filled(rect, 1.0, color);
        ui.painter().rect_stroke(
            rect,
            1.0,
            egui::Stroke::new(1.0, Color32::from_black_alpha(170)),
            egui::epaint::StrokeKind::Outside,
        );
        ui.label(RichText::new(ideology_label(key)).strong().color(color));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(
                RichText::new(format!("{:.0}%", pop * 100.0))
                    .strong()
                    .color(color),
            );
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: &str) -> DecisionEntry {
        DecisionEntry {
            id: id.into(),
            name: id.into(),
            description: String::new(),
            icon: String::new(),
            effect_preview: String::new(),
            category: DecisionCategory::Industry,
            mechanic_kind: DecisionMechanicKind::Standard,
            cost_political_power: 50.0,
            visible: true,
            clickable: true,
            mission_remaining: None,
            mission_total: None,
            cooldown_remaining: None,
            already_fired: false,
        }
    }

    fn law_entry(category: LawCategory) -> PoliticsLawEntry {
        PoliticsLawEntry {
            category,
            current_id: default_law_id_for_test(category).to_owned(),
            current_name: default_law_name_for_test(category).to_owned(),
            cooldown_days: 0,
            pending: None,
            is_locked: false,
        }
    }

    fn default_law_id_for_test(category: LawCategory) -> &'static str {
        match category {
            LawCategory::Conscription => "volunteer_only",
            LawCategory::Economy => "laissez_faire",
            LawCategory::Trade => "free_trade",
            LawCategory::Taxation => "medium_taxation",
            LawCategory::CivilRights => "limited_rights",
            LawCategory::InformationControl => "regulated_press",
        }
    }

    fn default_law_name_for_test(category: LawCategory) -> &'static str {
        match category {
            LawCategory::Conscription => "Volunteer Only",
            LawCategory::Economy => "Laissez Faire",
            LawCategory::Trade => "Free Trade",
            LawCategory::Taxation => "Medium Taxation",
            LawCategory::CivilRights => "Limited Rights",
            LawCategory::InformationControl => "Regulated Press",
        }
    }

    #[test]
    fn legacy_constructor_has_no_focus() {
        let d = PoliticsData::legacy("fascism".into(), vec![], vec![]);
        assert!(!d.focus_available);
        assert_eq!(d.political_power, 0.0);
    }

    #[test]
    fn entry_from_def_inherits_fields() {
        use hoi4_content::{Decision, DecisionCategory, Trigger};
        let d = Decision {
            id: "x".into(),
            name: "X".into(),
            description: "desc".into(),
            icon: "GFX_decision_gate19".into(),
            category: DecisionCategory::Military,
            mechanic_kind: DecisionMechanicKind::Standard,
            visible: Trigger::AlwaysTrue,
            available: Trigger::AlwaysTrue,
            cost_political_power: 75.0,
            days_mission_timeout: 0,
            days_re_enable: 0,
            fire_only_once: false,
            on_activation: vec![],
            on_complete: vec![],
            cancel_trigger: Trigger::AlwaysFalse,
            on_cancel: vec![],
        };
        let e = DecisionEntry::from_def(&d);
        assert_eq!(e.id, "x");
        assert_eq!(e.icon, "GFX_decision_gate19");
        assert_eq!(e.cost_political_power, 75.0);
        assert!(matches!(e.category, DecisionCategory::Military));
    }

    #[test]
    fn entry_status_flags_independent() {
        let mut e = entry("e");
        e.mission_remaining = Some(10);
        e.mission_total = Some(30);
        // 直接断字段：UI 渲染逻辑由 render_decision_status 消费这几个字段。
        assert_eq!(e.mission_remaining, Some(10));
        assert!(e.mission_total.is_some());
    }

    #[test]
    fn idea_modifier_summary_formats_known_modifiers() {
        let s = idea_modifier_summary(&[
            ("political_power_gain".to_owned(), 0.15),
            ("consumer_goods_factor".to_owned(), -0.10),
        ]);
        assert!(s.contains("政治力量 +0.15/日"));
        assert!(s.contains("消费品工厂 -10%"));
    }
}
#[cfg(test)]
mod gate8_12_tests {
    use super::*;
    use crate::vanilla_gui::{GuiAction, GuiActionKind, GuiNodePath, VanillaPanelProfile};
    use std::{cell::RefCell, rc::Rc};

    fn all_law_categories() -> [LawCategory; 6] {
        [
            LawCategory::Conscription,
            LawCategory::Economy,
            LawCategory::Trade,
            LawCategory::Taxation,
            LawCategory::CivilRights,
            LawCategory::InformationControl,
        ]
    }

    fn law_entry(category: LawCategory) -> PoliticsLawEntry {
        PoliticsLawEntry {
            category,
            current_id: default_law_id_for_test(category).to_owned(),
            current_name: default_law_name_for_test(category).to_owned(),
            cooldown_days: 0,
            pending: None,
            is_locked: false,
        }
    }

    fn default_law_id_for_test(category: LawCategory) -> &'static str {
        match category {
            LawCategory::Conscription => "volunteer_only",
            LawCategory::Economy => "laissez_faire",
            LawCategory::Trade => "free_trade",
            LawCategory::Taxation => "medium_taxation",
            LawCategory::CivilRights => "limited_rights",
            LawCategory::InformationControl => "regulated_press",
        }
    }

    fn default_law_name_for_test(category: LawCategory) -> &'static str {
        match category {
            LawCategory::Conscription => "Volunteer Only",
            LawCategory::Economy => "Laissez Faire",
            LawCategory::Trade => "Free Trade",
            LawCategory::Taxation => "Medium Taxation",
            LawCategory::CivilRights => "Limited Rights",
            LawCategory::InformationControl => "Regulated Press",
        }
    }

    fn gate10_politics_data() -> PoliticsData {
        let mut data = PoliticsData::legacy(
            "fascism".to_owned(),
            vec![
                ("fascism".to_owned(), 0.71),
                ("neutrality".to_owned(), 0.18),
                ("democratic".to_owned(), 0.08),
                ("communism".to_owned(), 0.03),
            ],
            vec![
                IdeaEntry {
                    key: "general_staff".to_owned(),
                    name: "General Staff".to_owned(),
                    category: "country".to_owned(),
                    picture: Some("general_staff".to_owned()),
                    modifiers: vec![("Factory Output".to_owned(), 0.05)],
                },
                IdeaEntry {
                    key: "autarkic_economy".to_owned(),
                    name: "Autarkic Economy".to_owned(),
                    category: "country".to_owned(),
                    picture: Some("autarkic_economy".to_owned()),
                    modifiers: vec![("Construction Speed".to_owned(), 0.10)],
                },
            ],
        );
        data.country_tag = "GER".to_owned();
        data.leader_name = "Adolf Hitler".to_owned();
        data.leader_portrait_key = Some("GFX_portrait_GER_adolf_hitler".to_owned());
        data.party_full_name = "Nationalsozialistische Deutsche Arbeiterpartei".to_owned();
        data.party_names = vec![
            ("fascism".to_owned(), "NSDAP".to_owned()),
            ("neutrality".to_owned(), "DNVP".to_owned()),
            ("democratic".to_owned(), "Zentrum".to_owned()),
            ("communism".to_owned(), "KPD".to_owned()),
        ];
        data.current_focus_name = Some("Rhineland".to_owned());
        data.current_focus_progress = 35.0;
        data.current_focus_cost_days = Some(70);
        data.law_slots = all_law_categories().into_iter().map(law_entry).collect();
        data
    }

    #[test]
    fn gate8_idea_category_rows_match_vanilla_density() {
        assert!(vanilla_idea_categories_height() >= 310.0);
        assert!(vanilla_idea_category_row_step() >= vanilla_idea_category_row_height());
        assert_eq!(
            IdeaCategoryRowKind::GovernmentLaws.title(),
            "Government / Laws"
        );
        assert_eq!(
            IdeaCategoryRowKind::ResearchProduction.title(),
            "Research & Production"
        );
        assert_eq!(IdeaCategoryRowKind::MilitaryStaff.title(), "Military Staff");
    }

    #[test]
    fn gate6_country_profile_declares_templates_and_dynamic_bindings() {
        let profile = CountryPoliticsProfile;
        assert_eq!(profile.profile_id(), COUNTRY_POLITICS_PROFILE_ID);
        assert_eq!(profile.root_template(), COUNTRY_POLITICS_ROOT);
        assert_eq!(profile.required_gui_files(), &[COUNTRY_POLITICS_GUI_FILE]);
        assert!(profile
            .template_instances()
            .iter()
            .any(|instance| instance.template_name == COUNTRY_POLITICS_IDEA_CATEGORY_TEMPLATE));
        assert!(profile
            .template_instances()
            .iter()
            .any(|instance| instance.template_name == COUNTRY_POLITICS_PARTY_TEMPLATE));

        let mut data = PoliticsData::legacy(
            "fascism".to_owned(),
            vec![("fascism".to_owned(), 0.7), ("democratic".to_owned(), 0.3)],
            vec![],
        );
        data.leader_name = "Adolf Hitler".to_owned();
        data.leader_portrait_key = Some("GFX_portrait_GER_adolf_hitler".to_owned());
        data.current_focus_name = Some("Four Year Plan".to_owned());
        data.current_focus_progress = 35.0;
        data.current_focus_cost_days = Some(70);
        data.law_slots = vec![law_entry(LawCategory::Conscription)];
        data.party_names = vec![("fascism".to_owned(), "NSDAP".to_owned())];

        let leader = profile.bind_node(
            &GuiNodePath::root(COUNTRY_POLITICS_ROOT).child("leader"),
            &data,
        );
        assert_eq!(
            leader.sprite.as_deref(),
            Some("GFX_portrait_GER_adolf_hitler")
        );
        let leader_name = profile.bind_node(
            &GuiNodePath::root(COUNTRY_POLITICS_ROOT).child("leader_name"),
            &data,
        );
        assert_eq!(leader_name.text.as_deref(), Some("Adolf Hitler"));

        let progress = profile.bind_node(
            &GuiNodePath::root(COUNTRY_POLITICS_ROOT)
                .child("active_goal")
                .child("progress"),
            &data,
        );
        assert_eq!(progress.progress, Some(0.5));

        let parties = profile.bind_node(
            &GuiNodePath::root(COUNTRY_POLITICS_ROOT).child("parties_grid"),
            &data,
        );
        assert_eq!(parties.instance_count, Some(2));
        let party_name = profile.bind_node(
            &GuiNodePath::root(COUNTRY_POLITICS_ROOT)
                .child("parties_grid")
                .child("political_party_info_entry[0]")
                .child("name"),
            &data,
        );
        assert_eq!(party_name.text.as_deref(), Some("NSDAP 70%"));
        assert_eq!(party_name.text_color, Some(vanilla_text()));
        let fallback_party_name = profile.bind_node(
            &GuiNodePath::root(COUNTRY_POLITICS_ROOT)
                .child("parties_grid")
                .child("political_party_info_entry[1]")
                .child("name"),
            &data,
        );
        let expected_fallback_party_name = format!("{} 30%", tr("democratic"));
        assert_eq!(
            fallback_party_name.text.as_deref(),
            Some(expected_fallback_party_name.as_str())
        );
        assert_eq!(fallback_party_name.text_color, Some(vanilla_text()));

        let party_color = profile.bind_node(
            &GuiNodePath::root(COUNTRY_POLITICS_ROOT)
                .child("parties_grid")
                .child("political_party_info_entry[0]")
                .child("color_block"),
            &data,
        );
        assert_eq!(party_color.tint, Some(ideology_color("fascism")));
        let party_fill = profile.bind_node(
            &GuiNodePath::root(COUNTRY_POLITICS_ROOT)
                .child("parties_grid")
                .child("political_party_info_entry[0]")
                .child("leading_pol_party_bg"),
            &data,
        );
        assert_eq!(party_fill.visible, Some(false));
        assert_eq!(party_fill.progress, None);
        assert_eq!(party_fill.tint, None);

        let hidden = profile.bind_node(
            &GuiNodePath::root(COUNTRY_POLITICS_ROOT).child("power_balance_button"),
            &data,
        );
        assert_eq!(hidden.visible, Some(false));

        let command = profile.handle_action(
            GuiAction {
                node_path: GuiNodePath::root(COUNTRY_POLITICS_ROOT).child("active_goal"),
                kind: GuiActionKind::Click,
            },
            &data,
        );
        assert!(matches!(command, Some(DecisionCommand::OpenFocusTree)));
    }

    #[test]
    fn gate6_empty_focus_hides_vanilla_goal_icon_and_progress_chrome() {
        let profile = CountryPoliticsProfile;
        let data = PoliticsData::legacy("fascism".to_owned(), vec![], vec![]);

        for name in ["goal_icon", "progress", "progress_frame", "focus_cost"] {
            let binding = profile.bind_node(
                &GuiNodePath::root(COUNTRY_POLITICS_ROOT)
                    .child("active_goal")
                    .child(name),
                &data,
            );
            assert_eq!(binding.visible, Some(false), "{name}");
        }

        let title = profile.bind_node(
            &GuiNodePath::root(COUNTRY_POLITICS_ROOT)
                .child("active_goal")
                .child("title"),
            &data,
        );
        assert_eq!(title.visible, None);
        assert!(!title.text.as_deref().unwrap_or_default().is_empty());

        let button = profile.bind_node(
            &GuiNodePath::root(COUNTRY_POLITICS_ROOT)
                .child("active_goal")
                .child("add_national_goal_button"),
            &data,
        );
        assert_eq!(
            button.click.as_ref().map(|click| click.command.as_str()),
            Some("open_focus_tree")
        );
        assert_eq!(button.visible, None);
    }

    #[test]
    fn gate6_party_pie_segments_ignore_unknown_gray_keys() {
        let data = PoliticsData::legacy(
            "fascism".to_owned(),
            vec![
                ("fascist".to_owned(), 0.7),
                ("democracy".to_owned(), 0.2),
                ("custom_gray_party".to_owned(), 0.1),
            ],
            vec![],
        );

        let segments = party_pie_segments(&data);

        assert_eq!(
            segments,
            vec![
                (0.7, ideology_color("fascism")),
                (0.2, ideology_color("democratic"))
            ]
        );
    }

    #[test]
    fn gate6_party_rows_and_pie_share_normalized_popularity() {
        let profile = CountryPoliticsProfile;
        let data = PoliticsData::legacy(
            "fascist".to_owned(),
            vec![
                ("fascist".to_owned(), 0.5),
                ("fascism".to_owned(), 0.25),
                ("democracy".to_owned(), 0.25),
                ("custom_gray_party".to_owned(), 0.1),
            ],
            vec![],
        );

        let parties = profile.bind_node(
            &GuiNodePath::root(COUNTRY_POLITICS_ROOT).child("parties_grid"),
            &data,
        );
        assert_eq!(parties.instance_count, Some(2));

        let first_name = profile.bind_node(
            &GuiNodePath::root(COUNTRY_POLITICS_ROOT)
                .child("parties_grid")
                .child("political_party_info_entry[0]")
                .child("name"),
            &data,
        );
        let expected_first_name = format!("{} 75%", tr("fascism"));
        assert_eq!(
            first_name.text.as_deref(),
            Some(expected_first_name.as_str())
        );

        let first_color = profile.bind_node(
            &GuiNodePath::root(COUNTRY_POLITICS_ROOT)
                .child("parties_grid")
                .child("political_party_info_entry[0]")
                .child("color_block"),
            &data,
        );
        assert_eq!(first_color.tint, Some(ideology_color("fascism")));

        let pie = profile.bind_node(
            &GuiNodePath::root(COUNTRY_POLITICS_ROOT).child("political_pie_chart"),
            &data,
        );
        assert_eq!(
            pie.pie_segments,
            vec![
                (0.75, ideology_color("fascism")),
                (0.25, ideology_color("democratic"))
            ]
        );
    }

    #[test]
    fn gate6_ideology_colors_match_vanilla_00_ideologies() {
        assert_eq!(ideology_color("fascism"), Color32::from_rgb(150, 75, 0));
        assert_eq!(ideology_color("democratic"), Color32::from_rgb(0, 0, 255));
        assert_eq!(ideology_color("communism"), Color32::from_rgb(255, 0, 0));
        assert_eq!(
            ideology_color("neutrality"),
            Color32::from_rgb(124, 124, 124)
        );
    }

    #[test]
    fn gate6_binding_specs_document_only_data_binding_values() {
        assert!(COUNTRY_POLITICS_BINDING_SPECS.len() >= 20);
        for spec in COUNTRY_POLITICS_BINDING_SPECS {
            assert!(spec.path_pattern.starts_with(COUNTRY_POLITICS_ROOT));
            assert!(!spec.values.is_empty(), "{spec:?}");
            assert!(!spec.reason.trim().is_empty(), "{spec:?}");
            for value in spec.values {
                assert!(
                    matches!(
                        value,
                        CountryPoliticsBindingValue::Visibility
                            | CountryPoliticsBindingValue::Text
                            | CountryPoliticsBindingValue::TextColor
                            | CountryPoliticsBindingValue::Sprite
                            | CountryPoliticsBindingValue::Tint
                            | CountryPoliticsBindingValue::Progress
                            | CountryPoliticsBindingValue::Pie
                            | CountryPoliticsBindingValue::Tooltip
                            | CountryPoliticsBindingValue::Command
                            | CountryPoliticsBindingValue::Instances
                    ),
                    "{spec:?}"
                );
            }
        }

        assert!(COUNTRY_POLITICS_BINDING_SPECS
            .iter()
            .any(|spec| spec.path_pattern.contains("active_goal.progress")
                && spec.values == [CountryPoliticsBindingValue::Progress]));
        assert!(COUNTRY_POLITICS_BINDING_SPECS.iter().any(|spec| {
            spec.path_pattern.contains("add_idea_button")
                && spec.values.contains(&CountryPoliticsBindingValue::Command)
        }));
    }

    #[test]
    fn gate6_profile_binds_law_and_spirit_slots_without_visual_overlay_state() {
        let profile = CountryPoliticsProfile;
        let mut data = PoliticsData::legacy(
            "fascism".to_owned(),
            vec![],
            vec![IdeaEntry {
                key: "spirit".to_owned(),
                name: "National Spirit".to_owned(),
                category: "national_spirit".to_owned(),
                picture: Some("GFX_idea_generic_political_reform".to_owned()),
                modifiers: vec![],
            }],
        );
        data.law_slots = vec![law_entry(LawCategory::Economy)];

        let law = profile.bind_node(
            &GuiNodePath::root(COUNTRY_POLITICS_ROOT)
                .child("idea_categories_grid")
                .child(format!("{COUNTRY_POLITICS_IDEA_CATEGORY_TEMPLATE}[0]"))
                .child("ideas_grid")
                .child("political_idea_entry[0]")
                .child("add_idea_button"),
            &data,
        );
        assert_eq!(
            law.sprite.as_deref(),
            Some(politics_law_idea_sprite(&data.law_slots[0]))
        );
        assert!(!law.tooltip.as_deref().unwrap_or_default().is_empty());
        assert_eq!(
            law.click.as_ref().map(|click| click.command.as_str()),
            Some("law:Economy")
        );
        assert_eq!(law.text, None);
        assert_eq!(law.progress, None);
        assert!(law.pie_segments.is_empty());

        let spirit = profile.bind_node(
            &GuiNodePath::root(COUNTRY_POLITICS_ROOT)
                .child("national_spirit")
                .child("spirit_grid")
                .child("political_idea_entry[0]")
                .child("add_idea_button"),
            &data,
        );
        assert_eq!(
            spirit.sprite.as_deref(),
            Some("GFX_idea_generic_political_reform")
        );
        assert_eq!(spirit.tooltip.as_deref(), Some("National Spirit"));
        assert!(spirit.click.is_none());
        assert_eq!(spirit.text, None);
        assert_eq!(spirit.progress, None);
        assert!(spirit.pie_segments.is_empty());
    }

    #[test]
    fn gate6_national_spirit_slots_exclude_vanilla_law_ideas() {
        let profile = CountryPoliticsProfile;
        let data = PoliticsData::legacy(
            "fascism".to_owned(),
            vec![],
            vec![
                IdeaEntry {
                    key: "sour_loser".to_owned(),
                    name: "Bitter Loser".to_owned(),
                    category: "country".to_owned(),
                    picture: None,
                    modifiers: vec![],
                },
                IdeaEntry {
                    key: "limited_exports".to_owned(),
                    name: "Limited Exports".to_owned(),
                    category: "trade_laws".to_owned(),
                    picture: None,
                    modifiers: vec![],
                },
                IdeaEntry {
                    key: "limited_conscription".to_owned(),
                    name: "Limited Conscription".to_owned(),
                    category: "mobilization_laws".to_owned(),
                    picture: None,
                    modifiers: vec![],
                },
                IdeaEntry {
                    key: "partial_economic_mobilisation".to_owned(),
                    name: "Partial Mobilization".to_owned(),
                    category: "economy".to_owned(),
                    picture: None,
                    modifiers: vec![],
                },
            ],
        );

        let spirit_grid = profile.bind_node(
            &GuiNodePath::root(COUNTRY_POLITICS_ROOT)
                .child("national_spirit")
                .child("spirit_grid"),
            &data,
        );
        assert_eq!(spirit_grid.instance_count, Some(1));
        assert_eq!(national_spirit_ideas(&data).len(), 1);
        assert_eq!(national_spirit_ideas(&data)[0].key, "sour_loser");

        let first_spirit = profile.bind_node(
            &GuiNodePath::root(COUNTRY_POLITICS_ROOT)
                .child("national_spirit")
                .child("spirit_grid")
                .child("political_idea_entry[0]")
                .child("add_idea_button"),
            &data,
        );
        assert_eq!(first_spirit.sprite.as_deref(), Some("GFX_idea_sour_loser"));

        let second_spirit = profile.bind_node(
            &GuiNodePath::root(COUNTRY_POLITICS_ROOT)
                .child("national_spirit")
                .child("spirit_grid")
                .child("political_idea_entry[1]")
                .child("add_idea_button"),
            &data,
        );
        assert!(second_spirit.sprite.is_none());
        assert_eq!(
            second_spirit.tooltip.as_deref(),
            Some(tr("national_spirits"))
        );
    }

    #[test]
    fn gate6_idea_picture_names_follow_vanilla_gfx_prefixes() {
        let bare_picture = IdeaEntry {
            key: "general_staff".to_owned(),
            name: "General Staff".to_owned(),
            category: "country".to_owned(),
            picture: Some("general_staff".to_owned()),
            modifiers: vec![],
        };
        assert_eq!(
            idea_icon_gfx(&bare_picture).as_deref(),
            Some("GFX_idea_general_staff")
        );

        let idea_prefixed_picture = IdeaEntry {
            key: "x".to_owned(),
            name: "Idea".to_owned(),
            category: "country".to_owned(),
            picture: Some("idea_limited_conscription".to_owned()),
            modifiers: vec![],
        };
        assert_eq!(
            idea_icon_gfx(&idea_prefixed_picture).as_deref(),
            Some("GFX_idea_limited_conscription")
        );
    }

    #[test]
    fn gate10_law_and_advisor_slots_stay_inside_small_rows() {
        let law_width = vanilla_slot_width_for_count(188.0, 6, 6.0, 66.0);
        let law_total = law_width * 6.0 + 6.0 * 5.0;
        assert!(law_total <= 188.0 + f32::EPSILON);

        let advisor_width = vanilla_slot_width_for_count(188.0, 5, 9.0, 52.0);
        let advisor_total = advisor_width * 5.0 + 9.0 * 4.0;
        assert!(advisor_total <= 188.0 + f32::EPSILON);

        let desktop_width = vanilla_slot_width_for_count(420.0, 6, 6.0, 66.0);
        assert_eq!(desktop_width, 65.0);
    }

    #[test]
    fn gate9_law_slot_statuses_cover_current_pending_cooldown_locked() {
        let current = law_entry(LawCategory::Conscription);
        assert_eq!(politics_law_status_text(&current), tr("current"));

        let mut pending = law_entry(LawCategory::Economy);
        pending.pending = Some(("War Economy".to_owned(), 12));
        assert!(politics_law_status_text(&pending).contains(tr("v6_law_pending")));
        assert!(politics_law_tooltip(&pending).contains("War Economy"));

        let mut cooldown = law_entry(LawCategory::Trade);
        cooldown.cooldown_days = 7;
        assert!(politics_law_status_text(&cooldown).contains(tr("cooldown")));
        assert!(politics_law_tooltip(&cooldown).contains("7"));

        let mut locked = law_entry(LawCategory::CivilRights);
        locked.is_locked = true;
        assert_eq!(politics_law_status_text(&locked), tr("locked"));
    }

    #[test]
    fn gate9_law_detail_command_keeps_category_and_no_specific_law() {
        let cmd = politics_law_detail_command(&law_entry(LawCategory::InformationControl));
        match cmd {
            DecisionCommand::Panel(PanelCommand::OpenDetail(ActiveDetailPanel::Law {
                category,
                law_id,
            })) => {
                assert_eq!(category, "InformationControl");
                assert!(law_id.is_none());
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn gate6_profile_preserves_close_and_focus_button_bindings() {
        let profile = CountryPoliticsProfile;
        let data = PoliticsData::legacy("fascism".to_owned(), vec![], vec![]);

        let close = profile.bind_node(
            &GuiNodePath::root(COUNTRY_POLITICS_ROOT).child("close_button"),
            &data,
        );
        assert_eq!(
            close.click.as_ref().map(|click| click.command.as_str()),
            Some("close")
        );

        let focus = profile.bind_node(
            &GuiNodePath::root(COUNTRY_POLITICS_ROOT).child("add_national_goal_button"),
            &data,
        );
        assert_eq!(
            focus.click.as_ref().map(|click| click.command.as_str()),
            Some("open_focus_tree")
        );
    }

    #[test]
    fn gate13_cjk_fit_font_shrinks_long_law_text_without_collapsing() {
        let base = crate::v9::TextRole::Caption.font_id();
        let fitted = fit_text_font("非常漫长的中文法律名称", base.clone(), 48.0);

        assert!(fitted.size < base.size);
        assert!(fitted.size >= base.size * 0.60);
    }

    #[test]
    fn gate14_six_law_categories_bind_to_non_overlapping_vanilla_slots() {
        let mut data = PoliticsData::legacy("fascism".to_owned(), vec![], vec![]);
        data.law_slots = all_law_categories().into_iter().map(law_entry).collect();

        let profile = CountryPoliticsProfile;
        let binding = profile.bind_node(
            &GuiNodePath::root(COUNTRY_POLITICS_ROOT)
                .child("idea_categories_grid")
                .child("ideas_grid"),
            &data,
        );
        assert_eq!(binding.instance_count, Some(6));

        let row = Rect::from_min_size(
            Pos2::ZERO,
            Vec2::new(550.0, vanilla_idea_category_row_height()),
        );
        let row_step = vanilla_idea_category_row_step();
        let slots = vanilla_idea_category_slot_rects(row, data.law_slots.len());
        assert_eq!(slots.len(), 6);
        for slot in &slots {
            assert!(slot.width() > 1.0);
            assert!(slot.height() > 1.0);
            assert!(slot.left() >= row.left() - f32::EPSILON);
            assert!(slot.right() <= row.right() + f32::EPSILON);
            assert!(slot.top() >= row.top() - f32::EPSILON);
            assert!(slot.bottom() <= row.top() + row_step + f32::EPSILON);
        }
        for pair in slots.windows(2) {
            assert!(pair[0].right() <= pair[1].left() + f32::EPSILON);
        }
    }

    #[test]
    fn gate14_idea_category_bridge_uses_template_width_not_grid_slot_width() {
        let Some(context) = politics_vanilla_gui_context() else {
            return;
        };
        let Some(root) = context.root_template(COUNTRY_POLITICS_GUI_FILE, COUNTRY_POLITICS_ROOT)
        else {
            return;
        };
        let Some(template) = context.root_template(
            COUNTRY_POLITICS_GUI_FILE,
            COUNTRY_POLITICS_IDEA_CATEGORY_TEMPLATE,
        ) else {
            return;
        };
        let Some(grid_node) = root.find_node_by_name("idea_categories_grid") else {
            return;
        };
        let viewport = crate::vanilla_gui::GuiRect::new(0.0, 0.0, 1920.0, 1080.0);
        let layout = crate::vanilla_gui::compute_layout_tree(
            root,
            &crate::vanilla_gui::LayoutOptions::new(viewport).shown_position(true),
        );
        let grid_layout = layout.find_by_name("idea_categories_grid").unwrap();
        let slot = crate::vanilla_gui::grid_slots(grid_node, grid_layout.rect, 1)
            .into_iter()
            .next()
            .unwrap();
        assert_eq!(slot.width, 64.0);

        let row_slot = crate::vanilla_gui::GuiRect::new(
            slot.x,
            slot.y,
            vanilla_idea_category_row_width(),
            vanilla_idea_category_row_height(),
        );
        let row_layout = crate::vanilla_gui::compute_layout_tree_with_path(
            template,
            &crate::vanilla_gui::LayoutOptions::new(row_slot),
            crate::vanilla_gui::GuiNodePath::root(COUNTRY_POLITICS_ROOT)
                .child("idea_categories_grid")
                .child(format!("{COUNTRY_POLITICS_IDEA_CATEGORY_TEMPLATE}[0]")),
        );

        assert_eq!(row_layout.rect.width, 550.0);
        assert_eq!(row_layout.rect.height, 100.0);
        let header = row_layout.find_by_name("category_header").unwrap();
        assert_eq!(header.rect.width, 0.0);
        assert_eq!(header.rect.x, row_layout.rect.x - 5.0);
    }

    #[test]
    fn gate1_politics_context_loads_gui_and_gfx_index_when_vanilla_available() {
        let Some(context) = politics_vanilla_gui_context() else {
            return;
        };
        assert!(context
            .root_template(COUNTRY_POLITICS_GUI_FILE, COUNTRY_POLITICS_ROOT)
            .is_some());
        assert!(!context.gfx_index.is_empty());
        let report = context.profile_report(&crate::vanilla_gui::COUNTRY_POLITICS_DESCRIPTOR);
        assert_eq!(report.gfx_hits.requested, POLITICS_PANEL_SPRITES.len());
        assert!(
            report.gfx_hits.hits >= 5,
            "unexpected missing politics GFX tokens: {:?}",
            report.gfx_hits.missing
        );
    }

    #[test]
    fn gate2_render_stats_commands_keep_close_and_focus_actions() {
        let mut stats = crate::vanilla_gui::RenderStats::default();
        stats.clicked_commands.push("open_focus_tree".to_owned());
        stats.clicked_commands.push("close".to_owned());
        let profile = CountryPoliticsProfile;
        let data = PoliticsData::legacy("fascism".to_owned(), vec![], vec![]);

        let commands = politics_commands_from_render_stats(&profile, &data, &stats);

        assert!(politics_close_requested_from_render_stats(&stats));
        assert!(matches!(
            commands.as_slice(),
            [DecisionCommand::OpenFocusTree]
        ));
    }

    #[test]
    fn gate3_profile_binds_dynamic_politics_nodes_after_tree_render() {
        let profile = CountryPoliticsProfile;
        let mut data = PoliticsData::legacy(
            "fascism".to_owned(),
            vec![
                ("fascism".to_owned(), 0.65),
                ("democratic".to_owned(), 0.35),
            ],
            vec![IdeaEntry {
                key: "spirit".to_owned(),
                name: "National Spirit".to_owned(),
                category: "national_spirit".to_owned(),
                picture: Some("GFX_idea_generic_political_reform".to_owned()),
                modifiers: vec![],
            }],
        );
        data.leader_name = "Leader".to_owned();
        data.current_focus_name = Some("Focus".to_owned());
        data.current_focus_progress = 10.0;
        data.current_focus_cost_days = Some(20);
        data.law_slots = all_law_categories().into_iter().map(law_entry).collect();

        let pie = profile.bind_node(
            &GuiNodePath::root(COUNTRY_POLITICS_ROOT).child("political_pie_chart"),
            &data,
        );
        let spirits = profile.bind_node(
            &GuiNodePath::root(COUNTRY_POLITICS_ROOT).child("spirit_grid"),
            &data,
        );
        let laws = profile.bind_node(
            &GuiNodePath::root(COUNTRY_POLITICS_ROOT)
                .child("idea_categories_grid")
                .child("ideas_grid"),
            &data,
        );

        assert_eq!(pie.pie_segments.len(), 2);
        assert_eq!(spirits.instance_count, Some(1));
        assert_eq!(laws.instance_count, Some(6));
    }

    #[test]
    fn gate3_national_spirit_bridge_renders_vanilla_slot_template() {
        let Some(context) = politics_vanilla_gui_context() else {
            return;
        };
        let Ok(path_cfg) = hoi4_paths::PathConfig::resolve(Default::default()) else {
            return;
        };
        let Some(root) = context.root_template(COUNTRY_POLITICS_GUI_FILE, COUNTRY_POLITICS_ROOT)
        else {
            return;
        };
        let viewport = crate::vanilla_gui::GuiRect::new(0.0, 0.0, 1920.0, 1080.0);
        let layout = crate::vanilla_gui::compute_layout_tree(
            root,
            &crate::vanilla_gui::LayoutOptions::new(viewport).shown_position(true),
        );
        let profile = CountryPoliticsProfile;
        let mut data = PoliticsData::legacy(
            "fascism".to_owned(),
            vec![],
            vec![
                IdeaEntry {
                    key: "spirit_a".to_owned(),
                    name: "Spirit A".to_owned(),
                    category: "country".to_owned(),
                    picture: Some("general_staff".to_owned()),
                    modifiers: vec![],
                },
                IdeaEntry {
                    key: "spirit_b".to_owned(),
                    name: "Spirit B".to_owned(),
                    category: "country".to_owned(),
                    picture: Some("autarkic_economy".to_owned()),
                    modifiers: vec![],
                },
            ],
        );
        data.law_slots = all_law_categories().into_iter().map(law_entry).collect();

        let stats_slot: Rc<RefCell<Option<crate::vanilla_gui::RenderStats>>> =
            Rc::new(RefCell::new(None));
        let stats_out = Rc::clone(&stats_slot);
        let mut harness = egui_kittest::Harness::builder()
            .with_size(Vec2::new(1920.0, 1080.0))
            .build(move |ctx| {
                let mut icon_bank = crate::icons::IconBank::new(ctx.clone(), path_cfg.clone());
                icon_bank.add_politics_search_dirs();
                egui::Area::new(egui::Id::new("gate3_spirit_bridge"))
                    .order(egui::Order::Foreground)
                    .fixed_pos(Pos2::ZERO)
                    .show(ctx, |ui| {
                        let renderer =
                            crate::vanilla_gui::VanillaGuiRenderer::new(&context.gfx_index);
                        let mut cmds = Vec::new();
                        let stats = paint_politics_national_spirit_template_instances(
                            ui,
                            &renderer,
                            context,
                            &profile,
                            &data,
                            &layout,
                            &mut icon_bank,
                            &mut cmds,
                        );
                        *stats_out.borrow_mut() = Some(stats);
                    });
            });

        harness.run();
        let stats = stats_slot.borrow().clone().unwrap_or_default();
        assert!(stats.nodes_seen >= 2, "{stats:?}");
        assert_eq!(stats.buttons, 2, "{stats:?}");
        assert!(stats.sprites_painted >= 2, "{stats:?}");
    }

    #[test]
    fn gate4_runtime_idea_category_slot_counts_follow_data() {
        let mut data = PoliticsData::legacy(
            "fascism".to_owned(),
            vec![],
            vec![
                IdeaEntry {
                    key: "a".to_owned(),
                    name: "A".to_owned(),
                    category: "national_spirit".to_owned(),
                    picture: None,
                    modifiers: vec![],
                },
                IdeaEntry {
                    key: "b".to_owned(),
                    name: "B".to_owned(),
                    category: "national_spirit".to_owned(),
                    picture: None,
                    modifiers: vec![],
                },
            ],
        );
        data.law_slots = all_law_categories().into_iter().map(law_entry).collect();

        assert_eq!(
            politics_idea_category_slot_count(IdeaCategoryRowKind::GovernmentLaws, &data),
            6
        );
        assert_eq!(
            politics_idea_category_slot_count(IdeaCategoryRowKind::ResearchProduction, &data),
            5
        );
        assert_eq!(
            politics_idea_category_slot_count(IdeaCategoryRowKind::MilitaryStaff, &data),
            6
        );
    }

    #[test]
    fn gate_harden_active_goal_layout() {
        let Some(context) = politics_vanilla_gui_context() else {
            return;
        };
        let root = context
            .root_template(COUNTRY_POLITICS_GUI_FILE, COUNTRY_POLITICS_ROOT)
            .unwrap();
        let button_resource = context
            .gfx_index
            .get("GFX_add_national_goal_button")
            .expect("GFX_add_national_goal_button should come from countrypoliticsview.gfx");
        assert_eq!(button_resource.frame_count, Some(3));

        for (width, height) in [(1920.0, 1080.0), (960.0, 640.0)] {
            let viewport = crate::vanilla_gui::GuiRect::new(0.0, 0.0, width, height);
            let layout = crate::vanilla_gui::compute_layout_tree(
                root,
                &crate::vanilla_gui::LayoutOptions::new(viewport).shown_position(true),
            );
            let active = layout.find_by_name("active_goal").unwrap();
            let bg = active.find_by_name("Background").unwrap();
            let button = active.find_by_name("add_national_goal_button").unwrap();
            let goal_icon = active.find_by_name("goal_icon").unwrap();
            let progress = active.find_by_name("progress").unwrap();
            let progress_frame = active.find_by_name("progress_frame").unwrap();
            let cost = active.find_by_name("focus_cost").unwrap();
            let drop_focus = active.find_by_name("drop_focus_button").unwrap();
            let drop_continuous = active.find_by_name("drop_continuous_focus_button").unwrap();

            assert_eq!(active.rect.x, 173.0, "viewport={width}x{height}");
            assert_eq!(active.rect.y, 126.0, "viewport={width}x{height}");
            assert_eq!(active.rect.width, 330.0);
            assert_eq!(active.rect.height, 50.0);
            assert_eq!(bg.rect, active.rect);
            assert_eq!(button.rect.x, active.rect.x + 100.0);
            assert_eq!(button.rect.y, active.rect.y + 7.0);
            assert_eq!(button.rect.width, 0.0);
            assert_eq!(button.rect.height, 0.0);

            let ctx = egui::Context::default();
            let Ok(path_cfg) = hoi4_paths::PathConfig::resolve(Default::default()) else {
                return;
            };
            let mut icon_bank = crate::icons::IconBank::new(ctx, path_cfg);
            icon_bank.add_politics_search_dirs();
            let hit = crate::vanilla_gui::resource_hit_rect(
                Rect::from(button.rect),
                "GFX_add_national_goal_button",
                Some(button_resource),
                &mut icon_bank,
            );
            assert!(hit.width() >= 250.0, "{hit:?}");
            assert!(hit.height() >= 80.0, "{hit:?}");
            assert_eq!(hit.min, Rect::from(button.rect).min);

            let goal_icon_center_x = goal_icon.rect.x + goal_icon.rect.width * 0.5;
            let goal_icon_center_y = goal_icon.rect.y + goal_icon.rect.height * 0.5;
            let active_center_y = active.rect.y + active.rect.height * 0.5;
            assert!(goal_icon_center_x < button.rect.x);
            assert!(goal_icon_center_y > active_center_y);
            assert_eq!(cost.rect.x, active.rect.x + 132.0);
            assert_eq!(progress.rect.x, active.rect.x + 112.0);
            assert_eq!(progress.rect.y, active.rect.y + 72.0);
            assert_eq!(progress_frame.rect.x, active.rect.x + 110.0);
            assert_eq!(progress_frame.rect.y, progress.rect.y);
            assert_eq!(drop_focus.rect.x, active.rect.x + 327.0 * 0.9);
            assert_eq!(drop_continuous.rect.x, drop_focus.rect.x);
        }
    }

    #[test]
    fn gate5_politics_icon_bank_diagnoses_key_sprites_when_vanilla_available() {
        let Ok(path_cfg) = hoi4_paths::PathConfig::resolve(Default::default()) else {
            return;
        };
        if path_cfg.find(COUNTRY_POLITICS_GUI_FILE).is_none() {
            return;
        }
        let ctx = egui::Context::default();
        let mut icon_bank = crate::icons::IconBank::new(ctx, path_cfg);
        icon_bank.add_politics_search_dirs();
        let (loaded, missing) = icon_bank.diagnose_sprites(POLITICS_PANEL_SPRITES.iter().copied());

        assert!(loaded >= 5, "loaded={loaded} missing={missing}");
        assert!(missing <= POLITICS_PANEL_SPRITES.len().saturating_sub(loaded));
        assert!(
            icon_bank.has_sprite_mapping("GFX_pol_view_bg"),
            "GFX_pol_view_bg must resolve through parsed .gfx metadata"
        );
    }

    #[test]
    fn gate9_viewport_layout_keeps_vanilla_root_and_key_regions_visible() {
        let Some(context) = politics_vanilla_gui_context() else {
            return;
        };
        let root = context
            .root_template(COUNTRY_POLITICS_GUI_FILE, COUNTRY_POLITICS_ROOT)
            .unwrap();

        for (width, height) in [(1920.0, 1080.0), (2560.0, 1440.0), (960.0, 640.0)] {
            let viewport = crate::vanilla_gui::GuiRect::new(0.0, 0.0, width, height);
            let layout = crate::vanilla_gui::compute_layout_tree(
                root,
                &crate::vanilla_gui::LayoutOptions::new(viewport).shown_position(true),
            );
            assert_eq!(layout.rect.x, -6.0);
            assert_eq!(layout.rect.y, 78.0);
            assert_eq!(layout.rect.width, 550.0);
            assert!(
                layout.rect.height >= height - 1.0,
                "root should keep vanilla percent-height at {width}x{height}: {:?}",
                layout.rect
            );

            for name in [
                "political_title",
                "leader_name",
                "active_goal",
                "national_spirit",
                "political_pie_chart",
                "ruling_party_info",
                "idea_categories_grid",
            ] {
                let node = layout
                    .find_by_name(name)
                    .unwrap_or_else(|| panic!("missing politics layout node {name}"));
                assert!(
                    node.rect.width > 0.0
                        && node.rect.height > 0.0
                        && node.rect.x + node.rect.width > 0.0
                        && node.rect.x < width
                        && node.rect.y + node.rect.height > 0.0
                        && node.rect.y < height,
                    "{name} should remain sized and inside or intersect the {width}x{height} viewport: {:?}",
                    node.rect
                );
            }

            for (name, sprite) in [
                ("Background", "GFX_tiled_plain_bg"),
                ("production_header_bg", "GFX_header_bg"),
                ("pol_view_bg", "GFX_pol_view_bg"),
                ("leader", "GFX_portrait_unknown"),
                ("close_button", "GFX_closebutton"),
            ] {
                let node = layout
                    .find_by_name(name)
                    .unwrap_or_else(|| panic!("missing politics sprite node {name}"));
                assert!(
                    node.rect.x >= -16.0
                        && node.rect.x < width
                        && node.rect.y >= 0.0
                        && node.rect.y < height,
                    "{name} should keep its vanilla anchor in the {width}x{height} viewport: {:?}",
                    node.rect
                );
                if !matches!(sprite, "GFX_portrait_unknown" | "GFX_closebutton") {
                    assert!(context.gfx_index.contains(sprite), "missing {sprite}");
                }
            }
        }
    }

    #[test]
    fn gate10_real_countrypoliticsview_tree_render_smoke_when_vanilla_available() {
        let Some(context) = politics_vanilla_gui_context() else {
            return;
        };
        let Ok(path_cfg) = hoi4_paths::PathConfig::resolve(Default::default()) else {
            return;
        };
        if path_cfg.find(COUNTRY_POLITICS_GUI_FILE).is_none() {
            return;
        }

        let root = context
            .root_template(COUNTRY_POLITICS_GUI_FILE, COUNTRY_POLITICS_ROOT)
            .unwrap();
        let viewport = crate::vanilla_gui::GuiRect::new(0.0, 0.0, 1920.0, 1080.0);
        let layout = crate::vanilla_gui::compute_layout_tree(
            root,
            &crate::vanilla_gui::LayoutOptions::new(viewport).shown_position(true),
        );
        let profile = CountryPoliticsProfile;
        let data = gate10_politics_data();
        let bindings = crate::vanilla_gui::bind_profile_tree(&profile, root, &data);
        let stats_slot: Rc<RefCell<Option<crate::vanilla_gui::RenderStats>>> =
            Rc::new(RefCell::new(None));
        let stats_out = Rc::clone(&stats_slot);

        let mut harness = egui_kittest::Harness::builder()
            .with_size(Vec2::new(1920.0, 1080.0))
            .build(move |ctx| {
                let mut icon_bank = crate::icons::IconBank::new(ctx.clone(), path_cfg.clone());
                icon_bank.add_politics_search_dirs();
                icon_bank.add_leader_dirs(["GER"]);
                egui::Area::new(egui::Id::new("gate10_real_politics_tree"))
                    .order(egui::Order::Foreground)
                    .fixed_pos(Pos2::ZERO)
                    .show(ctx, |ui| {
                        let _ = ui.allocate_exact_size(Vec2::new(1920.0, 1080.0), Sense::hover());
                        let renderer =
                            crate::vanilla_gui::VanillaGuiRenderer::new(&context.gfx_index);
                        let mut stats =
                            renderer.paint_tree(ui, root, &layout, &bindings, &mut icon_bank);
                        let mut cmds = Vec::new();
                        let bridge_stats = paint_politics_template_instance_bridge(
                            ui,
                            &renderer,
                            context,
                            &profile,
                            &data,
                            root,
                            &layout,
                            &mut icon_bank,
                            &mut cmds,
                        );
                        stats.merge(bridge_stats);
                        *stats_out.borrow_mut() = Some(stats);
                    });
            });

        harness.run();
        let stats = stats_slot
            .borrow()
            .clone()
            .expect("politics tree render stats should be captured");
        assert!(stats.nodes_seen >= 100, "{stats:?}");
        assert!(stats.nodes_painted >= 70, "{stats:?}");
        assert!(stats.sprites_painted >= 25, "{stats:?}");
        assert_eq!(stats.fallback_painted, 0, "{stats:?}");
        assert!(stats.text_painted >= 5, "{stats:?}");
        assert!(stats.buttons >= 15, "{stats:?}");
        assert_eq!(stats.progress_bars, 1, "{stats:?}");
        assert_eq!(stats.pie_charts, 1, "{stats:?}");
    }

    #[test]
    fn gate10_key_politics_gfx_textures_load_when_vanilla_available() {
        let Ok(path_cfg) = hoi4_paths::PathConfig::resolve(Default::default()) else {
            return;
        };
        if path_cfg.find(COUNTRY_POLITICS_GUI_FILE).is_none() {
            return;
        }
        let ctx = egui::Context::default();
        let mut icon_bank = crate::icons::IconBank::new(ctx, path_cfg);
        icon_bank.add_politics_search_dirs();

        for sprite in [
            "GFX_pol_view_bg",
            "GFX_pol_goal_bg",
            "GFX_pol_leader_frame",
            "GFX_category_header",
            "GFX_idea_categories",
        ] {
            let report = icon_bank.diagnose_sprite(sprite);
            assert!(
                report.loaded,
                "expected {sprite} to load from textureFile={:?}; tried={:?}; reason={:?}",
                report.texture_file, report.attempted_paths, report.failure_reason
            );
        }
    }

    #[test]
    fn gate_harden_resource_colors() {
        let Some(context) = politics_vanilla_gui_context() else {
            return;
        };
        let Ok(path_cfg) = hoi4_paths::PathConfig::resolve(Default::default()) else {
            return;
        };
        if path_cfg.find(COUNTRY_POLITICS_GUI_FILE).is_none() {
            return;
        }
        let ctx = egui::Context::default();
        let mut icon_bank = crate::icons::IconBank::new(ctx, path_cfg);
        icon_bank.add_politics_search_dirs();

        for sprite in [
            "GFX_pol_view_bg",
            "GFX_pol_goal_bg",
            "GFX_add_national_goal_button",
            "GFX_category_header",
            "GFX_idea_categories",
        ] {
            let resource = context
                .gfx_index
                .get(sprite)
                .unwrap_or_else(|| panic!("missing {sprite} from parsed .gfx index"));
            let texture_file = resource
                .fallback_texture_name()
                .unwrap_or_else(|| panic!("{sprite} has no textureFile"));
            assert_eq!(icon_bank.sprite_texturefile(sprite), Some(texture_file));
            let probe = icon_bank.diagnose_texture_file(texture_file);
            assert!(
                probe.loaded,
                "{sprite} textureFile probe failed: {:?}",
                probe
            );
            let stats = icon_bank
                .texture_file_pixel_stats(texture_file)
                .unwrap_or_else(|err| panic!("{sprite} pixel stats failed: {err}"));
            assert!(
                stats.non_transparent_pixels > 64,
                "{sprite} should have visible pixels: {:?}",
                stats
            );
            assert!(stats.max_alpha > 0, "{sprite} alpha should not be empty");
            assert!(
                stats
                    .mean_rgb
                    .iter()
                    .all(|channel| *channel >= 0.0 && *channel <= 255.0),
                "{sprite} mean RGB out of range: {:?}",
                stats
            );
            assert!(
                stats.near_white_gray_ratio < 0.98,
                "{sprite} looks like a white/gray placeholder: {:?}",
                stats
            );
        }
    }
}
