use crate::finance_panel::{FinanceCommand, FinancePanelData};
use crate::vanilla_gui::binding::{GuiAction, GuiActionKind, GuiBinding};
use crate::vanilla_gui::profile::{VanillaPanelProfile, VanillaTemplateInstance};
use crate::vanilla_gui::GuiNodePath;
use crate::vanilla_gui::{
    COUNTRY_FINANCE_GUI_FILE, COUNTRY_FINANCE_PROFILE_ID, COUNTRY_FINANCE_ROOT,
    FINANCE_KEY_TEMPLATES, FINANCE_REQUIRED_SPRITES,
};
use crate::PanelCommand;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FinanceTab {
    Overview,
    Budget,
    Debt,
    Exchange,
    Economy,
    Funding,
}

impl FinanceTab {
    pub const ALL: [Self; 6] = [
        Self::Overview,
        Self::Budget,
        Self::Debt,
        Self::Exchange,
        Self::Economy,
        Self::Funding,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Self::Overview => "overview",
            Self::Budget => "budget",
            Self::Debt => "debt",
            Self::Exchange => "exchange",
            Self::Economy => "economy",
            Self::Funding => "funding",
        }
    }

    pub fn from_id(value: &str) -> Option<Self> {
        match value {
            "overview" => Some(Self::Overview),
            "budget" => Some(Self::Budget),
            "debt" => Some(Self::Debt),
            "exchange" => Some(Self::Exchange),
            "economy" => Some(Self::Economy),
            "funding" => Some(Self::Funding),
            _ => None,
        }
    }
}

impl Default for FinanceTab {
    fn default() -> Self {
        Self::Overview
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FinanceSector {
    Primary,
    Secondary,
    Tertiary,
}

impl FinanceSector {
    pub const ALL: [Self; 3] = [Self::Primary, Self::Secondary, Self::Tertiary];

    pub fn id(self) -> &'static str {
        match self {
            Self::Primary => "primary",
            Self::Secondary => "secondary",
            Self::Tertiary => "tertiary",
        }
    }

    pub fn from_id(value: &str) -> Option<Self> {
        match value {
            "primary" => Some(Self::Primary),
            "secondary" => Some(Self::Secondary),
            "tertiary" => Some(Self::Tertiary),
            _ => None,
        }
    }
}

impl Default for FinanceSector {
    fn default() -> Self {
        Self::Primary
    }
}

#[derive(Debug, Clone)]
pub struct FinanceVanillaInput {
    pub data: FinancePanelData,
    pub tab: FinanceTab,
    pub sector: FinanceSector,
}

impl FinanceVanillaInput {
    pub fn new(data: FinancePanelData, tab: FinanceTab, sector: FinanceSector) -> Self {
        Self { data, tab, sector }
    }
}

pub fn finance_tab_persisted_id() -> egui::Id {
    egui::Id::new("countryfinanceview_tab")
}

pub fn finance_sector_persisted_id() -> egui::Id {
    egui::Id::new("countryfinanceview_sector")
}

pub fn persisted_finance_tab(ctx: &egui::Context) -> FinanceTab {
    ctx.data_mut(|data| {
        data.get_persisted::<FinanceTab>(finance_tab_persisted_id())
            .unwrap_or_default()
    })
}

pub fn set_persisted_finance_tab(ctx: &egui::Context, tab: FinanceTab) {
    ctx.data_mut(|data| data.insert_persisted(finance_tab_persisted_id(), tab));
}

pub fn persisted_finance_sector(ctx: &egui::Context) -> FinanceSector {
    ctx.data_mut(|data| {
        data.get_persisted::<FinanceSector>(finance_sector_persisted_id())
            .unwrap_or_default()
    })
}

pub fn set_persisted_finance_sector(ctx: &egui::Context, sector: FinanceSector) {
    ctx.data_mut(|data| data.insert_persisted(finance_sector_persisted_id(), sector));
}

pub struct FinanceVanillaProfile;

impl VanillaPanelProfile for FinanceVanillaProfile {
    type Data = FinanceVanillaInput;
    type Command = FinanceCommand;

    fn profile_id(&self) -> &'static str {
        COUNTRY_FINANCE_PROFILE_ID
    }

