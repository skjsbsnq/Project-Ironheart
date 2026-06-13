use crate::finance_panel::{format_million, signed_million, FinanceCommand, FinancePanelData};
use crate::vanilla_gui::binding::{GuiAction, GuiActionKind, GuiBinding};
use crate::vanilla_gui::profile::{VanillaPanelProfile, VanillaTemplateInstance};
use crate::vanilla_gui::GuiNodePath;
use crate::vanilla_gui::{
    COUNTRY_FINANCE_GUI_FILE, COUNTRY_FINANCE_PROFILE_ID, COUNTRY_FINANCE_ROOT,
    FINANCE_KEY_TEMPLATES, FINANCE_REQUIRED_SPRITES,
};
use crate::{ActiveDetailPanel, ActivePrimaryPanel, PanelCommand};
use egui::Color32;

// ─── 财政面板调色板（egui Color32，独立于 ledger 的 VanillaIron 调色板，
//      便于纯函数 binding 与单测断言） ───────────────────────────────
const FIN_GOOD: Color32 = Color32::from_rgb(0x60, 0xc0, 0x60);
const FIN_WARN: Color32 = Color32::from_rgb(0xc0, 0xc0, 0x30);
const FIN_BAD: Color32 = Color32::from_rgb(0xc0, 0x40, 0x40);
const FIN_GOLD: Color32 = Color32::from_rgb(0xc8, 0xa4, 0x4c);
const FIN_INFO: Color32 = Color32::from_rgb(0x6c, 0x9c, 0xc8);
const FIN_MUTED: Color32 = Color32::from_gray(0x88);
const FIN_TEXT: Color32 = Color32::from_rgb(0xe6, 0xdf, 0xc8);
// tab/sector 按钮共用 GFX_tiled_button，靠 tint 区分选中态（选中偏亮金，未选中压暗）。
const FIN_TAB_SELECTED: Color32 = Color32::from_rgb(0xff, 0xe6, 0xa8);
const FIN_TAB_IDLE: Color32 = Color32::from_rgb(0x8c, 0x88, 0x78);

fn finance_ratio(value: f64, total: f64) -> f64 {
    if total > 0.0 {
        value / total
    } else {
        0.0
    }
}

fn debt_ratio_color(ratio: f64) -> Color32 {
    if ratio >= 0.75 {
        FIN_BAD
    } else if ratio >= 0.45 {
        FIN_WARN
    } else {
        FIN_GOOD
    }
}

fn mefo_ratio_color(ratio: f64) -> Color32 {
    if ratio >= 0.30 {
        FIN_BAD
    } else if ratio >= 0.18 {
        FIN_WARN
    } else {
        FIN_GOOD
    }
}

fn balance_color(amount: f64) -> Color32 {
    if amount.abs() < 0.5 {
        FIN_MUTED
    } else if amount > 0.0 {
        FIN_GOOD
    } else {
        FIN_BAD
    }
}

fn rating_color(rating: &str) -> Color32 {
    match rating {
        "AAA" => Color32::from_rgb(0x60, 0xc0, 0x60),
        "AA" => Color32::from_rgb(0x80, 0xc0, 0x50),
        "A" => Color32::from_rgb(0xa0, 0xc0, 0x30),
        "BBB" => Color32::from_rgb(0xc0, 0xc0, 0x30),
        "BB" => Color32::from_rgb(0xc0, 0xa0, 0x20),
        "B" => Color32::from_rgb(0xc0, 0x80, 0x20),
        "CCC" => Color32::from_rgb(0xc0, 0x60, 0x20),
        "D" => Color32::from_rgb(0xc0, 0x30, 0x30),
        _ => Color32::from_gray(200),
    }
}

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

    /// 玩家可见 tab 标签。
    pub fn label(self) -> &'static str {
        match self {
            Self::Overview => "总览",
            Self::Budget => "预算",
            Self::Debt => "债务",
            Self::Exchange => "外汇",
            Self::Economy => "经济",
            Self::Funding => "资金",
        }
    }

    /// `.gui` 里 tab 栏容器名（`tab_<id>`）→ enum。
    pub fn from_tab_container(name: &str) -> Option<Self> {
        name.strip_prefix("tab_")
            .filter(|rest| !rest.starts_with("body_"))
            .and_then(Self::from_id)
    }

    /// `.gui` 里 tab body 容器名（`tab_body_<id>`）→ enum。
    pub fn from_body_container(name: &str) -> Option<Self> {
        name.strip_prefix("tab_body_").and_then(Self::from_id)
    }

    /// tab `hit` 按钮点击命令；由 show 函数拦截，不进 `FinanceCommand`。
    pub fn tab_click_command(self) -> String {
        format!("finance:tab:{}", self.id())
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

    /// 玩家可见 sector 标签。
    pub fn label(self) -> &'static str {
        match self {
            Self::Primary => "一产",
            Self::Secondary => "二产",
            Self::Tertiary => "三产",
        }
    }

    /// `EconomySectorBuildingEntry.sector_id` 匹配键。
    pub fn sector_id(self) -> &'static str {
        self.id()
    }

    /// sector 选择按钮点击命令；由 show 函数拦截，不进 `FinanceCommand`。
    pub fn sector_click_command(self) -> String {
        format!("finance:sector:{}", self.id())
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

    fn bind_node(&self, node_path: &GuiNodePath, data: &Self::Data) -> GuiBinding {
        let segments = &node_path.0;
        let name = segments.last().map(String::as_str).unwrap_or_default();
        let path_str = node_path.to_string();

        if path_str.ends_with("finance_title") {
            return GuiBinding::default()
                .text("财政")
                .tooltip(finance_title_tooltip(&data.data));
        }
        if path_str.ends_with("close_button") {
            return GuiBinding::default().click("close").tooltip("关闭财政面板");
        }
        if path_str.ends_with("finance_footer") {
            return GuiBinding::default().text("数据每日刷新 · 点击行查看详情");
        }

        // Gate 4 字色梯度：分区头（fb/ti/cb/ib_header）金色；KV 标签 muted。
        // 命中后仍允许后续 tv_* 值绑定按 leaf 名分流（值文本不走这两条）。
        if matches!(name, "fb_header" | "ti_header" | "cb_header" | "ib_header") {
            return GuiBinding::default().text_color(FIN_GOLD);
        }
        if name == "kv_label" {
            return GuiBinding::default().text_color(FIN_MUTED);
        }

        // KPI 条：6 格各含 kpi_label / kpi_value，靠父容器名区分。
        if name == "kpi_label" || name == "kpi_value" {
            if let Some(cell) = segments
                .iter()
                .rev()
                .nth(1)
                .map(String::as_str)
                .and_then(FinanceKpiCell::from_id)
            {
                return bind_kpi_node(cell, name, &data.data);
            }
        }

        // tab 栏：每个 tab_<id> 容器含 tab_bg / hit / label。
        // 选中态靠 input.tab 决定 sprite + label 颜色；hit 绑 finance:tab:<id> 点击
        //（点击在 show 函数拦截，不进 handle_action / FinanceCommand）。
        if matches!(name, "tab_bg" | "hit" | "label") {
            if let Some(tab) = segments
                .iter()
                .rev()
                .nth(1)
                .and_then(|seg| seg.strip_prefix("tab_"))
                .and_then(FinanceTab::from_id)
            {
                return bind_tab_node(name, tab, data.tab);
            }
        }

        // body：6 个同位 tab_body_*，仅当前 tab 可见。
        if let Some(body_tab) = name.strip_prefix("tab_body_").and_then(FinanceTab::from_id) {
            return GuiBinding::default().visible(body_tab == data.tab);
        }

        // 总览 tab 固定区：国库 KV / 进度条 / 压力三块 / 风险横幅。
        if let Some(binding) = bind_overview_node(name, &data.data) {
            return binding;
        }

        // 债务 tab 固定区 + 动作按钮门控。
        if let Some(binding) = bind_debt_node(name, &data.data) {
            return binding;
        }

        // 外汇 tab 固定区 + 动作按钮门控。
        if let Some(binding) = bind_exchange_node(name, &data.data) {
            return binding;
        }

        // 资金 tab 固定区：建造资金路径 + 投资池。
        if let Some(binding) = bind_funding_node(name, &data.data) {
            return binding;
        }

        // 经济 tab sector 选择按钮：选中态靠 input.sector，click 走 finance:sector:<id>
        //（show 函数拦截，不进 handle_action / FinanceCommand）。
        if let Some(sector) = sector_button_target(name) {
            return bind_sector_button(sector, data.sector);
        }

        // 预算 tab 固定合计区（收入合计/支出合计/日结余/MEFO 备注）。
        if let Some(binding) = bind_budget_totals_node(name, &data.data) {
            return binding;
        }

        // 经济 tab GDP 总览固定区。
        if let Some(binding) = bind_economy_totals_node(name, &data.data) {
            return binding;
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
        let model_key = instance.model_key.as_deref();
        match instance.semantic_role.as_deref() {
            Some("finance_budget_row") => bind_budget_row_node(name, model_key, &data.data),
            Some("finance_gdp_row") => bind_gdp_row_node(name, model_key, &data.data),
            Some("finance_sector_row") => bind_sector_row_node(name, model_key, &data.data),
            Some("finance_employment_row") => bind_employment_row_node(name, model_key, &data.data),
            Some("finance_diagnostic_row") => bind_diagnostic_row_node(name, model_key, &data.data),
            _ => self.bind_node(node_path, data),
        }
    }

    fn handle_action(&self, action: GuiAction, data: &Self::Data) -> Option<Self::Command> {
        match action.kind {
            GuiActionKind::Click => {
                let path = action.node_path.to_string();
                if path == "close" || path.ends_with("close_button") {
                    Some(FinanceCommand::Panel(PanelCommand::ClosePrimary))
                } else {
                    match path.as_str() {
                        "finance:action:issue_domestic" => {
                            Some(FinanceCommand::IssueDomesticBond {
                                amount_rm: 500_000_000.0,
                            })
                        }
                        "finance:action:issue_foreign" => Some(FinanceCommand::IssueForeignBond {
                            amount_gbp: 100_000_000.0,
                        }),
                        "finance:action:print_mefo" => Some(FinanceCommand::PrintMefo),
                        "finance:action:sell_gold" => Some(FinanceCommand::SellGold {
                            kg: data.data.gold_kg * 0.1,
                        }),
                        "finance:action:buy_forex" => Some(FinanceCommand::BuyForeignCurrency {
                            gbp_amount: 10_000_000.0,
                        }),
                        // 行点击路由：跳到相关主面板 / 债务详情抽屉。
                        "finance:goto:trade" => Some(FinanceCommand::Panel(
                            PanelCommand::OpenPrimary(ActivePrimaryPanel::Trade),
                        )),
                        "finance:goto:logistics" => Some(FinanceCommand::Panel(
                            PanelCommand::OpenPrimary(ActivePrimaryPanel::Logistics),
                        )),
                        "finance:goto:construction" => Some(FinanceCommand::Panel(
                            PanelCommand::OpenPrimary(ActivePrimaryPanel::Construction),
                        )),
                        "finance:goto:debt" => Some(FinanceCommand::Panel(
                            PanelCommand::OpenDetail(ActiveDetailPanel::FinanceDebt),
                        )),
                        _ => None,
                    }
                }
            }
            GuiActionKind::Hover => None,
        }
    }

    fn uses_slide_animation(&self) -> bool {
        true
    }
}

// ─── KPI 条绑定 ─────────────────────────────────────────────────

/// 顶部 KPI 条 6 格，与 `countryfinanceview.gui` 的 `kpi_*` 容器一一对应。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FinanceKpiCell {
    Cash,
    Net,
    Reserve,
    Debt,
    Mefo,
    Rating,
}

