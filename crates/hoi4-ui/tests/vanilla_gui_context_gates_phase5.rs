use std::fmt::Write as _;
use std::path::PathBuf;

use egui::Color32;
use hoi4_ui::vanilla_gui::{
    apply_alpha_to_tint, button_frame_for_effect, parse_gui_str, progress_fill_for_effect,
    ButtonVisualState, GfxIndex, GuiAlphaSemantics, GuiBinding, GuiBindingMap,
    GuiButtonEffectPolicy, GuiDrawCommandKind, GuiProgressEffectPolicy, GuiRect, GuiRuntimeFrame,
    GuiRuntimeFrameInput, GuiRuntimeState,
};

fn report_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .expect("workspace root")
        .join("target/clausewitz_gui")
        .join(name)
}

fn write_report(name: &str, body: impl AsRef<str>) {
    let path = report_path(name);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, body.as_ref()).unwrap();
}

fn phase5_gfx() -> GfxIndex {
    GfxIndex::parse_single(
        "phase5.gfx",
        r#"
spriteTypes = {
    spriteType = {
        name = "GFX_plain"
        size = { x = 32 y = 16 }
        texturefile = "gfx/interface/plain.dds"
    }
    spriteType = {
        name = "GFX_button"
        size = { x = 40 y = 20 }
        noOfFrames = 4
        effectFile = "gfx/FX/buttonstate_onlydisable.lua"
    }
    spriteType = {
        name = "GFX_button_nodown"
        size = { x = 40 y = 20 }
        noOfFrames = 4
        effectFile = "gfx/FX/buttonstate_nodowneffect.lua"
    }
    spriteType = {
        name = "GFX_button_blend"
        size = { x = 40 y = 20 }
        noOfFrames = 4
        effectFile = "gfx/FX/buttonstate_blendframes.lua"
    }
    spriteType = {
        name = "GFX_button_unknown"
        size = { x = 40 y = 20 }
        noOfFrames = 4
        effectFile = "gfx/FX/buttonstate_future.lua"
    }
    spriteType = {
        name = "GFX_checkbox"
        size = { x = 20 y = 20 }
        noOfFrames = 5
    }
    pieChartType = {
        name = "GFX_political_chart"
        size = 27
        textureFile1 = "gfx/interface/pie_overlay.dds"
    }
    progressbarType = {
        name = "GFX_progress"
        size = { x = 100 y = 8 }
        textureFile1 = "gfx/interface/progress_fg.dds"
        textureFile2 = "gfx/interface/progress_bg.dds"
        effectFile = "gfx/FX/progress.lua"
    }
    progressbarType = {
        name = "GFX_progress_startend"
        size = { x = 120 y = 10 }
        effectFile = "gfx/FX/progress_startend.lua"
    }
}
"#,
    )
}

fn phase5_doc() -> hoi4_ui::vanilla_gui::GuiDocument {
    parse_gui_str(
        Some(PathBuf::from("interface/phase5.gui")),
        r#"
guiTypes = {
    containerWindowType = {
        name = "root"
        size = { width = 360 height = 220 }
        iconType = {
            name = "plain"
            position = { x = 10 y = 10 }
            spriteType = "GFX_plain"
            alpha = 0.5
        }
        buttonType = {
            name = "action_button"
            position = { x = 10 y = 40 }
            quadTextureSprite = "GFX_button"
            text = "OK"
        }
        checkBoxType = {
            name = "check"
            position = { x = 60 y = 40 }
            quadTextureSprite = "GFX_checkbox"
            frame = 2
        }
        iconType = {
            name = "political_pie_chart"
            position = { x = 54 y = 88 }
            spriteType = "GFX_political_chart"
        }
        iconType = {
            name = "progress"
            position = { x = 10 y = 150 }
            spriteType = "GFX_progress"
        }
        iconType = {
            name = "progress_startend"
            position = { x = 10 y = 166 }
            spriteType = "GFX_progress_startend"
        }
        instantTextboxType = {
            name = "title"
            position = { x = 10 y = 186 }
            maxWidth = 120
            maxHeight = 22
            format = center
            vertical_alignment = center
            font = "hoi_14mbs"
            text = "POLITICS_IDEOLOGY"
        }
    }
}
"#,
    )
}

