use egui_kittest::Harness;
use hoi4_ui::egui::{self, Align2, Color32, Rect, Stroke, Vec2};
use hoi4_ui::v9::{
    accessibility::{self, AccessibilitySettings},
    composites::{
        draw_summary_tiles, draw_tab_strip, MiniMapHud, MiniMapHudData, NotificationStack,
        PanelClass, PanelShell, SideRail, SideRailData,
    },
    layout::{GridLayout, Track},
    primitives::{
        Button, ButtonSize, ButtonVariant, DataTable, Modal, TableCell, TableColumn, TableRow,
        ToastKind,
    },
    tokens::{palette, spacing, TextRole},
};

fn prepare(ctx: &egui::Context) {
    accessibility::set_settings(ctx, AccessibilitySettings::default());
    egui::CentralPanel::default()
        .frame(egui::Frame::NONE.fill(palette::CANVAS_DEEP))
        .show(ctx, |_ui| {});
}

fn snapshot_if_enabled<State>(harness: &mut Harness<'_, State>, name: &str) {
    let enabled = std::env::var_os("RUN_V9_SNAPSHOTS").is_some()
        || std::env::var_os("UPDATE_SNAPSHOTS").is_some();
    if enabled {
        harness.snapshot(name);
    }
}

#[test]
fn v9_visual_snapshot_panel_shell() {
    let mut harness = Harness::builder()
        .with_size(Vec2::new(960.0, 720.0))
        .build(|ctx| {
            prepare(ctx);
            let _ = PanelShell::new("snapshot_panel_shell", "Politics")
                .subtitle("Germany 1936-03-15")
                .class(PanelClass::MilitaryDiplomacy)
                .accent(palette::BRASS_BRIGHT)
                .show(ctx, |ui, layout| {
                    draw_summary_tiles(
                        ui,
                        layout.summary,
                        &[
                            ("Stability", "73%".to_owned(), palette::GOOD),
                            ("War Support", "31%".to_owned(), palette::WARN),
                            ("PP", "220".to_owned(), palette::GOLD),
                        ],
                    );
                    draw_tab_strip(
                        ui,
                        layout.tabs,
                        "Overview / Decisions / Advisors",
                        palette::GOLD,
                    );
                    let body = layout.body.shrink2(Vec2::new(spacing::S4, spacing::S4));
                    DataTable::new(
                        vec![
                            TableColumn::new("Issue", 1.4),
                            TableColumn::new("Status", 0.8).center(),
                            TableColumn::new("Effect", 0.8).right(),
                        ],
                        vec![
                            TableRow::new(vec![
                                TableCell::strong("Cabinet cohesion"),
                                TableCell::colored("stable", palette::GOOD).center(),
                                TableCell::colored("+5%", palette::GOOD).right(),
                            ]),
                            TableRow::new(vec![
                                TableCell::strong("Army modernization"),
                                TableCell::colored("warning", palette::WARN).center(),
                                TableCell::colored("-40 PP", palette::WARN).right(),
                            ]),
                            TableRow::new(vec![
                                TableCell::strong("Foreign pressure"),
                                TableCell::colored("risk", palette::BAD).center(),
                                TableCell::colored("-2%", palette::BAD).right(),
                            ]),
                        ],
                    )
                    .show_at(ui, body);
                });
        });

    harness.run();
    snapshot_if_enabled(&mut harness, "v9_panel_shell");
}

#[test]
fn v9_visual_snapshot_side_rail() {
    let mut harness = Harness::builder()
        .with_size(Vec2::new(420.0, 720.0))
        .build(|ctx| {
            prepare(ctx);
            let data = SideRailData::gameplay(Some(hoi4_ui::PanelKind::Politics))
                .with_badge(hoi4_ui::PanelKind::Situation, 4);
            let _ = SideRail::show(ctx, &data);
        });

    harness.run();
    snapshot_if_enabled(&mut harness, "v9_side_rail");
}

#[test]
fn v9_visual_snapshot_mini_map_hud() {
    let mut harness = Harness::builder()
        .with_size(Vec2::new(760.0, 520.0))
        .build(|ctx| {
            prepare(ctx);
            let data = MiniMapHudData {
                date: "1936-03-15".to_owned(),
                speed_index: 3,
                map_mode: "Political".to_owned(),
                selected_label: Some("Berlin".to_owned()),
            };
            let _ = MiniMapHud::show(ctx, &data);
        });

    harness.run();
    snapshot_if_enabled(&mut harness, "v9_mini_map_hud");
}

#[test]
fn v9_visual_snapshot_notification_stack() {
    let mut harness = Harness::builder()
        .with_size(Vec2::new(760.0, 520.0))
        .build_state(
            |ctx, stack: &mut NotificationStack| {
                prepare(ctx);
                stack.push_keyed(
                    "law",
                    "Law change failed",
                    "Not enough political power.",
                    ToastKind::Bad,
                    4.0,
                );
                stack.push_keyed(
                    "research",
                    "Research complete",
                    "Improved machine tools are available.",
                    ToastKind::Good,
                    4.0,
                );
                stack.show(ctx);
            },
            NotificationStack::new(),
        );

    harness.run_steps(2);
    snapshot_if_enabled(&mut harness, "v9_notification_stack");
}

#[test]
fn v9_visual_snapshot_modal() {
    let mut harness = Harness::builder()
        .with_size(Vec2::new(800.0, 600.0))
        .build(|ctx| {
            prepare(ctx);
            let _ = Modal::new("snapshot_modal", Vec2::new(520.0, 320.0))
                .title("Major Event")
                .accent(palette::INFO)
                .show(ctx, |ui, body| {
                    let grid = GridLayout::new(
                        vec![Track::Fixed(58.0), Track::Fr(1.0), Track::Fixed(42.0)],
                        vec![Track::Fr(1.0)],
                    )
                    .with_gutter(0.0, spacing::S4);
                    let cells = grid.measure(body);
                    let header = GridLayout::cell(&cells, 0, 0);
                    ui.painter().rect_filled(header, 1.0, Color32::from_black_alpha(150));
                    ui.painter().text(
                        header.left_center() + Vec2::new(spacing::S4, 0.0),
                        Align2::LEFT_CENTER,
                        "Cabinet approves emergency measures",
                        TextRole::Subheading.font_id(),
                        palette::GOLD_HOT,
                    );

                    let text = GridLayout::cell(&cells, 1, 0).shrink2(Vec2::new(spacing::S4, spacing::S4));
                    ui.painter().text(
                        text.left_top(),
                        Align2::LEFT_TOP,
                        "Political pressure is rising. The response will shape stability, war support, and the next round of decisions.",
                        TextRole::Body.font_id(),
                        palette::PARCHMENT,
                    );
                    ui.painter().hline(
                        text.left()..=text.right(),
                        text.bottom(),
                        Stroke::new(1.0, palette::HAIRLINE),
                    );

                    let actions = GridLayout::cell(&cells, 2, 0);
                    Button::new("Confirm")
                        .size(ButtonSize::Md)
                        .variant(ButtonVariant::Primary)
                        .show_at(
                            ui,
                            Rect::from_min_size(actions.left_top(), Vec2::new(132.0, 32.0)),
                        );
                });
        });

    harness.run();
    snapshot_if_enabled(&mut harness, "v9_modal");
}
