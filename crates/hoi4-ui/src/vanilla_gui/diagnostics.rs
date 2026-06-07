use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use super::error::VanillaGuiIssue;
use super::gfx_index::{GfxIndex, GfxResource};
use super::{
    collect_gfx_references, compute_layout_tree, AnimationSpec, GuiDocument, GuiNode, GuiRect,
    LayoutOptions, RenderStats,
};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct VanillaGuiDiagnostics {
    pub loaded_gui_files: usize,
    pub loaded_gfx_files: usize,
    pub node_count: usize,
    pub resource_definitions: usize,
    pub referenced_resources: usize,
    pub resource_hits: usize,
    pub missing_resources: Vec<String>,
    pub gfx_type_distribution: BTreeMap<String, usize>,
    pub issues: Vec<VanillaGuiIssue>,
}

impl VanillaGuiDiagnostics {
    pub fn merge(&mut self, other: Self) {
        self.loaded_gui_files += other.loaded_gui_files;
        self.loaded_gfx_files += other.loaded_gfx_files;
        self.node_count += other.node_count;
        self.resource_definitions += other.resource_definitions;
        self.referenced_resources += other.referenced_resources;
        self.resource_hits += other.resource_hits;
        self.missing_resources.extend(other.missing_resources);
        for (key, value) in other.gfx_type_distribution {
            *self.gfx_type_distribution.entry(key).or_default() += value;
        }
        self.issues.extend(other.issues);
        self.missing_resources.sort();
        self.missing_resources.dedup();
    }