impl FinanceKpiCell {
    fn from_id(value: &str) -> Option<Self> {
        match value {
            "kpi_cash" => Some(Self::Cash),
            "kpi_net" => Some(Self::Net),
            "kpi_reserve" => Some(Self::Reserve),
            "kpi_debt" => Some(Self::Debt),
            "kpi_mefo" => Some(Self::Mefo),
            "kpi_rating" => Some(Self::Rating),
            _ => None,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Cash => "现金",
            Self::Net => "日净额",
            Self::Reserve => "外汇储备",
            Self::Debt => "债务/GDP",
            Self::Mefo => "MEFO/GDP",
            Self::Rating => "信用评级",
        }
    }
}

/// 给定 KPI 格与节点名（`kpi_label` / `kpi_value`），产出文本 + 颜色绑定。
fn bind_kpi_node(cell: FinanceKpiCell, name: &str, data: &FinancePanelData) -> GuiBinding {
    if name == "kpi_label" {
        return GuiBinding::default()
            .text(cell.label())
            .text_color(FIN_MUTED);
    }

    let (value, color) = kpi_value(cell, data);
    GuiBinding::default().text(value).text_color(color)
}

/// tab 容器内的 `tab_bg` / `hit` / `label` 绑定。
/// 选中态：`tab_bg` 用真页签 sprite `GFX_tab_intel_ledger`（noOfFrames=2），
/// frame 2=选中（底边高亮）/ frame 1=未选中；不再走 tint。
/// `label` 文本色仍按选中态微调（金 vs 中性），仅作辅助强调。
/// `hit` 始终绑 `finance:tab:<id>` 点击命令（show 函数拦截，不进 FinanceCommand）。
fn bind_tab_node(name: &str, tab: FinanceTab, active: FinanceTab) -> GuiBinding {
    let selected = tab == active;
    match name {
        "tab_bg" => GuiBinding::default()
            .sprite("GFX_tab_intel_ledger")
            .frame(if selected { 2 } else { 1 }),
        "hit" => GuiBinding::default()
            .click(tab.tab_click_command())
            .tooltip(tab.label()),
        "label" => GuiBinding::default()
            .text(tab.label())
            .text_color(if selected { FIN_GOLD } else { FIN_TEXT }),
        _ => GuiBinding::default(),
    }
}

fn kpi_value(cell: FinanceKpiCell, data: &FinancePanelData) -> (String, Color32) {
    match cell {
        FinanceKpiCell::Cash => (format_million(data.cash_rm), balance_color(data.cash_rm)),
        FinanceKpiCell::Net => {
            let net = data.daily_income_rm - data.daily_expense_rm;
            (signed_million(net), balance_color(net))
        }
        FinanceKpiCell::Reserve => (format!("£ {}", format_million(data.reserve_gbp)), FIN_INFO),
        FinanceKpiCell::Debt => {
            let ratio = finance_ratio(data.public_debt_rm + data.mefo_debt_rm, data.gdp_rm);
            (format!("{:.1}%", ratio * 100.0), debt_ratio_color(ratio))
        }
        FinanceKpiCell::Mefo => {
            let ratio = finance_ratio(data.mefo_debt_rm, data.gdp_rm);
            (format!("{:.1}%", ratio * 100.0), mefo_ratio_color(ratio))
        }
        FinanceKpiCell::Rating => (
            data.credit_rating.clone(),
            rating_color(&data.credit_rating),
        ),
    }
}

fn finance_title_tooltip(data: &FinancePanelData) -> String {
    let net = data.daily_income_rm - data.daily_expense_rm;
    format!(
        "现金 {} | 日净额 {} | 储备 £{}",
        format_million(data.cash_rm),
        signed_million(net),
        format_million(data.reserve_gbp)
    )
}

// ─── 总览 tab 绑定（treasury KV / 进度条 / 压力三块 / 风险横幅） ───────

/// 派生风险标签，门限与 ledger `finance_risk_label` 一致。
fn finance_risk_label(daily_balance: f64, debt_ratio: f64, mefo_ratio: f64) -> String {
    if daily_balance < 0.0 && mefo_ratio >= 0.30 {
        "高风险：赤字 + MEFO 透支，违约/通胀压力剧增".to_owned()
    } else if daily_balance < 0.0 || debt_ratio >= 0.45 || mefo_ratio >= 0.18 {
        "中风险：财政承压，关注赤字与债务比率".to_owned()
    } else {
        "低风险：财政稳健".to_owned()
    }
}

fn finance_risk_color(daily_balance: f64, debt_ratio: f64, mefo_ratio: f64) -> Color32 {
    if daily_balance < 0.0 && mefo_ratio >= 0.30 {
        FIN_BAD
    } else if daily_balance < 0.0 || debt_ratio >= 0.45 || mefo_ratio >= 0.18 {
        FIN_WARN
    } else {
        FIN_GOOD
    }
}