#[test]
fn gate22_runtime_outputs_draw_command_ir_for_sprite_text_button_progress_and_pie() {
    let gfx = phase5_gfx();
    let doc = phase5_doc();
    let root = doc.template_index().get("root").unwrap();
    let mut bindings = GuiBindingMap::default();
    bindings.insert_name("action_button", GuiBinding::default().click("ok"));
    bindings.insert_name("progress", GuiBinding::default().progress(0.5));
    bindings.insert_name("progress_startend", GuiBinding::default().progress(0.5));
    let frame = GuiRuntimeFrame::build(
        GuiRuntimeFrameInput::new(
            root,
            GuiRect::new(0.0, 0.0, 360.0, 220.0),
            "phase5",
            &bindings,
        )
        .with_gfx_index(&gfx),
    );

    assert!(frame
        .draw_list
        .iter()
        .any(|command| matches!(command.kind, GuiDrawCommandKind::DrawSprite(_))));
    assert!(frame
        .draw_list
        .iter()
        .any(|command| matches!(command.kind, GuiDrawCommandKind::DrawText(_))));
    assert!(frame
        .draw_list
        .iter()
        .any(|command| matches!(command.kind, GuiDrawCommandKind::DrawProgress(_))));
    assert!(frame
        .draw_list
        .iter()
        .any(|command| matches!(command.kind, GuiDrawCommandKind::DrawPieChart(_))));
    assert_eq!(frame.diagnostics.draw_commands.total, frame.draw_list.len());
    assert!(frame.diagnostics.draw_commands.sprites >= 2);
    assert!(frame
        .diagnostics
        .draw_command_markdown()
        .contains("DrawProgress"));
}

#[test]
fn gate23_button_effect_policy_covers_required_buttonstate_scripts_and_reports_unsupported() {
    assert_eq!(
        button_frame_for_effect(
            &GuiButtonEffectPolicy::OnlyDisable,
            Some(4),
            ButtonVisualState::Normal
        ),
        1
    );
    assert_eq!(
        button_frame_for_effect(
            &GuiButtonEffectPolicy::OnlyDisable,
            Some(4),
            ButtonVisualState::Disabled
        ),
        2
    );
    assert_eq!(
        button_frame_for_effect(
            &GuiButtonEffectPolicy::NoDownEffect,
            Some(4),
            ButtonVisualState::Pressed
        ),
        2
    );
    assert_eq!(
        button_frame_for_effect(
            &GuiButtonEffectPolicy::BlendFrames,
            Some(4),
            ButtonVisualState::Hover
        ),
        2
    );

    let gfx = phase5_gfx();
    let doc = parse_gui_str(
        Some(PathBuf::from("interface/phase5_gate23.gui")),
        r#"
guiTypes = {
    containerWindowType = {
        name = "root"
        size = { width = 100 height = 60 }
        buttonType = {
            name = "unknown_button"
            quadTextureSprite = "GFX_button_unknown"
        }
    }
}
"#,
    );
    let root = doc.template_index().get("root").unwrap();
    let frame = GuiRuntimeFrame::build(
        GuiRuntimeFrameInput::new(
            root,
            GuiRect::new(0.0, 0.0, 100.0, 60.0),
            "gate23",
            &GuiBindingMap::default(),
        )
        .with_gfx_index(&gfx),
    );

    assert_eq!(frame.diagnostics.draw_commands.unsupported_effects.len(), 1);
    assert!(frame
        .diagnostics
        .unsupported_effects_markdown()
        .contains("buttonstate_future.lua"));
    write_report(
        "gate23_unsupported_effects.md",
        frame.diagnostics.unsupported_effects_markdown(),
    );
}

#[test]
fn gate24_tint_and_alpha_semantics_are_carried_by_sprite_commands() {
    let gfx = phase5_gfx();
    let doc = phase5_doc();
    let root = doc.template_index().get("root").unwrap();
    let mut bindings = GuiBindingMap::default();
    bindings.insert_name(
        "plain",
        GuiBinding::default().tint(Color32::from_rgb(150, 75, 0)),
    );
    let frame = GuiRuntimeFrame::build(
        GuiRuntimeFrameInput::new(
            root,
            GuiRect::new(0.0, 0.0, 360.0, 220.0),
            "gate24",
            &bindings,
        )
        .with_gfx_index(&gfx),
    );
    let sprite = frame
        .draw_list
        .iter()
        .find(|command| command.source_name.as_deref() == Some("plain"))
        .and_then(|command| match &command.kind {
            GuiDrawCommandKind::DrawSprite(sprite) => Some(sprite),
            _ => None,
        })
        .unwrap();

    assert_eq!(sprite.tint, Color32::from_rgb(150, 75, 0));
    assert_eq!(sprite.alpha_semantics, GuiAlphaSemantics::NodeAlpha);
    assert_eq!(
        apply_alpha_to_tint(Color32::from_rgb(150, 75, 0), 0.5).a(),
        128
    );

    let mut report = String::new();
    let _ = writeln!(report, "# Gate 24 Tint and Alpha");
    let _ = writeln!(report, "- tint: {:?}", sprite.tint);
    let _ = writeln!(report, "- alpha: {:.2}", sprite.alpha);
    let _ = writeln!(
        report,
        "- alpha_semantics: {}",
        sprite.alpha_semantics.as_str()
    );
    let _ = writeln!(report, "- blend_mode: {}", sprite.blend_mode.as_str());
    write_report("gate24_tint_alpha_report.md", report);
}

