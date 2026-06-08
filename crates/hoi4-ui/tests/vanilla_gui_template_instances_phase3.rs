use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use egui_kittest::Harness;
use hoi4_ui::egui::{Color32, Pos2, Sense, Vec2};
use hoi4_ui::icons::IconBank;
use hoi4_ui::theme::apply_vanilla_theme;
use hoi4_ui::vanilla_gui::{
    GuiBinding, GuiBindingMap, GuiPoint, GuiRect, TemplateInstanceOptions, TemplateInstancer,
    VanillaGuiRenderer, VanillaGuiRuntimeContext, COUNTRY_DECISION_DESCRIPTOR,
    COUNTRY_DECISION_GUI_FILE, FOCUS_REQUIRED_SPRITES, NATIONAL_FOCUS_DESCRIPTOR,
    NATIONAL_FOCUS_GUI_FILE,
};

type DecisionBindingFn = fn(&hoi4_ui::vanilla_gui::GuiNodePath) -> GuiBindingMap;

fn snapshot_if_enabled<State>(harness: &mut Harness<'_, State>, name: &str) {
    let enabled = std::env::var_os("RUN_PHASE3_TEMPLATE_SNAPSHOT").is_some()
        || std::env::var_os("UPDATE_SNAPSHOTS").is_some();
    if enabled {
        harness.snapshot(name);
    }
}

fn report_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .expect("workspace root")
        .join("target/gate18/vanilla_gui_template_instances_phase3.md")
}

#[test]
fn gate18_template_instance_visual_smoke_decisions_focus_and_link() {
    let report_path = report_path();
    std::fs::create_dir_all(report_path.parent().unwrap()).unwrap();
    let Some(context) = VanillaGuiRuntimeContext::load_all_profiles(&[
        COUNTRY_DECISION_DESCRIPTOR,
        NATIONAL_FOCUS_DESCRIPTOR,
    ]) else {
        std::fs::write(
            report_path,
            "# Gate 18 Template Instance Smoke\n- skipped: HOI4 vanilla runtime unavailable\n",
        )
        .unwrap();
        return;
    };

    let stats_slot = Rc::new(RefCell::new(None));
    let stats_out = Rc::clone(&stats_slot);
    let path_cfg = context.path_cfg.clone();
    let decision_doc = context.document(COUNTRY_DECISION_GUI_FILE).unwrap().clone();
    let focus_doc = context.document(NATIONAL_FOCUS_GUI_FILE).unwrap().clone();
    let gfx_index = context.gfx_index.clone();

    let mut harness = Harness::builder()
        .with_size(Vec2::new(1280.0, 720.0))
        .build(move |ctx| {
            let _ = apply_vanilla_theme(ctx);
            let mut icon_bank = IconBank::new(ctx.clone(), path_cfg.clone());
            icon_bank.add_decision_search_dirs();
            icon_bank.add_focus_search_dirs();
            let renderer = VanillaGuiRenderer::new(&gfx_index);
            egui::Area::new(egui::Id::new("gate18_template_instance_smoke"))
                .fixed_pos(Pos2::ZERO)
                .show(ctx, |ui| {
                    let _ = ui.allocate_exact_size(Vec2::new(1280.0, 720.0), Sense::hover());
                    let mut stats = hoi4_ui::vanilla_gui::RenderStats::default();
                    stats.merge(paint_decision_template_smoke(
                        ui,
                        &renderer,
                        &decision_doc,
                        &mut icon_bank,
                    ));
                    stats.merge(paint_focus_template_smoke(
                        ui,
                        &renderer,
                        &focus_doc,
                        &mut icon_bank,
                    ));
                    *stats_out.borrow_mut() = Some(stats);
                });
        });

    harness.run();
    snapshot_if_enabled(&mut harness, "vanilla_gui_gate18_template_instances");
    let stats = stats_slot.borrow().clone().unwrap_or_default();

    assert!(stats.nodes_painted > 0, "{stats:?}");
    assert!(stats.text_painted >= 4, "{stats:?}");
    assert!(
        stats.sprites_painted + stats.fallback_painted >= 6,
        "{stats:?}"
    );
    let report = format!(
        "# Gate 18 Template Instance Smoke\n- nodes: {}/{}\n- sprites: {}\n- fallback: {}\n- text: {}\n- buttons: {}\n- focus_required_sprites: {:?}\n- snapshot: crates/hoi4-ui/tests/snapshots/vanilla_gui_gate18_template_instances.png\n",
        stats.nodes_painted,
        stats.nodes_seen,
        stats.sprites_painted,
        stats.fallback_painted,
        stats.text_painted,
        stats.buttons,
        FOCUS_REQUIRED_SPRITES
    );
    std::fs::write(report_path, report).unwrap();
}