/// 总览 tab 的固定区绑定：treasury 6 KV、债务/MEFO 进度条、压力三块、风险横幅。
/// 节点名均唯一（`tv_*` / `debt_bar` / `mefo_bar` / `risk_banner`），直接按 leaf 名分流。
fn bind_overview_node(name: &str, data: &FinancePanelData) -> Option<GuiBinding> {
    let debt_ratio = finance_ratio(data.public_debt_rm + data.mefo_debt_rm, data.gdp_rm);
    let mefo_ratio = finance_ratio(data.mefo_debt_rm, data.gdp_rm);
    let daily_balance = data.daily_income_rm - data.daily_expense_rm;

    let binding = match name {
        // treasury 6 KV。
        "tv_cash" => GuiBinding::default()
            .text(format_million(data.cash_rm))
            .text_color(balance_color(data.cash_rm)),
        "tv_reserve" => GuiBinding::default()
            .text(format!("£ {}", format_million(data.reserve_gbp)))
            .text_color(FIN_INFO),
        "tv_gold" => GuiBinding::default()
            .text(format!("{} kg", format_million(data.gold_kg)))
            .text_color(FIN_GOLD),
        "tv_cover" => {
            let days = if data.daily_expense_rm > 0.0 {
                data.cash_rm / data.daily_expense_rm
            } else {
                0.0
            };
            GuiBinding::default()
                .text(if data.daily_expense_rm > 0.0 {
                    format!("{:.0} 天", days)
                } else {
                    "—".to_owned()
                })
                .text_color(if days < 30.0 { FIN_WARN } else { FIN_TEXT })
        }
        "tv_oper" => {
            let oper = data.operating_income_rm - data.operating_expense_rm;
            GuiBinding::default()
                .text(signed_million(oper))
                .text_color(balance_color(oper))
        }
        "tv_postfin" => GuiBinding::default()
            .text(signed_million(data.post_financing_cash_change_rm))
            .text_color(balance_color(data.post_financing_cash_change_rm)),
        // 进度条 + 比率文本。
        "debt_bar" => GuiBinding::default().progress(debt_ratio as f32),
        "tv_debt_ratio" => GuiBinding::default()
            .text(format!("{:.1}%", debt_ratio * 100.0))
            .text_color(debt_ratio_color(debt_ratio)),
        "mefo_bar" => GuiBinding::default().progress(mefo_ratio as f32),
        "tv_mefo_ratio" => GuiBinding::default()
            .text(format!("{:.1}%", mefo_ratio * 100.0))
            .text_color(mefo_ratio_color(mefo_ratio)),
        // 压力三块（来源 budget_breakdown.expense_*）。
        "tv_pressure_military" => {
            let military = data.budget_breakdown.expense_military_wages_rm
                + data.budget_breakdown.expense_military_procurement_rm
                + data.budget_breakdown.expense_military_maintenance_rm;
            GuiBinding::default()
                .text(format_million(military))
                .text_color(FIN_TEXT)
        }
        "tv_pressure_construction" => {
            let construction = data.budget_breakdown.expense_construction_goods_rm
                + data.budget_breakdown.expense_construction_wages_rm;
            GuiBinding::default()
                .text(format_million(construction))
                .text_color(FIN_TEXT)
        }
        "tv_pressure_debt" => GuiBinding::default()
            .text(format_million(
                data.budget_breakdown.expense_debt_interest_rm,
            ))
            .text_color(FIN_TEXT),
        // 风险横幅。
        "risk_banner" => GuiBinding::default()
            .text(finance_risk_label(daily_balance, debt_ratio, mefo_ratio))
            .text_color(finance_risk_color(daily_balance, debt_ratio, mefo_ratio)),
        _ => return None,
    };
    Some(binding)
}

// ─── 债务 tab 绑定（debt_metrics / financing_block / 动作门控） ───────

/// 债务 tab 固定区 + 3 个动作按钮（发债/外债/MEFO）。
/// 动作按钮 `.click("finance:action:*")` + `.enabled(门控)`：
/// 外债 = `can_issue_foreign_bond`；MEFO = `can_print_mefo && deficit>0 && mefo/gdp<0.30`；国债恒开。
fn bind_debt_node(name: &str, data: &FinancePanelData) -> Option<GuiBinding> {
    let mefo_ratio = finance_ratio(data.mefo_debt_rm, data.gdp_rm);
    let deficit = data.daily_expense_rm - data.daily_income_rm;

    let binding = match name {
        // debt_metrics 7 KV。
        "tv_public_debt" => GuiBinding::default()
            .text(format_million(data.public_debt_rm))
            .text_color(FIN_TEXT),
        "tv_public_debt_gbp" => GuiBinding::default()
            .text(format!("£ {}", format_million(data.public_debt_gbp)))
            .text_color(FIN_INFO),
        "tv_mefo_debt" => GuiBinding::default()
            .text(format!(
                "{} ({:.1}%)",
                format_million(data.mefo_debt_rm),
                mefo_ratio * 100.0
            ))
            .text_color(mefo_ratio_color(mefo_ratio)),
        "tv_debt_rating" => GuiBinding::default()
            .text(data.credit_rating.clone())
            .text_color(rating_color(&data.credit_rating)),
        "tv_interest" => GuiBinding::default()
            .text(format!("{:.2}%", data.bond_interest_rate * 100.0))
            .text_color(FIN_TEXT),
        "tv_mefo_budget" => GuiBinding::default()
            .text(format_million(data.mefo_military_budget_rm))
            .text_color(FIN_TEXT),
        "tv_mefo_spent" => GuiBinding::default()
            .text(format_million(data.mefo_military_spent_rm))
            .text_color(FIN_TEXT),
        // financing_block 4 KV。
        "tv_mefo_issued" => GuiBinding::default()
            .text(format_million(data.financing_breakdown.mefo_issued_rm))
            .text_color(FIN_TEXT),
        "tv_mefo_cap" => GuiBinding::default()
            .text(format_million(
                data.financing_breakdown.mefo_interest_capitalized_rm,
            ))
            .text_color(FIN_TEXT),
        "tv_bond_issued" => GuiBinding::default()
            .text(format_million(
                data.financing_breakdown.domestic_bond_issued_rm,
            ))
            .text_color(FIN_TEXT),
        "tv_coverage" => GuiBinding::default()
            .text(format_million(data.mefo_coverage_rm))
            .text_color(FIN_TEXT),
        // 动作按钮：click + 门控。
        "act_issue_domestic" => GuiBinding::default()
            .click("finance:action:issue_domestic")
            .tooltip("发行国内债券（500M RM）")
            .enabled(true),
        "act_issue_foreign" => GuiBinding::default()
            .click("finance:action:issue_foreign")
            .tooltip("发行外币债券（100M £）")
            .enabled(data.can_issue_foreign_bond),
        "act_print_mefo" => GuiBinding::default()
            .click("finance:action:print_mefo")
            .tooltip("发行 MEFO 票据覆盖赤字")
            .enabled(data.can_print_mefo && deficit > 0.0 && mefo_ratio < 0.30),
        _ => return None,
    };
    Some(binding)
}

// ─── 外汇 tab 绑定（forex_metrics / trade_impact / 动作门控） ───────

/// 外汇 tab 固定区 + 2 个动作按钮（卖金/买汇）。
/// 卖金 = `gold_kg>0`；买汇 = `!is_foreign_exchange_control`。
fn bind_exchange_node(name: &str, data: &FinancePanelData) -> Option<GuiBinding> {
    let binding = match name {
        // forex_metrics 4 KV。
        "tv_forex_reserve" => GuiBinding::default()
            .text(format!("£ {}", format_million(data.reserve_gbp)))
            .text_color(FIN_INFO),
        "tv_forex_gold" => GuiBinding::default()
            .text(format!("{} kg", format_million(data.gold_kg)))
            .text_color(FIN_GOLD),
        "tv_forex_rate" => GuiBinding::default()
            .text(format!("{:.2} RM/£", data.exchange_rate_rm_per_gbp))
            .text_color(FIN_TEXT),
        "tv_forex_control" => GuiBinding::default()
            .text(if data.is_foreign_exchange_control {
                "外汇管制中"
            } else {
                "自由兑换"
            })
            .text_color(if data.is_foreign_exchange_control {
                FIN_WARN
            } else {
                FIN_GOOD
            }),
        // trade_impact 3 KV。
        "tv_net_exports" => GuiBinding::default()
            .text(signed_million(data.gdp_breakdown.net_exports_rm))
            .text_color(balance_color(data.gdp_breakdown.net_exports_rm)),
        "tv_trade_tariffs" => GuiBinding::default()
            .text(format_million(data.fiscal_revenue.trade_tariffs_rm))
            .text_color(FIN_TEXT),
        "tv_forex_expense" => GuiBinding::default()
            .text(format_million(data.fiscal_expense.foreign_exchange_rm))
            .text_color(FIN_TEXT),
        // 动作按钮：click + 门控。
        "act_sell_gold" => GuiBinding::default()
            .click("finance:action:sell_gold")
            .tooltip("出售黄金换取外汇（10% 储备）")
            .enabled(data.gold_kg > 0.0),
        "act_buy_forex" => GuiBinding::default()
            .click("finance:action:buy_forex")
            .tooltip("购买外汇（10M £）")
            .enabled(!data.is_foreign_exchange_control),
        _ => return None,
    };
    Some(binding)
}

// ─── 资金 tab 绑定（construction_block / investment_block） ───────

