//! Phase B menu rendering for the V9 frontend.
//!
//! The app still renders pre-game screens through the lightweight `PanelPass`
//! and `TextPass` path, but dimensions, colors, and layout now come from the
//! V9 token system. The public structs are kept stable for `main.rs` hit tests.

use crate::panel_pass::{Panel, PanelPass};
use crate::text_pass::{TextAlign, TextPass, TextSize};
use hoi4_ui::egui::{Color32, Pos2, Rect, Vec2};
use hoi4_ui::i18n::{current_language, tr, Language};
use hoi4_ui::v9::{
    layout::{GridLayout, Track},
    palette, spacing,
};

#[derive(Debug, Clone)]
pub struct MenuButton {
    pub id: &'static str,
    pub label: String,
    pub rect: (f32, f32, f32, f32),
    pub enabled: bool,
}

impl MenuButton {
    pub fn contains(&self, mx: f32, my: f32) -> bool {
        let (x, y, w, h) = self.rect;
        mx >= x && mx < x + w && my >= y && my < y + h
    }
}

#[derive(Debug, Clone)]
pub struct CountryEntry {
    pub tag: String,
    pub name: String,
    pub ideology: String,
    pub enabled: bool,
}

pub struct MainMenuLayout {
    pub buttons: Vec<MenuButton>,
}

pub struct CountrySelectLayout {
    pub list_rows: Vec<((f32, f32, f32, f32), usize)>,
    pub start_button: MenuButton,
    pub back_button: MenuButton,
    pub flag_rect: (f32, f32, f32, f32),
    pub detail_rect: (f32, f32, f32, f32),
}