    fn root_template(&self) -> &'static str {
        COUNTRY_FINANCE_ROOT
    }

    fn required_gui_files(&self) -> &'static [&'static str] {
        &[COUNTRY_FINANCE_GUI_FILE]
    }

    fn template_instances(&self) -> &'static [VanillaTemplateInstance] {
        &[]
    }

    fn required_sprites(&self) -> &'static [&'static str] {
        FINANCE_REQUIRED_SPRITES
    }

    fn key_templates(&self) -> &'static [&'static str] {
        FINANCE_KEY_TEMPLATES
    }

    fn bind_node(&self, _node_path: &GuiNodePath, _data: &Self::Data) -> GuiBinding {
        GuiBinding::default()
    }

    fn handle_action(&self, action: GuiAction, _data: &Self::Data) -> Option<Self::Command> {
        match action.kind {
            GuiActionKind::Click => {
                let path = action.node_path.to_string();
                if path == "close" || path.ends_with("close_button") {
                    Some(FinanceCommand::Panel(PanelCommand::ClosePrimary))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vanilla_gui::{GuiAction, GuiActionKind, VanillaPanelProfile};

    fn sample_input() -> FinanceVanillaInput {
        FinanceVanillaInput::new(
            FinancePanelData::default(),
            FinanceTab::Overview,
            FinanceSector::Primary,
        )
    }

    #[test]
    fn finance_tab_ids_round_trip() {
        for tab in FinanceTab::ALL {
            assert_eq!(FinanceTab::from_id(tab.id()), Some(tab));
        }
        assert_eq!(FinanceTab::from_id("missing"), None);
        assert_eq!(FinanceTab::default(), FinanceTab::Overview);
    }

    #[test]
    fn finance_sector_ids_round_trip() {
        for sector in FinanceSector::ALL {
            assert_eq!(FinanceSector::from_id(sector.id()), Some(sector));
        }
        assert_eq!(FinanceSector::from_id("missing"), None);
        assert_eq!(FinanceSector::default(), FinanceSector::Primary);
    }

    #[test]
    fn finance_tab_persisted_helpers_default_to_overview() {
        let ctx = egui::Context::default();

        assert_eq!(persisted_finance_tab(&ctx), FinanceTab::Overview);
        set_persisted_finance_tab(&ctx, FinanceTab::Debt);
        assert_eq!(persisted_finance_tab(&ctx), FinanceTab::Debt);
    }

    #[test]
    fn finance_sector_persisted_helpers_default_to_primary() {
        let ctx = egui::Context::default();

        assert_eq!(persisted_finance_sector(&ctx), FinanceSector::Primary);
        set_persisted_finance_sector(&ctx, FinanceSector::Tertiary);
        assert_eq!(persisted_finance_sector(&ctx), FinanceSector::Tertiary);
    }

    #[test]
    fn finance_profile_descriptor_matches_runtime_constants() {
        let profile = FinanceVanillaProfile;
        let descriptor = profile.descriptor();

        assert_eq!(descriptor.profile_id, COUNTRY_FINANCE_PROFILE_ID);
        assert_eq!(descriptor.root_template, COUNTRY_FINANCE_ROOT);
        assert_eq!(descriptor.required_gui_files, &[COUNTRY_FINANCE_GUI_FILE]);
        assert_eq!(descriptor.required_sprites, FINANCE_REQUIRED_SPRITES);
        assert_eq!(descriptor.key_templates, FINANCE_KEY_TEMPLATES);
        assert_eq!(descriptor.key_templates.len(), 5);
        assert!(descriptor.key_templates.contains(&"finance_budget_row"));
        assert!(descriptor.key_templates.contains(&"finance_gdp_row"));
        assert!(descriptor.key_templates.contains(&"finance_sector_row"));
        assert!(descriptor.key_templates.contains(&"finance_employment_row"));
        assert!(descriptor.key_templates.contains(&"finance_diagnostic_row"));
        assert!(profile.uses_slide_animation());
    }

    #[test]
    fn finance_profile_skeleton_only_handles_close() {
        let profile = FinanceVanillaProfile;
        let input = sample_input();

        let close = profile.handle_action(
            GuiAction {
                node_path: GuiNodePath::root("close"),
                kind: GuiActionKind::Click,
            },
            &input,
        );
        assert_eq!(
            close,
            Some(FinanceCommand::Panel(PanelCommand::ClosePrimary))
        );

        let ignored = profile.handle_action(
            GuiAction {
                node_path: GuiNodePath::root("finance:tab:budget"),
                kind: GuiActionKind::Click,
            },
            &input,
        );
        assert_eq!(ignored, None);
    }
}