/// 资金 tab 固定区：建造资金路径 9 项 + 预算合计、投资池 8 项。
/// 节点名均唯一（`tv_cf_*` / `tv_ip_*`），按 leaf 名分流。
fn bind_funding_node(name: &str, data: &FinancePanelData) -> Option<GuiBinding> {
    let cf = &data.construction_funding;
    let ip = &data.investment_pool;

    // 中性 KV 文本色，与其它 tab 一致。
    let plain = |text: String| GuiBinding::default().text(text).text_color(FIN_TEXT);

    let binding = match name {
        // construction_funding 9 项 + 合计。
        "tv_cf_government" => plain(format_million(cf.government_rm)),
        "tv_cf_mefo" => plain(format_million(cf.mefo_rm)),
        "tv_cf_private" => plain(format_million(cf.private_pool_rm)),
        "tv_cf_cartel" => plain(format_million(cf.cartel_pool_rm)),
        "tv_cf_overlord" => plain(format_million(cf.overlord_investment_rm)),
        "tv_cf_foreign" => plain(format_million(cf.foreign_investment_rm)),
        "tv_cf_paid" => GuiBinding::default()
            .text(format_million(cf.paid_rm))
            .text_color(FIN_GOOD),
        "tv_cf_remaining" => GuiBinding::default()
            .text(format_million(cf.remaining_rm))
            .text_color(if cf.remaining_rm > 0.0 {
                FIN_WARN
            } else {
                FIN_TEXT
            }),
        "tv_cf_projects" => plain(format!("{}", cf.active_projects)),
        "tv_cf_total" => GuiBinding::default()
            .text(format_million(cf.total_budget_rm()))
            .text_color(FIN_INFO),
        // investment_pool 8 项。
        "tv_ip_private" => plain(format_million(ip.private_rm)),
        "tv_ip_cartel" => plain(format_million(ip.cartel_rm)),
        "tv_ip_sdb" => plain(format_million(ip.state_development_bank_rm)),
        "tv_ip_colonial" => plain(format_million(ip.colonial_extraction_rm)),
        "tv_ip_foreign" => plain(format_million(ip.foreign_capital_rm)),
        "tv_ip_income" => GuiBinding::default()
            .text(signed_million(ip.income_rm))
            .text_color(balance_color(ip.income_rm)),
        "tv_ip_spent" => plain(format_million(ip.spent_rm)),
        "tv_ip_total" => GuiBinding::default()
            .text(format_million(ip.total_rm))
            .text_color(FIN_INFO),
        _ => return None,
    };
    Some(binding)
}

// ─── 动态列表行模型（与 finance_panel runtime frame 共享） ───────────────
//
// 预算 / GDP 行用稳定字符串键（key）：instance spec 取非零项的 key 作 model_keys，
// 绑定层按 key 反查模型；产业 / 就业 / 诊断行直接用 data 中的绝对下标作 model_key。

/// 可路由行的目标面板；映射到 `finance:goto:*` 命令（show 转 `FinanceCommand::Panel`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FinanceRoute {
    Trade,
    Logistics,
    Construction,
    Debt,
}

impl FinanceRoute {
    pub(crate) fn command(self) -> &'static str {
        match self {
            Self::Trade => "finance:goto:trade",
            Self::Logistics => "finance:goto:logistics",
            Self::Construction => "finance:goto:construction",
            Self::Debt => "finance:goto:debt",
        }
    }
}

/// 预算双栏单行模型。
#[derive(Debug, Clone)]
pub(crate) struct FinanceBudgetRowModel {
    pub key: &'static str,
    pub label: &'static str,
    pub amount: f64,
    pub income: bool,
    pub route: Option<FinanceRoute>,
}

fn budget_income_all(data: &FinancePanelData) -> Vec<FinanceBudgetRowModel> {
    let r = &data.fiscal_revenue;
    let row = |key: &'static str, label: &'static str, amount: f64, route: Option<FinanceRoute>| {
        FinanceBudgetRowModel {
            key,
            label,
            amount,
            income: true,
            route,
        }
    };
    vec![
        row(
            "budget:inc:pop_tax",
            "POP 所得税",
            r.pop_income_taxes_rm,
            None,
        ),
        row(
            "budget:inc:consumption",
            "消费税",
            r.consumption_taxes_rm,
            None,
        ),
        row("budget:inc:corporate", "企业税", r.corporate_taxes_rm, None),
        row(
            "budget:inc:tariffs",
            "贸易关税",
            r.trade_tariffs_rm,
            Some(FinanceRoute::Trade),
        ),
        row(
            "budget:inc:state_profit",
            "国企利润",
            r.state_profit_rm,
            None,
        ),
        row(
            "budget:inc:financing",
            "融资流入",
            r.financing_rm,
            Some(FinanceRoute::Debt),
        ),
        row("budget:inc:other", "其他收入", r.other_rm, None),
    ]
}

fn budget_expense_all(data: &FinancePanelData) -> Vec<FinanceBudgetRowModel> {
    let e = &data.fiscal_expense;
    let row = |key: &'static str, label: &'static str, amount: f64, route: Option<FinanceRoute>| {
        FinanceBudgetRowModel {
            key,
            label,
            amount,
            income: false,
            route,
        }
    };
    vec![
        row(
            "budget:exp:military",
            "军费",
            e.military_rm,
            Some(FinanceRoute::Logistics),
        ),
        row(
            "budget:exp:construction",
            "建设支出",
            e.construction_rm,
            Some(FinanceRoute::Construction),
        ),
        row("budget:exp:welfare", "福利", e.welfare_rm, None),
        row("budget:exp:admin", "行政", e.administration_rm, None),
        row(
            "budget:exp:interest",
            "债务利息",
            e.interest_rm,
            Some(FinanceRoute::Debt),
        ),
        row(
            "budget:exp:forex",
            "外汇支出",
            e.foreign_exchange_rm,
            Some(FinanceRoute::Trade),
        ),
        row("budget:exp:research", "研究经费", e.research_rm, None),
        row("budget:exp:other", "其他支出", e.other_rm, None),
    ]
}

/// 非零收入行（runtime frame 用其 key 作 model_keys）。
pub(crate) fn finance_budget_income_models(data: &FinancePanelData) -> Vec<FinanceBudgetRowModel> {
    budget_income_all(data)
        .into_iter()
        .filter(|row| row.amount.abs() >= 0.5)
        .collect()
}

/// 非零支出行。
pub(crate) fn finance_budget_expense_models(data: &FinancePanelData) -> Vec<FinanceBudgetRowModel> {
    budget_expense_all(data)
        .into_iter()
        .filter(|row| row.amount.abs() >= 0.5)
        .collect()
}

fn finance_budget_row_by_key(data: &FinancePanelData, key: &str) -> Option<FinanceBudgetRowModel> {
    budget_income_all(data)
        .into_iter()
        .chain(budget_expense_all(data))
        .find(|row| row.key == key)
}

/// GDP 构成单行模型。
#[derive(Debug, Clone)]
pub(crate) struct FinanceGdpRowModel {
    pub key: &'static str,
    pub label: &'static str,
    pub amount: f64,
}

/// GDP 构成行（始终全列，比率随 gdp_rm 计算）。
pub(crate) fn finance_gdp_row_models(data: &FinancePanelData) -> Vec<FinanceGdpRowModel> {
    let g = &data.gdp_breakdown;
    let row = |key: &'static str, label: &'static str, amount: f64| FinanceGdpRowModel {
        key,
        label,
        amount,
    };
    vec![
        row("gdp:primary", "一产增加值", g.building_primary_rm),
        row("gdp:secondary", "二产增加值", g.building_secondary_rm),
        row("gdp:tertiary", "三产增加值", g.building_tertiary_rm),
        row("gdp:pop_income", "POP 收入", g.pop_income_rm),
        row("gdp:pop_consumption", "POP 消费", g.pop_consumption_rm),
        row("gdp:gov_services", "政府服务", g.government_services_rm),
        row("gdp:mil_procurement", "军工采购", g.military_procurement_rm),
        row("gdp:net_exports", "净出口", g.net_exports_rm),
        row("gdp:colonial", "殖民增加值", g.colonial_value_added_rm),
    ]
}

fn finance_gdp_row_by_key(data: &FinancePanelData, key: &str) -> Option<FinanceGdpRowModel> {
    finance_gdp_row_models(data)
        .into_iter()
        .find(|row| row.key == key)
}

/// 当前 sector 过滤后的 `sector_buildings` 绝对下标（model_key = 下标字符串，
/// 绑定层据此回查同一个 Vec，避免过滤口径不一致）。
pub(crate) fn finance_sector_indices(data: &FinancePanelData, sector: FinanceSector) -> Vec<usize> {
    data.sector_buildings
        .iter()
        .enumerate()
        .filter(|(_, building)| building.sector_id == sector.sector_id())
        .map(|(index, _)| index)
        .collect()
}

// ─── sector 选择按钮绑定（经济 tab） ─────────────────────────────

fn sector_button_target(name: &str) -> Option<FinanceSector> {
    match name {
        "sector_primary" => Some(FinanceSector::Primary),
        "sector_secondary" => Some(FinanceSector::Secondary),
        "sector_tertiary" => Some(FinanceSector::Tertiary),
        _ => None,
    }
}

fn bind_sector_button(sector: FinanceSector, active: FinanceSector) -> GuiBinding {
    let selected = sector == active;
    GuiBinding::default()
        .sprite("GFX_tiled_button")
        .tint(if selected {
            FIN_TAB_SELECTED
        } else {
            FIN_TAB_IDLE
        })
        .click(sector.sector_click_command())
        .tooltip(sector.label())
}

// ─── 预算合计 / GDP 总览固定区绑定 ───────────────────────────────