pub fn draw_main_menu(
    panels: &mut PanelPass,
    text: &mut TextPass,
    sw: f32,
    sh: f32,
    hovered_id: Option<&str>,
) -> MainMenuLayout {
    panels.push(Panel::full_screen_dim(sw, sh, 0.66));
    panels.push(Panel::solid(
        0.0,
        0.0,
        sw,
        34.0,
        rgba_f32(0.0, 0.0, 0.0, 0.46),
    ));
    panels.push(Panel::solid(
        0.0,
        sh - 40.0,
        sw,
        40.0,
        rgba_f32(0.0, 0.0, 0.0, 0.50),
    ));

    let margin_x = if sw >= 1280.0 {
        spacing::S8
    } else {
        spacing::S5
    };
    let margin_y = if sh >= 720.0 {
        spacing::S9
    } else {
        spacing::S5
    };
    let shell_w = (sw - margin_x * 2.0).max(720.0);
    let shell_h = (sh - margin_y * 2.0).max(520.0);
    let shell = Rect::from_min_size(
        Pos2::new((sw - shell_w) * 0.5, (sh - shell_h) * 0.5),
        Vec2::new(shell_w, shell_h),
    );
    panels.push(v9_panel(shell, 8.0));
    paint_plate_lines(panels, shell);

    let inner = shell.shrink2(Vec2::new(spacing::S8, spacing::S7));
    let grid = GridLayout::new(
        vec![Track::Fixed(150.0), Track::Fr(1.0), Track::Fixed(36.0)],
        vec![Track::Fr(0.58), Track::Fr(0.42)],
    )
    .with_gutter(spacing::S8, spacing::S6);
    let cells = grid.measure(inner);
    let title_rect = GridLayout::span(&cells, 0, 0, 1, 2);
    let menu_rect = GridLayout::cell(&cells, 1, 0);
    let briefing_rect = GridLayout::cell(&cells, 1, 1);
    let footer_rect = GridLayout::span(&cells, 2, 0, 1, 2);

    text.draw_text_sized(
        "HEARTS OF IRON IV",
        title_rect.left(),
        title_rect.top() + 8.0,
        TextAlign::Left,
        title_rect.width(),
        TextSize::Title,
    );
    panels.push(Panel::header_underline(
        title_rect.left(),
        title_rect.top() + 76.0,
        180.0,
        3.0,
    ));
    text.draw_text_aligned(
        "Project Ironheart Engine",
        title_rect.left(),
        title_rect.top() + 92.0,
        TextAlign::Left,
        title_rect.width(),
    );
    text.draw_text_aligned(
        "1936 Campaign Prototype",
        title_rect.left(),
        title_rect.top() + 120.0,
        TextAlign::Left,
        title_rect.width(),
    );

    panels.push(v9_recessed(briefing_rect));
    text.draw_text_sized(
        ui_text("CAMPAIGN BRIEF", "战役简报"),
        briefing_rect.left() + spacing::S6,
        briefing_rect.top() + spacing::S5,
        TextAlign::Left,
        briefing_rect.width() - spacing::S7,
        TextSize::Heading,
    );
    panels.push(Panel::header_underline(
        briefing_rect.left() + spacing::S6,
        briefing_rect.top() + 52.0,
        96.0,
        2.0,
    ));
    for (idx, line) in [
        ui_text("Europe is rearming.", "欧洲正在重新武装。"),
        ui_text(
            "Industry, politics, and command are live.",
            "工业、政治与指挥系统已启用。",
        ),
        ui_text(
            "Choose a nation and enter the 1936 scenario.",
            "选择国家并进入 1936 年剧本。",
        ),
    ]
    .iter()
    .enumerate()
    {
        text.draw_text_aligned(
            line,
            briefing_rect.left() + spacing::S6,
            briefing_rect.top() + 76.0 + idx as f32 * 28.0,
            TextAlign::Left,
            briefing_rect.width() - spacing::S7,
        );
    }

    let btn_ids: [&'static str; 4] = ["btn_new_game", "btn_continue", "btn_settings", "btn_quit"];
    let btn_labels: [&str; 4] = [
        tr("new_game"),
        tr("continue_game"),
        tr("settings_btn"),
        tr("quit"),
    ];
    let btn_enabled = [true, false, true, true];
    let row_h = 48.0;
    let row_gap = spacing::S5;
    let btn_w = menu_rect.width().min(460.0);
    let btn_x = menu_rect.left();
    let mut btn_y = menu_rect.top() + spacing::S4;

    let mut buttons = Vec::with_capacity(btn_ids.len());
    for i in 0..btn_ids.len() {
        let id = btn_ids[i];
        let enabled = btn_enabled[i];
        let hovered = hovered_id == Some(id) && enabled;
        let rect = Rect::from_min_size(Pos2::new(btn_x, btn_y), Vec2::new(btn_w, row_h));
        panels.push(v9_button(rect, enabled));
        if hovered {
            panels.push(Panel::hover_highlight(
                rect.left(),
                rect.top(),
                rect.width(),
                rect.height(),
            ));
            panels.push(Panel::solid(
                rect.left(),
                rect.top(),
                5.0,
                rect.height(),
                rgba(palette::GOLD_HOT, 0.78),
            ));
        }

        let prefix = if hovered { "> " } else { "  " };
        text.draw_text_sized(
            &format!("{}{}", prefix, btn_labels[i]),
            rect.left() + spacing::S6,
            rect.top() + 7.0,
            TextAlign::Left,
            rect.width() - spacing::S7,
            TextSize::Heading,
        );

        buttons.push(MenuButton {
            id,
            label: btn_labels[i].to_owned(),
            rect: (rect.left(), rect.top(), rect.width(), rect.height()),
            enabled,
        });
        btn_y += row_h + row_gap;
    }

    text.draw_text_aligned(
        ui_text(
            "ENTER Select country     ESC Quit     F5 Toggle postprocess",
            "ENTER 选择国家     ESC 退出     F5 切换后处理",
        ),
        footer_rect.left(),
        footer_rect.top() + 6.0,
        TextAlign::Center,
        footer_rect.width(),
    );

    MainMenuLayout { buttons }
}

#[allow(clippy::too_many_arguments)]
pub fn draw_country_select(
    panels: &mut PanelPass,
    text: &mut TextPass,
    sw: f32,
    sh: f32,
    countries: &[CountryEntry],
    selected_idx: usize,
    hovered_button_id: Option<&str>,
    hovered_row: Option<usize>,
) -> CountrySelectLayout {
    panels.push(Panel::full_screen_dim(sw, sh, 0.58));

    text.draw_text_sized(
        tr("select_country"),
        0.0,
        38.0,
        TextAlign::Center,
        sw,
        TextSize::Title,
    );
    panels.push(Panel::header_underline(sw * 0.5 - 92.0, 108.0, 184.0, 2.0));
    text.draw_text_aligned(
        ui_text(
            "UP/DOWN Navigate     ENTER Confirm     ESC Back",
            "上下键 导航     ENTER 确认     ESC 返回",
        ),
        0.0,
        124.0,
        TextAlign::Center,
        sw,
    );

    let margin_x = if sw >= 1280.0 {
        spacing::S8
    } else {
        spacing::S5
    };
    let total_w = (sw - margin_x * 2.0).max(1040.0);
    let panel_y = if sh >= 720.0 { 150.0 } else { 132.0 };
    let bottom_h = 44.0 + spacing::S7;
    let panel_h = (sh - panel_y - bottom_h - spacing::S7).max(420.0);
    let grid_rect = Rect::from_min_size(
        Pos2::new((sw - total_w) * 0.5, panel_y),
        Vec2::new(total_w, panel_h),
    );
    let grid = GridLayout::new(
        vec![Track::Fixed(panel_h)],
        vec![Track::Fr(0.28), Track::Fr(0.32), Track::Fr(0.40)],
    )
    .with_gutter(spacing::S7, 0.0);
    let cells = grid.measure(grid_rect);
    let list_rect = GridLayout::cell(&cells, 0, 0);
    let center_rect = GridLayout::cell(&cells, 0, 1);
    let detail_rect = GridLayout::cell(&cells, 0, 2);

    for rect in [list_rect, center_rect, detail_rect] {
        panels.push(v9_card(rect));
        paint_plate_lines(panels, rect);
    }

    draw_column_title(panels, text, list_rect, tr("countries"), 80.0);
    let list_rows = draw_country_rows(
        panels,
        text,
        list_rect,
        countries,
        selected_idx,
        hovered_row,
    );

    let flag_rect = draw_country_focus(panels, text, center_rect, countries.get(selected_idx));
    draw_column_title(panels, text, detail_rect, tr("overview"), 82.0);

    let btn_w = 204.0;
    let btn_h = 44.0;
    let bottom_y = panel_y + panel_h + spacing::S7;
    let back_rect = Rect::from_min_size(
        Pos2::new(list_rect.center().x - btn_w * 0.5, bottom_y),
        Vec2::new(btn_w, btn_h),
    );
    let start_rect = Rect::from_min_size(
        Pos2::new(detail_rect.center().x - btn_w * 0.5, bottom_y),
        Vec2::new(btn_w, btn_h),
    );
    let start_enabled = countries
        .get(selected_idx)
        .map(|c| c.enabled)
        .unwrap_or(false);
    draw_bottom_button(
        panels,
        text,
        back_rect,
        "btn_back",
        tr("back"),
        true,
        hovered_button_id,
    );
    draw_bottom_button(
        panels,
        text,
        start_rect,
        "btn_start",
        tr("start_game"),
        start_enabled,
        hovered_button_id,
    );

    CountrySelectLayout {
        list_rows,
        start_button: MenuButton {
            id: "btn_start",
            label: tr("start_game").to_owned(),
            rect: (
                start_rect.left(),
                start_rect.top(),
                start_rect.width(),
                start_rect.height(),
            ),
            enabled: start_enabled,
        },
        back_button: MenuButton {
            id: "btn_back",
            label: tr("back").to_owned(),
            rect: (
                back_rect.left(),
                back_rect.top(),
                back_rect.width(),
                back_rect.height(),
            ),
            enabled: true,
        },
        flag_rect: (
            flag_rect.left(),
            flag_rect.top(),
            flag_rect.width(),
            flag_rect.height(),
        ),
        detail_rect: (
            detail_rect.left(),
            detail_rect.top(),
            detail_rect.width(),
            detail_rect.height(),
        ),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn draw_country_detail(
    text: &mut TextPass,
    detail_rect: (f32, f32, f32, f32),
    _name: &str,
    tag: &str,
    ideology: &str,
    manpower: u64,
    civ_factories: u32,
    mil_factories: u32,
    shipyards: u32,
    stability: f32,
    war_support: f32,
) {
    let (x, y, w, _) = detail_rect;
    let inner_x = x + spacing::S7;
    let inner_w = w - spacing::S8 - spacing::S6;
    let mut cy = y + 64.0;

    text.draw_text_sized(
        localized_country_name(tag),
        inner_x,
        cy,
        TextAlign::Left,
        inner_w,
        TextSize::Heading,
    );
    cy += 40.0;
    text.draw_text_aligned(
        &format!(
            "{} {}     {} {}",
            tr("tag"),
            tag,
            tr("ideology"),
            ideology_label(ideology)
        ),
        inner_x,
        cy,
        TextAlign::Left,
        inner_w,
    );
    cy += 42.0;

    cy = draw_stat_block(
        text,
        inner_x,
        cy,
        inner_w,
        tr("manpower"),
        &[(tr("pool"), &format_manpower(manpower))],
    );
    cy += spacing::S5;
    cy = draw_stat_block(
        text,
        inner_x,
        cy,
        inner_w,
        ui_text("INDUSTRY", "工业"),
        &[
            (tr("civ_factories"), &civ_factories.to_string()),
            (tr("mil_factories"), &mil_factories.to_string()),
            (tr("shipyards"), &shipyards.to_string()),
        ],
    );
    cy += spacing::S5;
    let _ = draw_stat_block(
        text,
        inner_x,
        cy,
        inner_w,
        tr("politics"),
        &[
            (tr("stability"), &format!("{:.0}%", stability * 100.0)),
            (tr("war_support"), &format!("{:.0}%", war_support * 100.0)),
        ],
    );
}

fn draw_country_rows(
    panels: &mut PanelPass,
    text: &mut TextPass,
    rect: Rect,
    countries: &[CountryEntry],
    selected_idx: usize,
    hovered_row: Option<usize>,
) -> Vec<((f32, f32, f32, f32), usize)> {
    let row_h = 36.0;
    let row_x = rect.left() + spacing::S5;
    let row_w = rect.width() - spacing::S6;
    let row_top = rect.top() + 58.0;
    let max_visible = ((rect.height() - 78.0) / row_h).max(0.0) as usize;
    let half = max_visible / 2;
    let total = countries.len();
    let start = selected_idx
        .saturating_sub(half)
        .min(total.saturating_sub(max_visible.min(total)));
    let mut rows = Vec::new();

    for visible in 0..max_visible {
        let i = start + visible;
        let Some(entry) = countries.get(i) else {
            break;
        };
        let y = row_top + visible as f32 * row_h;
        let row = Rect::from_min_size(Pos2::new(row_x, y), Vec2::new(row_w, row_h));
        if i == selected_idx {
            panels.push(Panel {
                x: row.left(),
                y: row.top(),
                w: row.width(),
                h: row.height(),
                radius: 3.0,
                fill_top: rgba(palette::GOLD, 0.22),
                fill_bottom: rgba(palette::BRASS_DARK, 0.24),
                border: rgba(palette::GOLD_HOT, 0.62),
                border_width: 1.0,
                shadow_offset: 0.0,
                shadow_alpha: 0.0,
            });
        } else if hovered_row == Some(i) && entry.enabled {
            panels.push(Panel::hover_highlight(
                row.left(),
                row.top(),
                row.width(),
                row.height(),
            ));
        } else if visible % 2 == 1 {
            panels.push(Panel::solid(
                row.left(),
                row.top(),
                row.width(),
                row.height(),
                rgba(palette::SOOT_BLACK, 0.22),
            ));
        }

        panels.push(Panel {
            x: row.left() + spacing::S4,
            y: row.top() + 10.0,
            w: 4.0,
            h: row.height() - 20.0,
            radius: 2.0,
            fill_top: ideology_color(&entry.ideology),
            fill_bottom: ideology_color(&entry.ideology),
            border: [0.0; 4],
            border_width: 0.0,
            shadow_offset: 0.0,
            shadow_alpha: 0.0,
        });

        let suffix = if entry.enabled {
            String::new()
        } else {
            format!("  ({})", tr("locked"))
        };
        text.draw_text_aligned(
            &format!(
                "{}   {}{}",
                entry.tag,
                localized_country_name(&entry.tag),
                suffix
            ),
            row.left() + spacing::S6,
            row.top() + 8.0,
            TextAlign::Left,
            row.width() - spacing::S7,
        );
        rows.push(((row.left(), row.top(), row.width(), row.height()), i));
    }

    rows
}

fn draw_country_focus(
    panels: &mut PanelPass,
    text: &mut TextPass,
    rect: Rect,
    country: Option<&CountryEntry>,
) -> Rect {
    let flag_w = (rect.width() - spacing::S8).min(328.0).max(220.0);
    let flag_h = flag_w * (52.0 / 82.0);
    let flag = Rect::from_min_size(
        Pos2::new(rect.center().x - flag_w * 0.5, rect.top() + 68.0),
        Vec2::new(flag_w, flag_h),
    );
    let frame = flag.expand(6.0);
    panels.push(Panel {
        x: frame.left(),
        y: frame.top(),
        w: frame.width(),
        h: frame.height(),
        radius: 3.0,
        fill_top: rgba(palette::SOOT_BLACK, 0.74),
        fill_bottom: rgba(palette::OIL_BLACK, 0.88),
        border: rgba(palette::GOLD_HOT, 0.66),
        border_width: 2.0,
        shadow_offset: 4.0,
        shadow_alpha: 0.40,
    });

    let (display_name, tag, ideology) = country
        .map(|c| {
            (
                localized_country_name(&c.tag),
                c.tag.as_str(),
                c.ideology.as_str(),
            )
        })
        .unwrap_or((tr("unknown"), "---", ""));
    text.draw_text_sized(
        display_name,
        rect.left() + spacing::S5,
        flag.bottom() + spacing::S7,
        TextAlign::Center,
        rect.width() - spacing::S6,
        TextSize::Heading,
    );
    text.draw_text_aligned(
        tag,
        rect.left() + spacing::S5,
        flag.bottom() + spacing::S7 + 38.0,
        TextAlign::Center,
        rect.width() - spacing::S6,
    );

    let bar_w = (rect.width() - spacing::S9).min(240.0);
    let bar = Rect::from_min_size(
        Pos2::new(
            rect.center().x - bar_w * 0.5,
            flag.bottom() + spacing::S7 + 72.0,
        ),
        Vec2::new(bar_w, 4.0),
    );
    panels.push(Panel {
        x: bar.left(),
        y: bar.top(),
        w: bar.width(),
        h: bar.height(),
        radius: 2.0,
        fill_top: ideology_color(ideology),
        fill_bottom: ideology_color(ideology),
        border: [0.0; 4],
        border_width: 0.0,
        shadow_offset: 0.0,
        shadow_alpha: 0.0,
    });
    text.draw_text_aligned(
        ideology_label(ideology),
        rect.left() + spacing::S5,
        bar.bottom() + spacing::S3,
        TextAlign::Center,
        rect.width() - spacing::S6,
    );

    flag
}

fn draw_column_title(
    panels: &mut PanelPass,
    text: &mut TextPass,
    rect: Rect,
    title: &str,
    underline_w: f32,
) {
    text.draw_text_sized(
        title,
        rect.left() + spacing::S6,
        rect.top() + spacing::S5,
        TextAlign::Left,
        rect.width() - spacing::S7,
        TextSize::Heading,
    );
    panels.push(Panel::header_underline(
        rect.left() + spacing::S6,
        rect.top() + 50.0,
        underline_w,
        2.0,
    ));
}

fn draw_bottom_button(
    panels: &mut PanelPass,
    text: &mut TextPass,
    rect: Rect,
    id: &str,
    label: &str,
    enabled: bool,
    hovered_button_id: Option<&str>,
) {
    let hovered = hovered_button_id == Some(id) && enabled;
    panels.push(v9_button(rect, enabled));
    if hovered {
        panels.push(Panel::hover_highlight(
            rect.left(),
            rect.top(),
            rect.width(),
            rect.height(),
        ));
    }
    let prefix = if hovered { "> " } else { "  " };
    text.draw_text_aligned(
        &format!("{}{}", prefix, label),
        rect.left(),
        rect.top() + 12.0,
        TextAlign::Center,
        rect.width(),
    );
}

fn draw_stat_block(
    text: &mut TextPass,
    x: f32,
    y: f32,
    w: f32,
    title: &str,
    rows: &[(&str, &str)],
) -> f32 {
    text.draw_text_sized(title, x, y, TextAlign::Left, w, TextSize::Heading);
    let mut cy = y + 34.0;
    for (label, value) in rows {
        text.draw_text_aligned(label, x + spacing::S4, cy, TextAlign::Left, w * 0.48);
        text.draw_text_aligned(
            value,
            x + w * 0.52,
            cy,
            TextAlign::Right,
            w * 0.48 - spacing::S4,
        );
        cy += 25.0;
    }
    cy
}

fn format_manpower(mp: u64) -> String {
    if mp >= 1_000_000 {
        format!("{:.2} M", mp as f64 / 1_000_000.0)
    } else if mp >= 1_000 {
        format!("{:.1} K", mp as f64 / 1_000.0)
    } else {
        mp.to_string()
    }
}

pub fn ideology_color(ideology: &str) -> [f32; 4] {
    let color = match ideology {
        "fascism" => palette::IDEO_FASCISM,
        "democratic" => palette::IDEO_DEMOCRATIC,
        "communism" => palette::IDEO_COMMUNISM,
        "neutrality" => palette::IDEO_NEUTRALITY,
        _ => palette::MUTED,
    };
    rgba(color, 0.95)
}

pub fn ideology_label(ideology: &str) -> &'static str {
    match ideology {
        "fascism" => tr("fascism"),
        "democratic" => tr("democratic"),
        "communism" => tr("communism"),
        "neutrality" => tr("neutrality"),
        _ => tr("unknown"),
    }
}

fn ui_text<'a>(english: &'a str, chinese: &'a str) -> &'a str {
    if current_language() == Language::Chinese {
        chinese
    } else {
        english
    }
}

fn localized_country_name(tag: &str) -> &str {
    let translated = tr(tag);
    if translated == tag && tag == tr("unknown") {
        tr("unknown")
    } else {
        translated
    }
}

fn v9_panel(rect: Rect, radius: f32) -> Panel {
    Panel {
        x: rect.left(),
        y: rect.top(),
        w: rect.width(),
        h: rect.height(),
        radius,
        fill_top: rgba(palette::IRON, 0.90),
        fill_bottom: rgba(palette::SOOT_BLACK, 0.96),
        border: rgba(palette::BRASS_DARK, 0.70),
        border_width: 1.5,
        shadow_offset: 10.0,
        shadow_alpha: 0.55,
    }
}

fn v9_card(rect: Rect) -> Panel {
    Panel {
        x: rect.left(),
        y: rect.top(),
        w: rect.width(),
        h: rect.height(),
        radius: 4.0,
        fill_top: rgba(palette::IRON, 0.78),
        fill_bottom: rgba(palette::OIL_BLACK, 0.90),
        border: rgba(palette::BRASS_DARK, 0.50),
        border_width: 1.0,
        shadow_offset: 6.0,
        shadow_alpha: 0.45,
    }
}

fn v9_recessed(rect: Rect) -> Panel {
    Panel {
        x: rect.left(),
        y: rect.top(),
        w: rect.width(),
        h: rect.height(),
        radius: 3.0,
        fill_top: rgba(palette::SOOT_BLACK, 0.48),
        fill_bottom: rgba(palette::OIL_BLACK, 0.72),
        border: rgba(palette::HAIRLINE, 0.68),
        border_width: 1.0,
        shadow_offset: 0.0,
        shadow_alpha: 0.0,
    }
}

fn v9_button(rect: Rect, enabled: bool) -> Panel {
    Panel {
        x: rect.left(),
        y: rect.top(),
        w: rect.width(),
        h: rect.height(),
        radius: 3.0,
        fill_top: if enabled {
            rgba(palette::IRON_DARK, 0.78)
        } else {
            rgba(palette::SOOT_BLACK, 0.46)
        },
        fill_bottom: if enabled {
            rgba(palette::OIL_BLACK, 0.88)
        } else {
            rgba(palette::SOOT_BLACK, 0.54)
        },
        border: if enabled {
            rgba(palette::BRASS_DARK, 0.60)
        } else {
            rgba(palette::HAIRLINE, 0.28)
        },
        border_width: 1.0,
        shadow_offset: 3.0,
        shadow_alpha: 0.28,
    }
}

fn paint_plate_lines(panels: &mut PanelPass, rect: Rect) {
    panels.push(Panel::solid(
        rect.left() + spacing::S5,
        rect.top() + spacing::S5,
        rect.width() - spacing::S6,
        1.0,
        rgba(palette::EDGE_LIGHT, 0.18),
    ));
    panels.push(Panel::solid(
        rect.left() + spacing::S5,
        rect.bottom() - spacing::S5,
        rect.width() - spacing::S6,
        1.0,
        rgba(palette::BRASS_DARK, 0.38),
    ));
}

fn rgba(color: Color32, alpha: f32) -> [f32; 4] {
    [
        color.r() as f32 / 255.0,
        color.g() as f32 / 255.0,
        color.b() as f32 / 255.0,
        alpha.clamp(0.0, 1.0),
    ]
}

fn rgba_f32(r: f32, g: f32, b: f32, a: f32) -> [f32; 4] {
    [r, g, b, a]
}