fn paint_decision_template_smoke(
    ui: &mut egui::Ui,
    renderer: &VanillaGuiRenderer<'_>,
    document: &hoi4_ui::vanilla_gui::GuiDocument,
    icon_bank: &mut IconBank,
) -> hoi4_ui::vanilla_gui::RenderStats {
    let index = document.template_index();
    let entries = [
        (
            "category_header",
            GuiRect::new(30.0, 30.0, 502.0, 58.0),
            decision_category_bindings as DecisionBindingFn,
        ),
        (
            "decision_item",
            GuiRect::new(30.0, 88.0, 502.0, 41.0),
            decision_item_one_bindings as DecisionBindingFn,
        ),
        (
            "timed_decision_item",
            GuiRect::new(30.0, 129.0, 502.0, 40.0),
            decision_item_two_bindings as DecisionBindingFn,
        ),
        (
            "category_end",
            GuiRect::new(30.0, 169.0, 502.0, 20.0),
            decision_end_bindings as DecisionBindingFn,
        ),
    ];

    let parent_path =
        hoi4_ui::vanilla_gui::GuiNodePath::root("countrydecisionview").child("decision_grid");
    let mut total = hoi4_ui::vanilla_gui::RenderStats::default();
    for (idx, (template_name, rect, bind_fn)) in entries.into_iter().enumerate() {
        let Some(template) = index.get(template_name) else {
            continue;
        };
        let instancer = TemplateInstancer::new(template_name, template);
        let instances = instancer.rect_instances(&parent_path, [rect]);
        total.merge(instancer.paint_instances_with_bindings(
            ui,
            renderer,
            instances,
            |instance| {
                let mut bindings = bind_fn(&instance.path);
                bindings.insert_path(instance.path.clone(), GuiBinding::default());
                if idx == 0 {
                    bindings.insert_path(
                        instance.path.child("track_decisions_checkbox"),
                        GuiBinding::default().checked(false),
                    );
                }
                bindings
            },
            icon_bank,
        ));
    }
    total
}

fn decision_category_bindings(path: &hoi4_ui::vanilla_gui::GuiNodePath) -> GuiBindingMap {
    let mut bindings = GuiBindingMap::default();
    bindings.insert_path(
        path.child("name_text"),
        GuiBinding::default().text("Industry Decisions"),
    );
    bindings
}

fn decision_item_one_bindings(path: &hoi4_ui::vanilla_gui::GuiNodePath) -> GuiBindingMap {
    let mut bindings = GuiBindingMap::default();
    bindings.insert_path(
        path.child("name_text"),
        GuiBinding::default().text("Expand the Autobahn"),
    );
    bindings.insert_path(
        path.child("icon"),
        GuiBinding::default().sprite("GFX_decision_unknown"),
    );
    bindings.insert_path(
        path.child("cost_and_timer_text"),
        GuiBinding::default().text("50 PP"),
    );
    bindings.insert_path(
        path.child("target_flag"),
        GuiBinding::default().visible(false),
    );
    bindings.insert_path(
        path.child("target_flag_frame"),
        GuiBinding::default().visible(false),
    );
    bindings.insert_path(
        path.child("track_decision_checkbox"),
        GuiBinding::default().checked(true),
    );
    bindings.insert_path(
        path.child("btn_select"),
        GuiBinding::default().click("decision:activate:gate18.expand_autobahn"),
    );
    bindings
}