fn bind_budget_totals_node(name: &str, data: &FinancePanelData) -> Option<GuiBinding> {
    let net = data.daily_income_rm - data.daily_expense_rm;
    let binding = match name {
        "tv_income_total" => GuiBinding::default()
            .text(format_million(data.daily_income_rm))
            .text_color(FIN_GOOD),
        "tv_expense_total" => GuiBinding::default()
            .text(format_million(data.daily_expense_rm))
            .text_color(FIN_BAD),
        "tv_net_total" => GuiBinding::default()
            .text(signed_million(net))
            .text_color(balance_color(net)),
        "tv_mefo_note" => {
            if data.mefo_coverage_rm > 0.0 {
                GuiBinding::default()
                    .text(format!(
                        "MEFO 覆盖赤字 {}（不计经营收入）",
                        format_million(data.mefo_coverage_rm)
                    ))
                    .text_color(FIN_GOLD)
            } else {
                GuiBinding::default()
                    .text(format!(
                        "经营性 收入 {} / 支出 {}",
                        format_million(data.operating_income_rm),
                        format_million(data.operating_expense_rm)
                    ))
                    .text_color(FIN_MUTED)
            }
        }
        _ => return None,
    };
    Some(binding)
}

fn bind_economy_totals_node(name: &str, data: &FinancePanelData) -> Option<GuiBinding> {
    let pair = |rm: f64, gbp: f64| format!("{} / £{}", format_million(rm), format_million(gbp));
    let binding = match name {
        "tv_gt_total" => GuiBinding::default()
            .text(pair(data.gdp_rm, data.gdp_gbp))
            .text_color(FIN_GOLD),
        "tv_gt_domestic" => GuiBinding::default()
            .text(pair(data.domestic_gdp_rm, data.domestic_gdp_gbp))
            .text_color(FIN_GOOD),
        "tv_gt_colonial" => GuiBinding::default()
            .text(pair(data.colonial_gdp_rm, data.colonial_gdp_gbp))
            .text_color(FIN_WARN),
        "tv_gt_extracted" => GuiBinding::default()
            .text(pair(
                data.colonial_extracted_value_rm,
                data.colonial_extracted_value_gbp,
            ))
            .text_color(FIN_GOLD),
        _ => return None,
    };
    Some(binding)
}

// ─── 动态行单元绑定（bind_node_with_context 分流目标） ───────────────

fn bind_budget_row_node(
    name: &str,
    model_key: Option<&str>,
    data: &FinancePanelData,
) -> GuiBinding {
    let Some(model) = model_key.and_then(|key| finance_budget_row_by_key(data, key)) else {
        return GuiBinding::default();
    };
    let amount_color = if model.income { FIN_GOOD } else { FIN_BAD };
    match name {
        "label" => GuiBinding::default().text(model.label).text_color(FIN_TEXT),
        "amount" => GuiBinding::default()
            .text(format_million(model.amount))
            .text_color(amount_color),
        // Gate 10：方向图标走真 frame（GFX_resources_strip noOfFrames=7，
        // frame 1=收入 / frame 2=支出），不再叠加 tint——语义由金额色（绿/红）独立承担。
        "dir_icon" => GuiBinding::default()
            .visible(true)
            .frame(if model.income { 1 } else { 2 }),
        "row_hit" => match model.route {
            Some(route) => GuiBinding::default()
                .click(route.command())
                .tooltip("打开相关详情"),
            None => GuiBinding::default(),
        },
        _ => GuiBinding::default(),
    }
}

fn bind_gdp_row_node(name: &str, model_key: Option<&str>, data: &FinancePanelData) -> GuiBinding {
    let Some(model) = model_key.and_then(|key| finance_gdp_row_by_key(data, key)) else {
        return GuiBinding::default();
    };
    match name {
        "label" => GuiBinding::default().text(model.label).text_color(FIN_TEXT),
        "amount" => GuiBinding::default()
            .text(format_million(model.amount))
            .text_color(if model.amount >= 0.0 {
                FIN_TEXT
            } else {
                FIN_BAD
            }),
        "percent" => {
            let ratio = finance_ratio(model.amount, data.gdp_rm);
            GuiBinding::default()
                .text(format!("{:.1}%", ratio * 100.0))
                .text_color(FIN_MUTED)
        }
        _ => GuiBinding::default(),
    }
}

fn bind_sector_row_node(
    name: &str,
    model_key: Option<&str>,
    data: &FinancePanelData,
) -> GuiBinding {
    let Some(building) = model_key
        .and_then(|key| key.parse::<usize>().ok())
        .and_then(|index| data.sector_buildings.get(index))
    else {
        return GuiBinding::default();
    };
    match name {
        "building" => GuiBinding::default()
            .text(building.building_name.clone())
            .text_color(FIN_TEXT),
        "level" => GuiBinding::default()
            .text(building.level.to_string())
            .text_color(FIN_TEXT),
        "employ" => GuiBinding::default()
            .text(format!("{}/{}", building.employed, building.demand))
            .text_color(FIN_TEXT),
        "fill_bar" => GuiBinding::default().progress(building.employment_rate),
        "value" => GuiBinding::default()
            .text(format_million(building.value_added_rm))
            .text_color(FIN_GOLD),
        _ => GuiBinding::default(),
    }
}

fn bind_employment_row_node(
    name: &str,
    model_key: Option<&str>,
    data: &FinancePanelData,
) -> GuiBinding {
    let Some(row) = model_key
        .and_then(|key| key.parse::<usize>().ok())
        .and_then(|index| data.employment_rows.get(index))
    else {
        return GuiBinding::default();
    };
    match name {
        "label" => GuiBinding::default()
            .text(row.label.clone())
            .text_color(FIN_TEXT),
        "employed" => GuiBinding::default()
            .text(row.employed.to_string())
            .text_color(FIN_TEXT),
        "demand" => GuiBinding::default()
            .text(row.demand.to_string())
            .text_color(FIN_TEXT),
        "rate" => GuiBinding::default()
            .text(format!("{:.1}%", row.employment_rate * 100.0))
            .text_color(employment_rate_color(row.employment_rate)),
        "value" => GuiBinding::default()
            .text(format_million(row.value_added_rm))
            .text_color(FIN_GOLD),
        _ => GuiBinding::default(),
    }
}

fn bind_diagnostic_row_node(
    name: &str,
    model_key: Option<&str>,
    data: &FinancePanelData,
) -> GuiBinding {
    let Some(row) = model_key
        .and_then(|key| key.parse::<usize>().ok())
        .and_then(|index| data.diagnostics.get(index))
    else {
        return GuiBinding::default();
    };
    match name {
        "source" => GuiBinding::default()
            .text(row.source.clone())
            .text_color(FIN_TEXT),
        "status" => GuiBinding::default()
            .text(row.status.clone())
            .text_color(diagnostic_status_color(&row.status)),
        "detail" => GuiBinding::default()
            .text(row.detail.clone())
            .text_color(FIN_MUTED),
        _ => GuiBinding::default(),
    }
}

fn employment_rate_color(rate: f32) -> Color32 {
    if rate >= 0.92 {
        FIN_GOOD
    } else if rate >= 0.75 {
        FIN_WARN
    } else {
        FIN_BAD
    }
}