    pub fn add_issue(&mut self, issue: VanillaGuiIssue) {
        self.issues.push(issue);
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GfxHitReport {
    pub requested: usize,
    pub hits: usize,
    pub missing: Vec<String>,
}

impl GfxHitReport {
    pub fn all_hit(&self) -> bool {
        self.missing.is_empty() && self.hits == self.requested
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Gate1SpriteEvidence {
    pub sprite: String,
    pub gfx_source: Option<PathBuf>,
    pub texture_files: Vec<String>,
    pub frame_count: Option<u32>,
    pub dds_dimensions: Vec<Gate1TextureEvidence>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Gate1TextureEvidence {
    pub texture_file: String,
    pub resolved_path: Option<PathBuf>,
    pub dimensions: Option<(u32, u32)>,
    pub format: Option<String>,
    pub error: Option<String>,
}

pub fn gate1_politics_evidence_report(
    gui: &GuiDocument,
    gfx_index: &GfxIndex,
    path_cfg: &hoi4_paths::PathConfig,
    render_stats: Option<&RenderStats>,
) -> String {
    let mut out = String::new();
    let refs = collect_gfx_references(gui);
    let hit_report = gfx_index.hit_report(refs.iter().map(String::as_str));
    let gfx_diagnostics = gfx_index.diagnostics();
    let root = gui.template_index().get("countrypoliticsview");

    let _ = writeln!(out, "# Gate 1 Politics Vanilla GUI Runtime Baseline");
    let _ = writeln!(out);
    let _ = writeln!(out, "- gui_loaded: {}", root.is_some());
    let _ = writeln!(
        out,
        "- gui_source: {}",
        gui.source
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "<memory>".to_owned())
    );
    let _ = writeln!(out, "- gui_root_count: {}", gui.roots.len());
    let _ = writeln!(out, "- gui_node_count: {}", gui.node_count());
    let _ = writeln!(out, "- gfx_tokens: {}", refs.len());
    let _ = writeln!(out, "- gfx_tokens_hit: {}", hit_report.hits);
    let _ = writeln!(out, "- gfx_tokens_missing: {}", hit_report.missing.len());
    let _ = writeln!(
        out,
        "- gfx_files_loaded: {}",
        gfx_diagnostics.loaded_gfx_files
    );
    let _ = writeln!(
        out,
        "- gfx_resources_loaded: {}",
        gfx_diagnostics.resource_definitions
    );
    if let Some(stats) = render_stats {
        let _ = writeln!(
            out,
            "- render_stats: nodes={}/{} sprites={} fallback={} text={} buttons={} progress={} pie={}",
            stats.nodes_painted,
            stats.nodes_seen,
            stats.sprites_painted,
            stats.fallback_painted,
            stats.text_painted,
            stats.buttons,
            stats.progress_bars,
            stats.pie_charts
        );
    } else {
        let _ = writeln!(out, "- render_stats: <not captured>");
    }
    if !hit_report.missing.is_empty() {
        let _ = writeln!(out, "- missing_gfx_tokens: {:?}", hit_report.missing);
    }

    if let Some(root) = root {
        write_animation_section(&mut out, root);
        write_layout_section(&mut out, root);
    }
    write_sprite_section(&mut out, gui, gfx_index, path_cfg);
    write_snapshot_section(&mut out);
    out
}

fn write_animation_section(out: &mut String, root: &GuiNode) {
    let spec = AnimationSpec::from_node(root);
    let _ = writeln!(out);
    let _ = writeln!(out, "## Root Animation");
    let _ = writeln!(
        out,
        "- hidden_position: x={} y={}",
        spec.hidden_position.x, spec.hidden_position.y
    );
    let _ = writeln!(
        out,
        "- shown_position: x={} y={}",
        spec.shown_position.x, spec.shown_position.y
    );
    let _ = writeln!(out, "- show_curve: {:?}", spec.show_curve);
    let _ = writeln!(out, "- hide_curve: {:?}", spec.hide_curve);
    let _ = writeln!(out, "- duration_ms: {}", spec.duration_ms);
}

fn write_layout_section(out: &mut String, root: &GuiNode) {
    let _ = writeln!(out);
    let _ = writeln!(out, "## Computed Rects");
    for (label, width, height) in [
        ("1080p", 1920.0, 1080.0),
        ("1440p", 2560.0, 1440.0),
        ("small_window", 960.0, 640.0),
    ] {
        let viewport = GuiRect::new(0.0, 0.0, width, height);
        let layout = compute_layout_tree(root, &LayoutOptions::new(viewport).shown_position(true));
        let _ = writeln!(out, "### {label} ({width}x{height})");
        for name in [
            "active_goal",
            "add_national_goal_button",
            "goal_icon",
            "progress",
        ] {
            match layout.find_by_name(name) {
                Some(node) => {
                    let _ = writeln!(
                        out,
                        "- {}: path={} rect=({}, {}, {}, {}) clip=({}, {}, {}, {}) visible={}",
                        name,
                        node.path,
                        node.rect.x,
                        node.rect.y,
                        node.rect.width,
                        node.rect.height,
                        node.clip_rect.x,
                        node.clip_rect.y,
                        node.clip_rect.width,
                        node.clip_rect.height,
                        node.visible
                    );
                }
                None => {
                    let _ = writeln!(out, "- {name}: <missing>");
                }
            }
        }
    }
}

fn write_sprite_section(
    out: &mut String,
    gui: &GuiDocument,
    gfx_index: &GfxIndex,
    path_cfg: &hoi4_paths::PathConfig,
) {
    let mut sprites = collect_gfx_references(gui);
    for key in [
        "GFX_pol_view_bg",
        "GFX_pol_goal_bg",
        "GFX_add_national_goal_button",
        "GFX_activegoal_progress",
        "GFX_category_header",
        "GFX_idea_categories",
    ] {
        if !sprites.iter().any(|sprite| sprite == key) {
            sprites.push(key.to_owned());
        }
    }
    sprites.sort();
    sprites.dedup();

    let _ = writeln!(out);
    let _ = writeln!(out, "## Sprite Sources");
    for sprite in sprites {
        let evidence = gate1_sprite_evidence(&sprite, gfx_index.get(&sprite), path_cfg);
        let source = evidence
            .gfx_source
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "<missing .gfx mapping>".to_owned());
        let _ = writeln!(
            out,
            "- {}: source={} textureFile={:?} frame_count={:?}",
            evidence.sprite, source, evidence.texture_files, evidence.frame_count
        );
        for texture in evidence.dds_dimensions {
            let path = texture
                .resolved_path
                .as_ref()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| "<unresolved>".to_owned());
            let _ = writeln!(
                out,
                "  - texture={} path={} dds_size={:?} format={:?} error={:?}",
                texture.texture_file, path, texture.dimensions, texture.format, texture.error
            );
        }
    }
}

pub fn gate1_sprite_evidence(
    sprite: &str,
    resource: Option<&GfxResource>,
    path_cfg: &hoi4_paths::PathConfig,
) -> Gate1SpriteEvidence {
    let texture_files = resource
        .map(|resource| resource.textures.clone())
        .unwrap_or_default();
    let dds_dimensions = texture_files
        .iter()
        .map(|texture| gate1_texture_evidence(texture, path_cfg))
        .collect();
    Gate1SpriteEvidence {
        sprite: sprite.to_owned(),
        gfx_source: resource.and_then(|resource| resource.source.clone()),
        texture_files,
        frame_count: resource.and_then(|resource| resource.frame_count),
        dds_dimensions,
    }
}

fn gate1_texture_evidence(
    texture_file: &str,
    path_cfg: &hoi4_paths::PathConfig,
) -> Gate1TextureEvidence {
    let resolved_path = path_cfg.find(texture_file);
    let Some(path) = resolved_path.as_deref() else {
        return Gate1TextureEvidence {
            texture_file: texture_file.to_owned(),
            resolved_path,
            dimensions: None,
            format: None,
            error: Some("textureFile path not found".to_owned()),
        };
    };
    let extension = path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if extension != "dds" {
        return Gate1TextureEvidence {
            texture_file: texture_file.to_owned(),
            resolved_path,
            dimensions: None,
            format: Some(extension),
            error: Some("not a DDS texture".to_owned()),
        };
    }
    match read_dds_header(path) {
        Ok((width, height, format)) => Gate1TextureEvidence {
            texture_file: texture_file.to_owned(),
            resolved_path,
            dimensions: Some((width, height)),
            format: Some(format),
            error: None,
        },
        Err(error) => Gate1TextureEvidence {
            texture_file: texture_file.to_owned(),
            resolved_path,
            dimensions: None,
            format: None,
            error: Some(error),
        },
    }
}

fn read_dds_header(path: &Path) -> Result<(u32, u32, String), String> {
    let bytes = std::fs::read(path).map_err(|err| err.to_string())?;
    let dds = hoi4_assets::dds::DdsImage::parse(&bytes).map_err(|err| format!("{err:?}"))?;
    Ok((dds.width, dds.height, format!("{:?}", dds.format)))
}

fn write_snapshot_section(out: &mut String) {
    let _ = writeln!(out);
    let _ = writeln!(out, "## Snapshot Baseline");
    for path in [
        "crates/hoi4-ui/tests/snapshots/politics_gate9_1936_1080p.png",
        "crates/hoi4-ui/tests/snapshots/politics_gate9_1936_1440p.png",
        "crates/hoi4-ui/tests/snapshots/politics_gate9_1936_small_window.png",
    ] {
        let exists = Path::new(path).exists();
        let _ = writeln!(out, "- {path}: exists={exists}");
    }
}

#[cfg(test)]
mod gate1_tests {
    use super::*;
    use crate::icons::IconBank;
    use crate::vanilla_gui::{bind_profile_tree, parse_gui_file, VanillaGuiRenderer};
    use crate::{politics::PoliticsData, vanilla_gui::VanillaPanelProfile};
    use egui::{Pos2, Sense, Vec2};
    use std::cell::RefCell;
    use std::rc::Rc;

    struct Gate1EmptyPoliticsProfile;

    impl VanillaPanelProfile for Gate1EmptyPoliticsProfile {
        type Data = PoliticsData;
        type Command = ();

        fn profile_id(&self) -> &'static str {
            "gate1_country_politics"
        }

        fn root_template(&self) -> &'static str {
            "countrypoliticsview"
        }

        fn required_gui_files(&self) -> &'static [&'static str] {
            &["interface/countrypoliticsview.gui"]
        }