fn decision_item_two_bindings(path: &hoi4_ui::vanilla_gui::GuiNodePath) -> GuiBindingMap {
    let mut bindings = GuiBindingMap::default();
    bindings.insert_path(
        path.child("name_text"),
        GuiBinding::default().text("Army Training Mission"),
    );
    bindings.insert_path(
        path.child("icon"),
        GuiBinding::default().sprite("GFX_decision_unknown"),
    );
    bindings.insert_path(path.child("timer_text"), GuiBinding::default().text("21d"));
    bindings.insert_path(
        path.child("cost_and_timer_text"),
        GuiBinding::default().text("25 PP"),
    );
    bindings.insert_path(
        path.child("target_flag"),
        GuiBinding::default().visible(false),
    );
    bindings.insert_path(
        path.child("target_flag_frame"),
        GuiBinding::default().visible(false),
    );
    bindings.insert_path(
        path.child("btn_select"),
        GuiBinding::default()
            .enabled(false)
            .click("decision:activate:gate18.training"),
    );
    bindings
}

fn decision_end_bindings(_: &hoi4_ui::vanilla_gui::GuiNodePath) -> GuiBindingMap {
    GuiBindingMap::default()
}

fn paint_focus_template_smoke(
    ui: &mut egui::Ui,
    renderer: &VanillaGuiRenderer<'_>,
    document: &hoi4_ui::vanilla_gui::GuiDocument,
    icon_bank: &mut IconBank,
) -> hoi4_ui::vanilla_gui::RenderStats {
    let index = document.template_index();
    let parent_path = hoi4_ui::vanilla_gui::GuiNodePath::root("nationalfocusview")
        .child("tree")
        .child("grid");
    let Some(focus_template) = index.get("national_focus_item") else {
        return hoi4_ui::vanilla_gui::RenderStats::default();
    };
    let instancer = TemplateInstancer::new("national_focus_item", focus_template);
    let focus_rects = [
        GuiRect::new(690.0, 120.0, 1.0, 1.0),
        GuiRect::new(570.0, 270.0, 1.0, 1.0),
        GuiRect::new(810.0, 270.0, 1.0, 1.0),
    ];
    let instances = instancer.rect_instances_with_options(
        &parent_path,
        focus_rects,
        TemplateInstanceOptions::default()
            .template_size(true)
            .zoom(0.85)
            .scroll_offset(GuiPoint { x: 0.0, y: 0.0 }),
    );
    let mut total = instancer.paint_instances_with_bindings(
        ui,
        renderer,
        instances,
        |instance| {
            let names = [
                "Industrial Recovery",
                "Army Modernization",
                "Diplomatic Pressure",
            ];
            let sprites = [
                "GFX_focus_completed",
                "GFX_focus_can_start",
                "GFX_focus_unavailable",
            ];
            let icons = [
                "GFX_goal_unknown",
                "GFX_goal_generic_army_doctrines",
                "GFX_goal_unknown",
            ];
            let mut bindings = GuiBindingMap::default();
            bindings.insert_path(
                instance.path.child("bg"),
                GuiBinding::default().sprite(sprites[instance.index.min(2)]),
            );
            bindings.insert_path(
                instance.path.child("symbol"),
                GuiBinding::default()
                    .sprite(icons[instance.index.min(2)])
                    .click(format!("focus:select:gate18_focus_{}", instance.index)),
            );
            bindings.insert_path(
                instance.path.child("name"),
                GuiBinding::default()
                    .text(names[instance.index.min(2)])
                    .text_color(Color32::WHITE),
            );
            bindings.insert_name("continuous_glow", GuiBinding::default().visible(false));
            bindings
        },
        icon_bank,
    );

    let Some(link_template) = index.get("national_focus_link") else {
        return total;
    };
    let link_instancer = TemplateInstancer::new("national_focus_link", link_template);
    let link_instances = link_instancer.rect_instances(
        &parent_path,
        [
            GuiRect::new(664.0, 210.0, 16.0, 96.0),
            GuiRect::new(794.0, 210.0, 16.0, 96.0),
        ],
    );
    total.merge(link_instancer.paint_instances_with_bindings(
        ui,
        renderer,
        link_instances,
        |instance| {
            let mut bindings = GuiBindingMap::default();
            bindings.insert_path(
                instance.path.child("link"),
                GuiBinding::default().sprite("GFX_focus_link_up_down"),
            );
            bindings
        },
        icon_bank,
    ));
    total
}