fn diagnostic_status_color(status: &str) -> Color32 {
    match status {
        "正常" | "可用" => FIN_GOOD,
        "关注" | "排队" => FIN_WARN,
        _ => FIN_BAD,
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

    fn kpi_path(cell: &str, leaf: &str) -> GuiNodePath {
        GuiNodePath::root(COUNTRY_FINANCE_ROOT)
            .child("kpi_strip")
            .child(cell)
            .child(leaf)
    }

    #[test]
    fn finance_bind_kpi() {
        let profile = FinanceVanillaProfile;
        let mut data = FinancePanelData::default();
        data.cash_rm = 12_000_000.0;
        data.daily_income_rm = 5_000_000.0;
        data.daily_expense_rm = 8_000_000.0; // 日净额 = -3.0M
        data.reserve_gbp = 4_000_000.0;
        data.gdp_rm = 100_000_000.0;
        data.public_debt_rm = 40_000_000.0;
        data.mefo_debt_rm = 20_000_000.0; // 债务/GDP = 60% -> WARN
        data.credit_rating = "AAA".to_owned();
        let input = FinanceVanillaInput::new(data, FinanceTab::Overview, FinanceSector::Primary);

        // 标题 / 关闭 / footer。
        let title = profile.bind_node(
            &GuiNodePath::root(COUNTRY_FINANCE_ROOT).child("finance_title"),
            &input,
        );
        assert_eq!(title.text.as_deref(), Some("财政"));
        assert!(title.tooltip.is_some());

        let close = profile.bind_node(
            &GuiNodePath::root(COUNTRY_FINANCE_ROOT).child("close_button"),
            &input,
        );
        assert_eq!(
            close.click.as_ref().map(|c| c.command.as_str()),
            Some("close")
        );

        let footer = profile.bind_node(
            &GuiNodePath::root(COUNTRY_FINANCE_ROOT).child("finance_footer"),
            &input,
        );
        assert!(footer.text.is_some());

        // 现金：正值 -> GOOD。
        let cash = profile.bind_node(&kpi_path("kpi_cash", "kpi_value"), &input);
        assert_eq!(cash.text.as_deref(), Some("12.0M"));
        assert_eq!(cash.text_color, Some(FIN_GOOD));
        let cash_label = profile.bind_node(&kpi_path("kpi_cash", "kpi_label"), &input);
        assert_eq!(cash_label.text.as_deref(), Some("现金"));

        // 日净额：负值 -> BAD，带符号。
        let net = profile.bind_node(&kpi_path("kpi_net", "kpi_value"), &input);
        assert_eq!(net.text.as_deref(), Some("-3.0M"));
        assert_eq!(net.text_color, Some(FIN_BAD));

        // 储备：£ 前缀 + INFO。
        let reserve = profile.bind_node(&kpi_path("kpi_reserve", "kpi_value"), &input);
        assert_eq!(reserve.text.as_deref(), Some("£ 4.0M"));
        assert_eq!(reserve.text_color, Some(FIN_INFO));

        // 债务/GDP = 60% -> WARN。
        let debt = profile.bind_node(&kpi_path("kpi_debt", "kpi_value"), &input);
        assert_eq!(debt.text.as_deref(), Some("60.0%"));
        assert_eq!(debt.text_color, Some(FIN_WARN));

        // MEFO/GDP = 20% -> WARN。
        let mefo = profile.bind_node(&kpi_path("kpi_mefo", "kpi_value"), &input);
        assert_eq!(mefo.text.as_deref(), Some("20.0%"));
        assert_eq!(mefo.text_color, Some(FIN_WARN));

        // 评级：AAA -> 绿。
        let rating = profile.bind_node(&kpi_path("kpi_rating", "kpi_value"), &input);
        assert_eq!(rating.text.as_deref(), Some("AAA"));
        assert_eq!(rating.text_color, Some(Color32::from_rgb(0x60, 0xc0, 0x60)));
    }

    fn tab_path(tab: FinanceTab, leaf: &str) -> GuiNodePath {
        GuiNodePath::root(COUNTRY_FINANCE_ROOT)
            .child("tab_bar")
            .child(format!("tab_{}", tab.id()))
            .child(leaf)
    }

    fn body_path(tab: FinanceTab) -> GuiNodePath {
        GuiNodePath::root(COUNTRY_FINANCE_ROOT).child(format!("tab_body_{}", tab.id()))
    }

    #[test]
    fn finance_tab_visibility() {
        let profile = FinanceVanillaProfile;

        for active in FinanceTab::ALL {
            let input = FinanceVanillaInput::new(
                FinancePanelData::default(),
                active,
                FinanceSector::Primary,
            );

            // 仅 active tab 的 body 可见，其余 false。
            for body in FinanceTab::ALL {
                let binding = profile.bind_node(&body_path(body), &input);
                assert_eq!(
                    binding.visible,
                    Some(body == active),
                    "tab_body_{} visible while active={}",
                    body.id(),
                    active.id()
                );
            }

            // active/非 active 都用真页签 sprite GFX_tab_intel_ledger，靠 frame 区分；
            // active frame=2（选中）/ 非 active frame=1（未选中）；label 颜色作辅助强调。
            let active_bg = profile.bind_node(&tab_path(active, "tab_bg"), &input);
            assert_eq!(active_bg.sprite.as_deref(), Some("GFX_tab_intel_ledger"));
            assert_eq!(active_bg.frame, Some(2));
            assert_eq!(active_bg.tint, None);
            let active_label = profile.bind_node(&tab_path(active, "label"), &input);
            assert_eq!(active_label.text.as_deref(), Some(active.label()));
            assert_eq!(active_label.text_color, Some(FIN_GOLD));

            let other = FinanceTab::ALL
                .into_iter()
                .find(|t| *t != active)
                .expect("another tab");
            let other_bg = profile.bind_node(&tab_path(other, "tab_bg"), &input);
            assert_eq!(other_bg.sprite.as_deref(), Some("GFX_tab_intel_ledger"));
            assert_eq!(other_bg.frame, Some(1));
            assert_eq!(other_bg.tint, None);
            let other_label = profile.bind_node(&tab_path(other, "label"), &input);
            assert_eq!(other_label.text_color, Some(FIN_TEXT));

            // hit 始终绑 finance:tab:<id> 点击命令。
            let hit = profile.bind_node(&tab_path(other, "hit"), &input);
            assert_eq!(
                hit.click.as_ref().map(|c| c.command.as_str()),
                Some(format!("finance:tab:{}", other.id()).as_str())
            );
        }
    }

    fn overview_path(leaf: &str) -> GuiNodePath {
        GuiNodePath::root(COUNTRY_FINANCE_ROOT)
            .child("tab_body_overview")
            .child(leaf)
    }

    #[test]
    fn finance_bind_overview() {
        let profile = FinanceVanillaProfile;
        let mut data = FinancePanelData::default();
        data.cash_rm = 12_000_000.0;
        data.reserve_gbp = 4_000_000.0;
        data.gold_kg = 80_000.0;
        data.daily_income_rm = 5_000_000.0;
        data.daily_expense_rm = 8_000_000.0; // 日净额 = -3.0M -> 赤字
        data.operating_income_rm = 6_000_000.0;
        data.operating_expense_rm = 5_000_000.0; // 经营结余 +1.0M
        data.post_financing_cash_change_rm = -2_000_000.0;
        data.gdp_rm = 100_000_000.0;
        data.public_debt_rm = 30_000_000.0;
        data.mefo_debt_rm = 20_000_000.0; // 债务/GDP = 50%，MEFO/GDP = 20%
        data.budget_breakdown.expense_military_wages_rm = 1_000_000.0;
        data.budget_breakdown.expense_military_procurement_rm = 2_000_000.0;
        data.budget_breakdown.expense_military_maintenance_rm = 1_000_000.0; // 军费 4.0M
        data.budget_breakdown.expense_construction_goods_rm = 1_500_000.0;
        data.budget_breakdown.expense_construction_wages_rm = 500_000.0; // 建造 2.0M
        data.budget_breakdown.expense_debt_interest_rm = 750_000.0;
        let input = FinanceVanillaInput::new(data, FinanceTab::Overview, FinanceSector::Primary);

        let bind = |leaf: &str| profile.bind_node(&overview_path(leaf), &input);

        // treasury KV。
        assert_eq!(bind("tv_cash").text.as_deref(), Some("12.0M"));
        assert_eq!(bind("tv_cash").text_color, Some(FIN_GOOD));
        assert_eq!(bind("tv_reserve").text.as_deref(), Some("£ 4.0M"));
        assert_eq!(bind("tv_gold").text.as_deref(), Some("80.0K kg"));
        // 现金覆盖 = 12M / 8M = 1.5 天 -> WARN。
        assert_eq!(bind("tv_cover").text.as_deref(), Some("2 天"));
        assert_eq!(bind("tv_cover").text_color, Some(FIN_WARN));
        // 经营结余 +1.0M。
        assert_eq!(bind("tv_oper").text.as_deref(), Some("+1.0M"));
        assert_eq!(bind("tv_oper").text_color, Some(FIN_GOOD));
        // 融资后现金变动 -2.0M。
        assert_eq!(bind("tv_postfin").text.as_deref(), Some("-2.0M"));
        assert_eq!(bind("tv_postfin").text_color, Some(FIN_BAD));

        // 进度条比率 = 债务/GDP = 0.5、MEFO/GDP = 0.2。
        assert_eq!(bind("debt_bar").progress, Some(0.5));
        assert_eq!(bind("tv_debt_ratio").text.as_deref(), Some("50.0%"));
        assert_eq!(bind("tv_debt_ratio").text_color, Some(FIN_WARN));
        assert_eq!(bind("mefo_bar").progress, Some(0.2));
        assert_eq!(bind("tv_mefo_ratio").text.as_deref(), Some("20.0%"));
        assert_eq!(bind("tv_mefo_ratio").text_color, Some(FIN_WARN));

        // 压力三块。
        assert_eq!(bind("tv_pressure_military").text.as_deref(), Some("4.0M"));
        assert_eq!(
            bind("tv_pressure_construction").text.as_deref(),
            Some("2.0M")
        );
        assert_eq!(bind("tv_pressure_debt").text.as_deref(), Some("750.0K"));

        // 风险横幅：赤字 + MEFO/GDP=20% < 30%，债务=50% >= 45% -> 中风险 WARN。
        let risk = bind("risk_banner");
        assert_eq!(risk.text_color, Some(FIN_WARN));
        assert!(risk.text.as_deref().unwrap_or_default().contains("中风险"));

        // 高风险门限：赤字 + MEFO/GDP >= 30%。
        let mut high = FinancePanelData::default();
        high.daily_expense_rm = 1.0; // 制造赤字
        high.gdp_rm = 100_000_000.0;
        high.mefo_debt_rm = 35_000_000.0; // MEFO/GDP = 35%
        let high_input =
            FinanceVanillaInput::new(high, FinanceTab::Overview, FinanceSector::Primary);
        let high_risk = profile.bind_node(&overview_path("risk_banner"), &high_input);
        assert_eq!(high_risk.text_color, Some(FIN_BAD));
        assert!(high_risk
            .text
            .as_deref()
            .unwrap_or_default()
            .contains("高风险"));
    }

    fn debt_path(leaf: &str) -> GuiNodePath {
        GuiNodePath::root(COUNTRY_FINANCE_ROOT)
            .child("tab_body_debt")
            .child(leaf)
    }

    fn exchange_path(leaf: &str) -> GuiNodePath {
        GuiNodePath::root(COUNTRY_FINANCE_ROOT)
            .child("tab_body_exchange")
            .child(leaf)
    }

    #[test]
    fn finance_bind_debt_exchange() {
        let profile = FinanceVanillaProfile;
        let mut data = FinancePanelData::default();
        data.public_debt_rm = 40_000_000.0;
        data.public_debt_gbp = 2_000_000.0;
        data.gdp_rm = 100_000_000.0;
        data.mefo_debt_rm = 20_000_000.0; // MEFO/GDP = 20%
        data.credit_rating = "BBB".to_owned();
        data.bond_interest_rate = 0.035;
        data.gold_kg = 50_000.0;
        data.reserve_gbp = 3_000_000.0;
        data.exchange_rate_rm_per_gbp = 12.5;
        data.gdp_breakdown.net_exports_rm = -1_000_000.0;
        // 门控起点：全部允许。
        data.daily_income_rm = 1_000_000.0;
        data.daily_expense_rm = 5_000_000.0; // 赤字 4.0M
        data.can_print_mefo = true;
        data.can_issue_foreign_bond = true;
        data.is_foreign_exchange_control = false;
        let input = FinanceVanillaInput::new(data, FinanceTab::Debt, FinanceSector::Primary);

        // 债务 KV。
        let public_debt = profile.bind_node(&debt_path("tv_public_debt"), &input);
        assert_eq!(public_debt.text.as_deref(), Some("40.0M"));
        let mefo_debt = profile.bind_node(&debt_path("tv_mefo_debt"), &input);
        assert_eq!(mefo_debt.text.as_deref(), Some("20.0M (20.0%)"));
        let rating = profile.bind_node(&debt_path("tv_debt_rating"), &input);
        assert_eq!(rating.text.as_deref(), Some("BBB"));
        let interest = profile.bind_node(&debt_path("tv_interest"), &input);
        assert_eq!(interest.text.as_deref(), Some("3.50%"));

        // 动作按钮：全部门控满足 -> enabled + 正确命令。
        let domestic = profile.bind_node(&debt_path("act_issue_domestic"), &input);
        assert_eq!(domestic.enabled, Some(true));
        assert_eq!(
            domestic.click.as_ref().map(|c| c.command.as_str()),
            Some("finance:action:issue_domestic")
        );
        let foreign = profile.bind_node(&debt_path("act_issue_foreign"), &input);
        assert_eq!(foreign.enabled, Some(true));
        assert_eq!(
            foreign.click.as_ref().map(|c| c.command.as_str()),
            Some("finance:action:issue_foreign")
        );
        let mefo = profile.bind_node(&debt_path("act_print_mefo"), &input);
        assert_eq!(mefo.enabled, Some(true));
        assert_eq!(
            mefo.click.as_ref().map(|c| c.command.as_str()),
            Some("finance:action:print_mefo")
        );

        // 门控翻转：禁外债、外汇管制。
        let mut blocked = input.data.clone();
        blocked.can_issue_foreign_bond = false;
        blocked.is_foreign_exchange_control = true;
        let blocked_input =
            FinanceVanillaInput::new(blocked, FinanceTab::Debt, FinanceSector::Primary);
        let foreign_off = profile.bind_node(&debt_path("act_issue_foreign"), &blocked_input);
        assert_eq!(foreign_off.enabled, Some(false));

        // MEFO 门控：mefo/gdp >= 30% 时关闭。
        let mut mefo_over = input.data.clone();
        mefo_over.mefo_debt_rm = 35_000_000.0; // 35% >= 30%
        let mefo_over_input =
            FinanceVanillaInput::new(mefo_over, FinanceTab::Debt, FinanceSector::Primary);
        let mefo_off = profile.bind_node(&debt_path("act_print_mefo"), &mefo_over_input);
        assert_eq!(mefo_off.enabled, Some(false));

        // 外汇 KV。
        let forex_rate = profile.bind_node(&exchange_path("tv_forex_rate"), &input);
        assert_eq!(forex_rate.text.as_deref(), Some("12.50 RM/£"));
        let control = profile.bind_node(&exchange_path("tv_forex_control"), &input);
        assert_eq!(control.text.as_deref(), Some("自由兑换"));
        assert_eq!(control.text_color, Some(FIN_GOOD));
        let net_exports = profile.bind_node(&exchange_path("tv_net_exports"), &input);
        assert_eq!(net_exports.text.as_deref(), Some("-1.0M"));
        assert_eq!(net_exports.text_color, Some(FIN_BAD));

        // 外汇动作门控：卖金 gold>0 -> on；买汇 !control -> on。
        let sell_gold = profile.bind_node(&exchange_path("act_sell_gold"), &input);
        assert_eq!(sell_gold.enabled, Some(true));
        assert_eq!(
            sell_gold.click.as_ref().map(|c| c.command.as_str()),
            Some("finance:action:sell_gold")
        );
        let buy_forex = profile.bind_node(&exchange_path("act_buy_forex"), &input);
        assert_eq!(buy_forex.enabled, Some(true));

        // 卖金门控翻转：gold=0 -> off。
        let mut no_gold = input.data.clone();
        no_gold.gold_kg = 0.0;
        no_gold.is_foreign_exchange_control = true; // 同时关买汇
        let no_gold_input =
            FinanceVanillaInput::new(no_gold, FinanceTab::Exchange, FinanceSector::Primary);
        let sell_off = profile.bind_node(&exchange_path("act_sell_gold"), &no_gold_input);
        assert_eq!(sell_off.enabled, Some(false));
        let buy_off = profile.bind_node(&exchange_path("act_buy_forex"), &no_gold_input);
        assert_eq!(buy_off.enabled, Some(false));

        // handle_action：5 个动作映射到正确 FinanceCommand。
        let issue_domestic = profile.handle_action(
            GuiAction {
                node_path: GuiNodePath::root("finance:action:issue_domestic"),
                kind: GuiActionKind::Click,
            },
            &input,
        );
        assert_eq!(
            issue_domestic,
            Some(FinanceCommand::IssueDomesticBond {
                amount_rm: 500_000_000.0
            })
        );
        let sell = profile.handle_action(
            GuiAction {
                node_path: GuiNodePath::root("finance:action:sell_gold"),
                kind: GuiActionKind::Click,
            },
            &input,
        );
        assert_eq!(
            sell,
            Some(FinanceCommand::SellGold {
                kg: input.data.gold_kg * 0.1
            })
        );
    }

    fn funding_path(leaf: &str) -> GuiNodePath {
        GuiNodePath::root(COUNTRY_FINANCE_ROOT)
            .child("tab_body_funding")
            .child(leaf)
    }

    #[test]
    fn finance_bind_funding() {
        let profile = FinanceVanillaProfile;
        let mut data = FinancePanelData::default();
        data.construction_funding.government_rm = 10_000_000.0;
        data.construction_funding.mefo_rm = 5_000_000.0;
        data.construction_funding.private_pool_rm = 3_000_000.0;
        data.construction_funding.cartel_pool_rm = 2_000_000.0;
        data.construction_funding.overlord_investment_rm = 1_000_000.0;
        data.construction_funding.foreign_investment_rm = 4_000_000.0;
        data.construction_funding.paid_rm = 8_000_000.0;
        data.construction_funding.remaining_rm = 17_000_000.0;
        data.construction_funding.active_projects = 7;
        // total_budget_rm = 25.0M（6 个来源求和）
        data.investment_pool.private_rm = 6_000_000.0;
        data.investment_pool.cartel_rm = 2_500_000.0;
        data.investment_pool.state_development_bank_rm = 4_000_000.0;
        data.investment_pool.colonial_extraction_rm = 1_500_000.0;
        data.investment_pool.foreign_capital_rm = 3_000_000.0;
        data.investment_pool.income_rm = 900_000.0;
        data.investment_pool.spent_rm = 7_000_000.0;
        data.investment_pool.total_rm = 17_000_000.0;
        let input = FinanceVanillaInput::new(data, FinanceTab::Funding, FinanceSector::Primary);
        let bind = |leaf: &str| profile.bind_node(&funding_path(leaf), &input);

        // 建造资金路径 9 项 + 预算合计。
        assert_eq!(bind("tv_cf_government").text.as_deref(), Some("10.0M"));
        assert_eq!(bind("tv_cf_mefo").text.as_deref(), Some("5.0M"));
        assert_eq!(bind("tv_cf_private").text.as_deref(), Some("3.0M"));
        assert_eq!(bind("tv_cf_cartel").text.as_deref(), Some("2.0M"));
        assert_eq!(bind("tv_cf_overlord").text.as_deref(), Some("1.0M"));
        assert_eq!(bind("tv_cf_foreign").text.as_deref(), Some("4.0M"));
        assert_eq!(bind("tv_cf_paid").text.as_deref(), Some("8.0M"));
        assert_eq!(bind("tv_cf_remaining").text.as_deref(), Some("17.0M"));
        assert_eq!(bind("tv_cf_projects").text.as_deref(), Some("7"));
        assert_eq!(bind("tv_cf_total").text.as_deref(), Some("25.0M"));

        // 投资池 8 项。
        assert_eq!(bind("tv_ip_private").text.as_deref(), Some("6.0M"));
        assert_eq!(bind("tv_ip_cartel").text.as_deref(), Some("2.5M"));
        assert_eq!(bind("tv_ip_sdb").text.as_deref(), Some("4.0M"));
        assert_eq!(bind("tv_ip_colonial").text.as_deref(), Some("1.5M"));
        assert_eq!(bind("tv_ip_foreign").text.as_deref(), Some("3.0M"));
        assert_eq!(bind("tv_ip_income").text.as_deref(), Some("+900.0K"));
        assert_eq!(bind("tv_ip_spent").text.as_deref(), Some("7.0M"));
        assert_eq!(bind("tv_ip_total").text.as_deref(), Some("17.0M"));
    }

    #[test]
    fn finance_gui_root() {
        let gui = include_str!("../assets/interface/countryfinanceview.gui");
        let doc = crate::vanilla_gui::parse_gui_str(None, gui);

        let root = doc
            .template_index()
            .get(COUNTRY_FINANCE_ROOT)
            .expect("root template present");

        for name in [
            "finance_title",
            "close_button",
            "kpi_strip",
            "kpi_cash",
            "kpi_net",
            "kpi_reserve",
            "kpi_debt",
            "kpi_mefo",
            "kpi_rating",
            "tab_bar",
            "tab_overview",
            "tab_budget",
            "tab_debt",
            "tab_exchange",
            "tab_economy",
            "tab_funding",
            "finance_footer",
        ] {
            assert!(
                root.find_node_by_name(name).is_some(),
                "missing shell node `{name}`"
            );
        }
    }

    #[test]
    fn finance_gui_bodies() {
        let gui = include_str!("../assets/interface/countryfinanceview.gui");
        let doc = crate::vanilla_gui::parse_gui_str(None, gui);

        let root = doc
            .template_index()
            .get(COUNTRY_FINANCE_ROOT)
            .expect("root template present");

        for name in [
            // six co-located tab bodies
            "tab_body_overview",
            "tab_body_budget",
            "tab_body_debt",
            "tab_body_exchange",
            "tab_body_economy",
            "tab_body_funding",
            // overview blocks
            "treasury_block",
            "debt_bar",
            "mefo_bar",
            "pressure_row",
            "risk_banner",
            "diagnostics_box",
            "diagnostics_grid",
            // budget blocks
            "budget_headers",
            "budget_income_scroll",
            "budget_income_grid",
            "budget_expense_scroll",
            "budget_expense_grid",
            "budget_totals",
            // debt blocks
            "debt_metrics",
            "financing_block",
            "debt_actions",
            // exchange blocks
            "forex_metrics",
            "trade_impact",
            "exchange_actions",
            // economy blocks
            "economy_scroll",
            "gdp_totals",
            "gdp_box",
            "gdp_grid",
            "sector_selector",
            "sector_box",
            "sector_grid",
            "employment_box",
            "employment_grid",
            // funding blocks
            "construction_block",
            "investment_block",
        ] {
            assert!(
                root.find_node_by_name(name).is_some(),
                "missing body node `{name}`"
            );
        }
    }

    #[test]
    fn finance_gui_templates() {
        let gui = include_str!("../assets/interface/countryfinanceview.gui");
        let doc = crate::vanilla_gui::parse_gui_str(None, gui);
        let index = doc.template_index();

        for (template, cells) in [
            (
                "finance_budget_row",
                &["row_hit", "dir_icon", "label", "amount", "row_rule"][..],
            ),
            (
                "finance_gdp_row",
                &["label", "amount", "percent", "row_rule"][..],
            ),
            (
                "finance_sector_row",
                &[
                    "building", "level", "employ", "fill_bar", "value", "row_rule",
                ][..],
            ),
            (
                "finance_employment_row",
                &["label", "employed", "demand", "rate", "value", "row_rule"][..],
            ),
            (
                "finance_diagnostic_row",
                &["source", "status", "detail", "row_rule"][..],
            ),
        ] {
            let node = index
                .get(template)
                .unwrap_or_else(|| panic!("missing row template `{template}`"));
            for cell in cells {
                assert!(
                    node.find_node_by_name(cell).is_some(),
                    "template `{template}` missing cell `{cell}`"
                );
            }
        }
    }

    #[test]
    fn finance_row_routing() {
        let profile = FinanceVanillaProfile;
        let mut data = FinancePanelData::default();
        data.fiscal_revenue.trade_tariffs_rm = 2_000_000.0;
        data.fiscal_revenue.financing_rm = 8_000_000.0;
        data.fiscal_expense.military_rm = 18_000_000.0;
        data.fiscal_expense.construction_rm = 12_000_000.0;
        data.fiscal_expense.interest_rm = 2_000_000.0;
        data.fiscal_expense.welfare_rm = 4_000_000.0;
        let input = FinanceVanillaInput::new(data, FinanceTab::Budget, FinanceSector::Primary);

        // 预算行 row_hit 命令：可路由行绑 finance:goto:*，无路由行不绑命令。
        let row_command = |key: &str| {
            let ctx = crate::vanilla_gui::GuiInstanceContext::new(
                "finance_budget_row",
                0,
                GuiNodePath::root("budget_income_grid"),
            )
            .with_semantic_role("finance_budget_row")
            .with_model_key(key);
            profile
                .bind_node_with_context(
                    &GuiNodePath::root("finance_budget_row[0]").child("row_hit"),
                    &input,
                    Some(&ctx),
                )
                .click
                .map(|click| click.command)
        };
        assert_eq!(
            row_command("budget:inc:tariffs").as_deref(),
            Some("finance:goto:trade")
        );
        assert_eq!(
            row_command("budget:inc:financing").as_deref(),
            Some("finance:goto:debt")
        );
        assert_eq!(
            row_command("budget:exp:military").as_deref(),
            Some("finance:goto:logistics")
        );
        assert_eq!(
            row_command("budget:exp:construction").as_deref(),
            Some("finance:goto:construction")
        );
        assert_eq!(
            row_command("budget:exp:interest").as_deref(),
            Some("finance:goto:debt")
        );
        // 无路由行（福利）：row_hit 不发命令。
        assert_eq!(row_command("budget:exp:welfare"), None);

        // handle_action 把 finance:goto:* 映射到对应 Panel 命令。
        let goto = |command: &str| {
            profile.handle_action(
                GuiAction {
                    node_path: GuiNodePath::root(command),
                    kind: GuiActionKind::Click,
                },
                &input,
            )
        };
        assert_eq!(
            goto("finance:goto:trade"),
            Some(FinanceCommand::Panel(PanelCommand::OpenPrimary(
                ActivePrimaryPanel::Trade
            )))
        );
        assert_eq!(
            goto("finance:goto:logistics"),
            Some(FinanceCommand::Panel(PanelCommand::OpenPrimary(
                ActivePrimaryPanel::Logistics
            )))
        );
        assert_eq!(
            goto("finance:goto:construction"),
            Some(FinanceCommand::Panel(PanelCommand::OpenPrimary(
                ActivePrimaryPanel::Construction
            )))
        );
        assert_eq!(
            goto("finance:goto:debt"),
            Some(FinanceCommand::Panel(PanelCommand::OpenDetail(
                ActiveDetailPanel::FinanceDebt
            )))
        );
    }

    #[test]
    fn finance_sector_button_selected_state_and_click() {
        let profile = FinanceVanillaProfile;
        let input = FinanceVanillaInput::new(
            FinancePanelData::default(),
            FinanceTab::Economy,
            FinanceSector::Secondary,
        );
        let button = |name: &str| {
            profile.bind_node(
                &GuiNodePath::root(COUNTRY_FINANCE_ROOT)
                    .child("tab_body_economy")
                    .child("economy_scroll")
                    .child("sector_selector")
                    .child(name),
                &input,
            )
        };
        // 选中（二产）金色 tint，未选中暗 tint；两者同 GFX_tiled_button；click 走 finance:sector:*。
        let secondary = button("sector_secondary");
        assert_eq!(secondary.sprite.as_deref(), Some("GFX_tiled_button"));
        assert_eq!(secondary.tint, Some(FIN_TAB_SELECTED));
        assert_eq!(
            secondary.click.as_ref().map(|c| c.command.as_str()),
            Some("finance:sector:secondary")
        );
        let primary = button("sector_primary");
        assert_eq!(primary.sprite.as_deref(), Some("GFX_tiled_button"));
        assert_eq!(primary.tint, Some(FIN_TAB_IDLE));
        assert_eq!(
            primary.click.as_ref().map(|c| c.command.as_str()),
            Some("finance:sector:primary")
        );
    }
}