        fn bind_node(
            &self,
            _node_path: &crate::vanilla_gui::GuiNodePath,
            _data: &Self::Data,
        ) -> crate::vanilla_gui::GuiBinding {
            crate::vanilla_gui::GuiBinding::default()
        }

        fn handle_action(
            &self,
            _action: crate::vanilla_gui::GuiAction,
            _data: &Self::Data,
        ) -> Option<Self::Command> {
            None
        }
    }

    #[test]
    fn gate13_politics_runtime_evidence_baseline() {
        let workspace_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|path| path.parent())
            .expect("workspace root")
            .to_path_buf();
        let out_dir = workspace_root.join("target/gate1");
        std::fs::create_dir_all(&out_dir).unwrap();
        let report_path = out_dir.join("politics_vanilla_gui_runtime_baseline.md");
        let Ok(path_cfg) = hoi4_paths::PathConfig::resolve(Default::default()) else {
            std::fs::write(
                &report_path,
                "# Gate 1 Politics Vanilla GUI Runtime Baseline\n\nvanilla_available: false\n",
            )
            .unwrap();
            return;
        };
        let Some(gui_path) = path_cfg.find("interface/countrypoliticsview.gui") else {
            std::fs::write(
                &report_path,
                "# Gate 1 Politics Vanilla GUI Runtime Baseline\n\nvanilla_available: false\nmissing: interface/countrypoliticsview.gui\n",
            )
            .unwrap();
            return;
        };

        let gui = parse_gui_file(gui_path).unwrap();
        let gfx_index = GfxIndex::from_path_config(&path_cfg);
        let root = gui
            .template_index()
            .get("countrypoliticsview")
            .expect("politics root template");
        let viewport = GuiRect::new(0.0, 0.0, 1920.0, 1080.0);
        let layout = compute_layout_tree(root, &LayoutOptions::new(viewport).shown_position(true));
        for name in [
            "active_goal",
            "add_national_goal_button",
            "goal_icon",
            "progress",
        ] {
            assert!(layout.find_by_name(name).is_some(), "missing {name}");
        }

        let profile = Gate1EmptyPoliticsProfile;
        let data = PoliticsData::legacy("fascism".to_owned(), Vec::new(), Vec::new());
        let bindings = bind_profile_tree(&profile, root, &data);
        let stats_slot: Rc<RefCell<Option<RenderStats>>> = Rc::new(RefCell::new(None));
        let stats_out = Rc::clone(&stats_slot);
        let mut harness = egui_kittest::Harness::builder()
            .with_size(Vec2::new(1920.0, 1080.0))
            .build(move |ctx| {
                let mut icon_bank = IconBank::new(ctx.clone(), path_cfg.clone());
                icon_bank.add_politics_search_dirs();
                egui::Area::new(egui::Id::new("gate1_politics_runtime_evidence"))
                    .fixed_pos(Pos2::ZERO)
                    .show(ctx, |ui| {
                        let _ = ui.allocate_exact_size(Vec2::new(1920.0, 1080.0), Sense::hover());
                        let renderer = VanillaGuiRenderer::new(&gfx_index);
                        let stats =
                            renderer.paint_tree(ui, root, &layout, &bindings, &mut icon_bank);
                        *stats_out.borrow_mut() = Some(stats);
                    });
            });
        harness.run();
        let stats = stats_slot.borrow().clone().expect("captured render stats");

        let path_cfg = hoi4_paths::PathConfig::resolve(Default::default()).unwrap();
        let gui_path = path_cfg.find("interface/countrypoliticsview.gui").unwrap();
        let gui = parse_gui_file(gui_path).unwrap();
        let gfx_index = GfxIndex::from_path_config(&path_cfg);
        let report = gate1_politics_evidence_report(&gui, &gfx_index, &path_cfg, Some(&stats));
        std::fs::write(&report_path, &report).unwrap();
        println!("{}", report);

        assert!(report.contains("gui_loaded: true"));
        assert!(report.contains("render_stats: nodes="));
        assert!(report.contains("GFX_pol_view_bg"));
        assert!(report.contains("GFX_activegoal_progress"));
    }
}
