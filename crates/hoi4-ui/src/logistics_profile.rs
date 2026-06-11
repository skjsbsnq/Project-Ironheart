use crate::vanilla_gui::binding::{GuiAction, GuiActionKind, GuiBinding};
use crate::vanilla_gui::profile::{VanillaPanelProfile, VanillaTemplateInstance};
use crate::vanilla_gui::GuiNodePath;
use crate::vanilla_gui::{
    COUNTRY_LOGISTICS_GUI_FILE, COUNTRY_LOGISTICS_PROFILE_ID, COUNTRY_LOGISTICS_ROOT,
    LOGISTICS_REQUIRED_SPRITES,
};
use crate::{
    logistics_panel::{
        logistics_equipment_icon_sprite_for_id, LogisticsData, LogisticsEntry, ResourceEntry,
        ResourceInputEntry,
    },
    ActiveDetailPanel, DetailSource, GoodsDetailTarget, PanelCommand,
};
use egui::Color32;

pub struct CountryLogisticsProfile;

impl VanillaPanelProfile for CountryLogisticsProfile {
    type Data = LogisticsData;
    type Command = PanelCommand;

    fn profile_id(&self) -> &'static str {
        COUNTRY_LOGISTICS_PROFILE_ID
    }

    fn root_template(&self) -> &'static str {
        COUNTRY_LOGISTICS_ROOT
    }

    fn required_gui_files(&self) -> &'static [&'static str] {
        &[COUNTRY_LOGISTICS_GUI_FILE]
    }

    fn template_instances(&self) -> &'static [VanillaTemplateInstance] {
        &[]
    }

    fn required_sprites(&self) -> &'static [&'static str] {
        LOGISTICS_REQUIRED_SPRITES
    }

    fn key_templates(&self) -> &'static [&'static str] {
        &[
            "logistics_overview_land_equipment_entry",
            "logistics_overview_naval_equipment_entry",
            "logistics_overview_air_equipment_entry",
            "logistics_overview_resource_item",
            "logistics_entry_resource_item",
        ]
    }

    fn bind_node(&self, node_path: &GuiNodePath, data: &Self::Data) -> GuiBinding {
        let path_str = node_path.to_string();

        if path_str.ends_with("close_button") {
            return GuiBinding::default().click("close").tooltip("关闭后勤面板");
        }
        if path_str.ends_with("logistics_title") {
            return GuiBinding::default()
                .text("后勤")
                .tooltip(crate::logistics_panel::logistics_vanilla_summary_text(data));
        }
        if path_str.ends_with("viewing_flag") || path_str.ends_with("fuel_info") {
            return GuiBinding::default().visible(false);
        }
        if path_str.ends_with("resources_label") {
            return GuiBinding::default().visible(false);
        }
        if path_str.ends_with("equipment_type_label") {
            return GuiBinding::default().text("装备");
        }
        if path_str.ends_with("status_label") {
            return GuiBinding::default().text("供需");
        }
        if path_str.ends_with("military_factories.military_factories_usage") {
            return GuiBinding::default().progress(supply_ratio(data));
        }
        if path_str.ends_with("production_win_bottom.military_factories") {
            return GuiBinding::default()
                .text(format!("军购 {}", format_rm(data.military_procurement_rm)))
                .tooltip("项目军购预算汇总；不伪造 HOI4 原版军工厂数量");
        }
        if path_str.ends_with("production_win_bottom.naval_factories") {
            return GuiBinding::default()
                .text(format!("维护 {}", format_rm(data.military_maintenance_rm)))
                .tooltip(format!(
                    "项目维护预算；舰船/运输船日产 {:.1}/日",
                    naval_daily_production(data)
                ));
        }
        if path_str.ends_with("production_win_bottom.nuke")
            || path_str.ends_with("production_win_bottom.nuke.nuke_count")
        {
            return GuiBinding::default().visible(false).text("0");
        }

        GuiBinding::default()
    }

    fn bind_node_with_context(
        &self,
        node_path: &GuiNodePath,
        data: &Self::Data,
        instance: Option<&crate::vanilla_gui::GuiInstanceContext>,
    ) -> GuiBinding {
        let Some(instance) = instance else {
            return self.bind_node(node_path, data);
        };
        let name = node_path.0.last().map(String::as_str).unwrap_or_default();
        match instance.semantic_role.as_deref() {
            Some("logistics_resource_strip_item") => {
                let resource = instance
                    .model_key
                    .as_deref()
                    .and_then(|key| find_resource(data, key));
                return bind_resource_strip_node(name, resource);
            }
            Some("logistics_entry_resource_item") => {
                let input = instance
                    .model_key
                    .as_deref()
                    .and_then(|key| find_resource_input(data, key));
                return bind_entry_resource_node(name, input);
            }
            Some("logistics_equipment_entry") | _ => {}
        }

        let entry = instance
            .model_key
            .as_deref()
            .and_then(|key| find_entry(data, key));
        if name == "row_hit" {
            let key = entry
                .map(entry_key)
                .or_else(|| instance.model_key.clone())
                .unwrap_or_default();
            return GuiBinding::default()
                .click(format!("logistics:entry:{key}"))
                .tooltip("打开相关商品详情");
        }
        let Some(entry) = entry else {
            return self.bind_node(node_path, data);
        };
        match name {
            "equipment_type" => GuiBinding::default().text(&entry.name),
            "produced" => GuiBinding::default().text(format!("{:.1}", entry.daily_production)),
            "needs" => GuiBinding::default()
                .text(format!("{:.1}", entry.daily_consumption))
                .text_color(warn_color(entry.deficit > 0.0)),
            "balance" => GuiBinding::default()
                .text(format!("{:.1}", entry.net_change))
                .text_color(warn_color(entry.net_change < 0.0)),
            "in_stock" => GuiBinding::default().text(format!("{:.0}", entry.stockpile)),
            "status_progressbar" => {
                GuiBinding::default().progress(if entry.daily_consumption <= 0.0 {
                    1.0
                } else {
                    (entry.daily_production / entry.daily_consumption).clamp(0.0, 1.0)
                })
            }
            "resources_grid" => GuiBinding::default(),
            "equipment_icon" => entry
                .equipment_icon_sprite
                .as_deref()
                .or_else(|| logistics_equipment_icon_sprite_for_id(&entry.id))
                .map(|sprite| GuiBinding::default().sprite(sprite).visible(true))
                .unwrap_or_else(|| GuiBinding::default().visible(false)),
            _ if name.starts_with(instance.template_name.as_str()) => {
                GuiBinding::default().tooltip(entry_tooltip(entry))
            }
            _ => self.bind_node(node_path, data),
        }
    }

    fn handle_action(&self, action: GuiAction, _data: &Self::Data) -> Option<Self::Command> {
        match action.kind {
            GuiActionKind::Click => {
                let path = action.node_path.to_string();
                if path == "close" || path.ends_with("close_button") {
                    Some(PanelCommand::ClosePrimary)
                } else if let Some(good_id) = path.strip_prefix("logistics:entry:") {
                    Some(PanelCommand::OpenDetail(ActiveDetailPanel::Goods(
                        GoodsDetailTarget::from_source(good_id.to_owned(), DetailSource::Logistics),
                    )))
                } else {
                    None
                }
            }
            GuiActionKind::Hover => None,
        }
    }

    fn uses_slide_animation(&self) -> bool {
        true
    }
}