#[test]
fn gate25_pie_chart_command_uses_segments_overlay_and_radius_size() {
    let gfx = phase5_gfx();
    let doc = phase5_doc();
    let root = doc.template_index().get("root").unwrap();
    let mut bindings = GuiBindingMap::default();
    bindings.insert_name(
        "political_pie_chart",
        GuiBinding {
            pie_segments: vec![
                (0.75, Color32::from_rgb(150, 75, 0)),
                (0.25, Color32::from_rgb(0, 0, 255)),
            ],
            ..GuiBinding::default()
        },
    );
    let frame = GuiRuntimeFrame::build(
        GuiRuntimeFrameInput::new(
            root,
            GuiRect::new(0.0, 0.0, 360.0, 220.0),
            "gate25",
            &bindings,
        )
        .with_gfx_index(&gfx),
    );
    let command = frame
        .draw_list
        .iter()
        .find(|command| command.source_name.as_deref() == Some("political_pie_chart"))
        .unwrap();

    match &command.kind {
        GuiDrawCommandKind::DrawPieChart(pie) => {
            assert_eq!(command.rect, GuiRect::new(27.0, 88.0, 54.0, 54.0));
            assert_eq!(pie.segments.len(), 2);
            assert!(!pie.used_fallback_segments);
            assert_eq!(
                pie.overlay_texture.as_deref(),
                Some("gfx/interface/pie_overlay.dds")
            );
        }
        other => panic!("expected pie chart command, got {other:?}"),
    }
}

#[test]
fn gate26_progress_command_records_value_effect_and_fill_rect() {
    let gfx = phase5_gfx();
    let doc = phase5_doc();
    let root = doc.template_index().get("root").unwrap();
    let mut bindings = GuiBindingMap::default();
    bindings.insert_name("progress", GuiBinding::default().progress(0.5));
    bindings.insert_name("progress_startend", GuiBinding::default().progress(0.5));
    let frame = GuiRuntimeFrame::build(
        GuiRuntimeFrameInput::new(
            root,
            GuiRect::new(0.0, 0.0, 360.0, 220.0),
            "gate26",
            &bindings,
        )
        .with_gfx_index(&gfx),
    );
    let progress = frame
        .draw_list
        .iter()
        .find(|command| command.source_name.as_deref() == Some("progress"))
        .and_then(|command| match &command.kind {
            GuiDrawCommandKind::DrawProgress(progress) => Some(progress),
            _ => None,
        })
        .unwrap();
    assert_eq!(progress.value, 0.5);
    assert_eq!(progress.effect, GuiProgressEffectPolicy::DefaultProgress);
    assert_eq!(progress.fill_rect.width, 50.0);

    let startend = frame
        .draw_list
        .iter()
        .find(|command| command.source_name.as_deref() == Some("progress_startend"))
        .and_then(|command| match &command.kind {
            GuiDrawCommandKind::DrawProgress(progress) => Some((command.rect, progress)),
            _ => None,
        })
        .unwrap();
    assert_eq!(startend.1.effect, GuiProgressEffectPolicy::StartEnd);
    assert_eq!(
        progress_fill_for_effect(startend.0, 0.5, &startend.1.effect).width,
        55.0
    );
}

#[test]
fn gate27_text_command_records_font_fit_clip_alignment_and_localization_fallback() {
    let gfx = phase5_gfx();
    let doc = phase5_doc();
    let root = doc.template_index().get("root").unwrap();
    let frame = GuiRuntimeFrame::build(
        GuiRuntimeFrameInput::new(
            root,
            GuiRect::new(0.0, 0.0, 360.0, 220.0),
            "gate27",
            &GuiBindingMap::default(),
        )
        .with_runtime_state(GuiRuntimeState::shown(GuiRect::new(0.0, 0.0, 360.0, 220.0)))
        .with_gfx_index(&gfx),
    );
    let text = frame
        .draw_list
        .iter()
        .find(|command| command.source_name.as_deref() == Some("title"))
        .and_then(|command| match &command.kind {
            GuiDrawCommandKind::DrawText(text) => Some(text),
            _ => None,
        })
        .unwrap();

    assert_eq!(text.raw_text, "POLITICS_IDEOLOGY");
    assert_eq!(text.text_layout.box_rect.width, 120.0);
    assert_eq!(
        text.text_layout.horizontal,
        hoi4_ui::vanilla_gui::GuiTextHorizontalAlign::Center
    );
    assert!(text.localization_fallback);
    assert_eq!(frame.diagnostics.draw_commands.text, 2);
}