fn find_entry<'a>(data: &'a LogisticsData, key: &str) -> Option<&'a LogisticsEntry> {
    data.entries
        .iter()
        .find(|entry| entry.id == key || entry.name == key)
}

fn find_resource<'a>(data: &'a LogisticsData, key: &str) -> Option<&'a ResourceEntry> {
    data.resources.iter().find(|entry| entry.name == key)
}

fn find_resource_input<'a>(data: &'a LogisticsData, key: &str) -> Option<&'a ResourceInputEntry> {
    data.entries
        .iter()
        .flat_map(|entry| entry.resource_inputs.iter())
        .find(|entry| entry.id == key || entry.name == key)
}

fn entry_key(entry: &LogisticsEntry) -> String {
    if entry.id.is_empty() {
        entry.name.clone()
    } else {
        entry.id.clone()
    }
}

fn bind_resource_strip_node(name: &str, resource: Option<&ResourceEntry>) -> GuiBinding {
    let Some(resource) = resource else {
        return GuiBinding::default();
    };
    match name {
        "icon" => GuiBinding::default()
            .sprite("GFX_resources_strip")
            .frame(resource_frame(&resource.name)),
        "value" => GuiBinding::default().text(format!("{:.0}", resource.stored)),
        "button" => GuiBinding::default().tooltip(format!(
            "{} 产出 {:.1}/日 消耗 {:.1}/日 库存 {:.1}",
            resource.name, resource.produced, resource.consumed, resource.stored
        )),
        _ => GuiBinding::default(),
    }
}

fn bind_entry_resource_node(name: &str, input: Option<&ResourceInputEntry>) -> GuiBinding {
    let Some(input) = input else {
        return GuiBinding::default();
    };
    match name {
        "icon" => GuiBinding::default()
            .sprite("GFX_resources_strip")
            .frame(resource_frame(&input.id))
            .tooltip(format!("{} x{:.1}/日", input.name, input.amount)),
        _ => GuiBinding::default().tooltip(format!("{} x{:.1}/日", input.name, input.amount)),
    }
}

fn resource_frame(name: &str) -> u32 {
    match name {
        "oil" => 1,
        "steel" => 2,
        "aluminium" | "aluminum" => 3,
        "rubber" => 4,
        "tungsten" => 5,
        "chromium" => 6,
        _ => 1,
    }
}

fn entry_tooltip(entry: &LogisticsEntry) -> String {
    let mut lines = Vec::new();
    if entry.production_sources.is_empty() {
        lines.push("当前无有效军工产出".to_owned());
    } else {
        lines.push("生产来源".to_owned());
        lines.extend(entry.production_sources.iter().take(3).cloned());
    }
    lines.join("\n")
}

fn warn_color(warn: bool) -> Color32 {
    if warn {
        Color32::from_rgb(0xff, 0xc0, 0x60)
    } else {
        Color32::from_rgb(0xe6, 0xdf, 0xc8)
    }
}

fn supply_ratio(data: &LogisticsData) -> f32 {
    if data.total_daily_need <= 0.0 {
        1.0
    } else {
        (data.total_daily_production / data.total_daily_need).clamp(0.0, 1.0)
    }
}

fn naval_daily_production(data: &LogisticsData) -> f32 {
    data.entries
        .iter()
        .filter(|entry| {
            matches!(entry.id.as_str(), "naval_vessel" | "convoy")
                || matches!(entry.name.as_str(), "舰艇" | "运输船")
        })
        .map(|entry| entry.daily_production)
        .sum()
}

fn format_rm(value: f64) -> String {
    if value.abs() >= 1_000_000.0 {
        format!("{:.1}M RM/日", value / 1_000_000.0)
    } else if value.abs() >= 1_000.0 {
        format!("{:.1}K RM/日", value / 1_000.0)
    } else {
        format!("{:.0} RM/日", value)
    }
}
