use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use clausewitz_parser::{parse, Block, Value};
use hoi4_paths::PathConfig;

use super::ast::GuiValueExt;
use super::diagnostics::{GfxHitReport, VanillaGuiDiagnostics};
use super::error::{VanillaGuiIssue, VanillaGuiIssueKind};

pub type GfxTypeDistribution = BTreeMap<String, usize>;

#[derive(Debug, Clone)]
pub struct GfxIndex {
    resources: HashMap<String, GfxResource>,
    diagnostics: VanillaGuiDiagnostics,
}

impl GfxIndex {
    pub fn from_path_config(path_cfg: &PathConfig) -> Self {
        let mut files = Vec::new();
        for interface_dir in path_cfg.find_all("interface") {
            collect_gfx_files_recursive(&interface_dir, &mut files);
        }
        files.sort();
        Self::from_files(files)
    }

    pub fn from_files(files: impl IntoIterator<Item = PathBuf>) -> Self {
        let mut index = Self {
            resources: HashMap::new(),
            diagnostics: VanillaGuiDiagnostics::default(),
        };
        for file in files {
            index.load_file(&file);
        }
        index.diagnostics.resource_definitions = index.resources.len();
        index.diagnostics.gfx_type_distribution = index.type_distribution();
        index
    }

    pub fn parse_single(source: impl Into<PathBuf>, text: &str) -> Self {
        let source = source.into();
        let mut index = Self {
            resources: HashMap::new(),
            diagnostics: VanillaGuiDiagnostics::default(),
        };
        for resource in parse_gfx_resources(Some(source), text) {
            index.insert_first(resource);
        }
        index.diagnostics.loaded_gfx_files = 1;
        index.diagnostics.resource_definitions = index.resources.len();
        index.diagnostics.gfx_type_distribution = index.type_distribution();
        index
    }

    pub fn get(&self, name: &str) -> Option<&GfxResource> {
        self.resources.get(name)
    }

    pub fn contains(&self, name: &str) -> bool {
        self.resources.contains_key(name)
    }

    pub fn len(&self) -> usize {
        self.resources.len()
    }

    pub fn is_empty(&self) -> bool {
        self.resources.is_empty()
    }

    pub fn diagnostics(&self) -> VanillaGuiDiagnostics {
        let mut diagnostics = self.diagnostics.clone();
        diagnostics.resource_definitions = self.resources.len();
        diagnostics.gfx_type_distribution = self.type_distribution();
        diagnostics
    }

    pub fn type_distribution(&self) -> GfxTypeDistribution {
        let mut out = BTreeMap::new();
        for resource in self.resources.values() {
            *out.entry(resource.kind.label().to_owned()).or_default() += 1;
        }
        out
    }

    pub fn hit_report<'a>(&self, names: impl IntoIterator<Item = &'a str>) -> GfxHitReport {
        let mut report = GfxHitReport::default();
        for name in names {
            report.requested += 1;
            if self.contains(name) {
                report.hits += 1;
            } else {
                report.missing.push(name.to_owned());
            }
        }
        report.missing.sort();
        report.missing.dedup();
        report
    }

    pub fn missing_texture_report(&self, path_cfg: &PathConfig) -> Vec<String> {
        let mut missing = Vec::new();
        for resource in self.resources.values() {
            for texture in &resource.textures {
                if path_cfg.find(texture).is_none() {
                    missing.push(format!("{} -> {}", resource.name, texture));
                }
            }
        }
        missing.sort();
        missing.dedup();
        missing
    }

    fn load_file(&mut self, path: &Path) {
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(err) => {
                self.diagnostics.add_issue(VanillaGuiIssue::with_source(
                    VanillaGuiIssueKind::MissingGfx,
                    path,
                    err.to_string(),
                ));
                return;
            }
        };
        self.diagnostics.loaded_gfx_files += 1;
        for resource in parse_gfx_resources(Some(path.to_path_buf()), &text) {
            self.insert_first(resource);
        }
    }

    fn insert_first(&mut self, resource: GfxResource) {
        self.resources
            .entry(resource.name.clone())
            .or_insert(resource);
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct GfxResource {
    pub name: String,
    pub kind: GfxResourceKind,
    pub source: Option<PathBuf>,
    pub primary_texture: Option<String>,
    pub textures: Vec<String>,
    pub size: Option<GfxSize>,
    pub frame_count: Option<u32>,
    pub fps: Option<f32>,
    pub looped: Option<bool>,
    pub border: Option<GfxBorder>,
    pub raw_properties: Vec<GfxProperty>,
}

impl GfxResource {
    pub fn fallback_texture_name(&self) -> Option<&str> {
        self.primary_texture.as_deref()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum GfxResourceKind {
    Sprite,
    CorneredTile,
    FrameAnimated,
    ProgressBar,
    TextSprite,
    MaskedShield,
    PieChart,
    Unknown(String),
}

impl GfxResourceKind {
    pub fn from_key(key: &str) -> Option<Self> {
        match key.to_ascii_lowercase().as_str() {
            "spritetype" => Some(Self::Sprite),
            "corneredtilespritetype" => Some(Self::CorneredTile),
            "frameanimatedspritetype" => Some(Self::FrameAnimated),
            "progressbartype" => Some(Self::ProgressBar),
            "textspritetype" => Some(Self::TextSprite),
            "maskedshieldtype" => Some(Self::MaskedShield),
            "piecharttype" => Some(Self::PieChart),
            _ => None,
        }
    }

    pub fn label(&self) -> &str {
        match self {
            Self::Sprite => "spriteType",
            Self::CorneredTile => "corneredTileSpriteType",
            Self::FrameAnimated => "frameAnimatedSpriteType",
            Self::ProgressBar => "progressbartype",
            Self::TextSprite => "textSpriteType",
            Self::MaskedShield => "maskedShieldType",
            Self::PieChart => "pieChartType",
            Self::Unknown(value) => value.as_str(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GfxSize {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GfxBorder {
    pub left: f32,
    pub right: f32,
    pub top: f32,
    pub bottom: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GfxProperty {
    pub key: String,
    pub value: String,
}

fn collect_gfx_files_recursive(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_gfx_files_recursive(&path, out);
        } else if path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.eq_ignore_ascii_case("gfx"))
            .unwrap_or(false)
        {
            out.push(path);
        }
    }
}

fn parse_gfx_resources(source: Option<PathBuf>, text: &str) -> Vec<GfxResource> {
    let block = parse(text);
    let mut out = Vec::new();
    collect_resources_from_block(source.as_ref(), &block, &mut out);
    out
}

fn collect_resources_from_block(
    source: Option<&PathBuf>,
    block: &Block,
    out: &mut Vec<GfxResource>,
) {
    for entry in &block.entries {
        if let Some(kind) = GfxResourceKind::from_key(&entry.key) {
            if let Value::Block(resource_block) = &entry.value {
                if let Some(resource) = resource_from_block(source.cloned(), kind, resource_block) {
                    out.push(resource);
                }
            }
            continue;
        }
        if let Value::Block(inner) = &entry.value {
            collect_resources_from_block(source, inner, out);
        }
    }
}

fn resource_from_block(
    source: Option<PathBuf>,
    kind: GfxResourceKind,
    block: &Block,
) -> Option<GfxResource> {
    let name = get_string_ci(block, "name")?;
    let textures = texture_properties(block);
    let primary_texture = textures.first().cloned();
    Some(GfxResource {
        name,
        kind,
        source,
        primary_texture,
        textures,
        size: parse_size(block),
        frame_count: parse_u32_any(block, &["noOfFrames", "frames", "frame_count"]),
        fps: parse_f32_any(block, &["animation_rate_fps", "fps", "animation_speed"]),
        looped: parse_bool_any(block, &["looping", "loop"]),
        border: parse_border(block),
        raw_properties: raw_properties(block),
    })
}

fn texture_properties(block: &Block) -> Vec<String> {
    let mut out = Vec::new();
    for entry in &block.entries {
        let key = entry.key.to_ascii_lowercase();
        if key.starts_with("texturefile") {
            if let Some(value) = entry.value.as_lossy_string() {
                out.push(normalize_texture_path(&value));
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

fn normalize_texture_path(value: &str) -> String {
    value.replace('\\', "/").replace("//", "/")
}

fn parse_size(block: &Block) -> Option<GfxSize> {
    let value = get_value_ci(block, "size")?;
    match value {
        Value::Integer(value) => Some(GfxSize {
            x: *value as f32,
            y: *value as f32,
        }),
        Value::Float(value) => Some(GfxSize {
            x: *value as f32,
            y: *value as f32,
        }),
        Value::Block(size) => {
            let x = get_value_ci(size, "x")
                .or_else(|| get_value_ci(size, "width"))
                .and_then(|value| value.as_lossy_f32())?;
            let y = get_value_ci(size, "y")
                .or_else(|| get_value_ci(size, "height"))
                .and_then(|value| value.as_lossy_f32())?;
            Some(GfxSize { x, y })
        }
        _ => None,
    }
}

fn parse_border(block: &Block) -> Option<GfxBorder> {
    let border = get_value_ci(block, "borderSize")
        .or_else(|| get_value_ci(block, "bordersize"))
        .or_else(|| get_value_ci(block, "corner_size"))?;
    match border {
        Value::Integer(value) => {
            let value = *value as f32;
            Some(GfxBorder {
                left: value,
                right: value,
                top: value,
                bottom: value,
            })
        }
        Value::Float(value) => {
            let value = *value as f32;
            Some(GfxBorder {
                left: value,
                right: value,
                top: value,
                bottom: value,
            })
        }
        Value::Block(block) => {
            let left = get_value_ci(block, "left")
                .or_else(|| get_value_ci(block, "x"))
                .and_then(|value| value.as_lossy_f32())?;
            let top = get_value_ci(block, "top")
                .or_else(|| get_value_ci(block, "y"))
                .and_then(|value| value.as_lossy_f32())?;
            let right = get_value_ci(block, "right")
                .and_then(|value| value.as_lossy_f32())
                .unwrap_or(left);
            let bottom = get_value_ci(block, "bottom")
                .and_then(|value| value.as_lossy_f32())
                .unwrap_or(top);
            Some(GfxBorder {
                left,
                right,
                top,
                bottom,
            })
        }
        _ => None,
    }
}

fn parse_u32_any(block: &Block, keys: &[&str]) -> Option<u32> {
    keys.iter()
        .find_map(|key| get_value_ci(block, key).and_then(|value| value.as_lossy_f32()))
        .map(|value| value.max(0.0) as u32)
}

fn parse_f32_any(block: &Block, keys: &[&str]) -> Option<f32> {
    keys.iter()
        .find_map(|key| get_value_ci(block, key).and_then(|value| value.as_lossy_f32()))
}

fn parse_bool_any(block: &Block, keys: &[&str]) -> Option<bool> {
    keys.iter()
        .find_map(|key| get_value_ci(block, key).and_then(|value| value.as_lossy_bool()))
}

fn raw_properties(block: &Block) -> Vec<GfxProperty> {
    block
        .entries
        .iter()
        .filter_map(|entry| {
            entry.value.as_lossy_string().map(|value| GfxProperty {
                key: entry.key.clone(),
                value,
            })
        })
        .collect()
}

fn get_string_ci(block: &Block, key: &str) -> Option<String> {
    get_value_ci(block, key).and_then(|value| value.as_lossy_string())
}

fn get_value_ci<'a>(block: &'a Block, key: &str) -> Option<&'a Value> {
    block
        .entries
        .iter()
        .find(|entry| entry.key.eq_ignore_ascii_case(key))
        .map(|entry| &entry.value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vanilla_gui::ast::{collect_gfx_references, parse_gui_file};

    #[test]
    fn parses_supported_resource_types() {
        let index = GfxIndex::parse_single(
            "fixture.gfx",
            r#"
spriteTypes = {
    spriteType = { name = "GFX_plain" texturefile = "gfx/interface/plain.dds" }
    corneredTileSpriteType = {
        name = "GFX_tile"
        texturefile = "gfx/interface/tile.dds"
        borderSize = 8
    }
    frameAnimatedSpriteType = {
        name = "GFX_anim"
        texturefile = "gfx/interface/anim.dds"
        noOfFrames = 12
        animation_rate_fps = 24
        looping = yes
    }
    progressbartype = {
        name = "GFX_progress"
        textureFile1 = "gfx/interface/progress.dds"
        textureFile2 = "gfx/interface/progress_bg.dds"
        size = { x = 237 y = 6 }
    }
    textSpriteType = { name = "GFX_text" texturefile = "gfx/interface/text.dds" }
    maskedShieldType = {
        name = "GFX_player_flag"
        textureFile1 = "gfx/interface/flag_overlay.dds"
        textureFile2 = "gfx/interface/shield_mask.tga"
    }
    pieChartType = { name = "GFX_political_chart" size = 27 }
}
"#,
        );

        assert_eq!(index.len(), 7);
        assert_eq!(
            index.get("GFX_tile").unwrap().kind,
            GfxResourceKind::CorneredTile
        );
        assert_eq!(index.get("GFX_anim").unwrap().frame_count, Some(12));
        assert_eq!(
            index.get("GFX_progress").unwrap().size,
            Some(GfxSize { x: 237.0, y: 6.0 })
        );
        assert_eq!(
            index.get("GFX_political_chart").unwrap().kind,
            GfxResourceKind::PieChart
        );
        assert_eq!(
            index.get("GFX_political_chart").unwrap().size,
            Some(GfxSize { x: 27.0, y: 27.0 })
        );
    }

    #[test]
    fn first_resource_definition_wins_for_override_order() {
        let mut index = GfxIndex {
            resources: HashMap::new(),
            diagnostics: VanillaGuiDiagnostics::default(),
        };
        for resource in parse_gfx_resources(
            Some(PathBuf::from("mod.gfx")),
            r#"spriteTypes = { spriteType = { name = "GFX_a" texturefile = "gfx/mod.dds" } }"#,
        ) {
            index.insert_first(resource);
        }
        for resource in parse_gfx_resources(
            Some(PathBuf::from("vanilla.gfx")),
            r#"spriteTypes = { spriteType = { name = "GFX_a" texturefile = "gfx/vanilla.dds" } }"#,
        ) {
            index.insert_first(resource);
        }

        assert_eq!(
            index.get("GFX_a").unwrap().primary_texture.as_deref(),
            Some("gfx/mod.dds")
        );
    }

    #[test]
    fn gate3_gfx_keeps_effect_files_and_unknown_fields_for_later_semantics() {
        let index = GfxIndex::parse_single(
            "effects.gfx",
            r#"
spriteTypes = {
    spriteType = {
        name = "GFX_button"
        textureFile = "gfx/interface/button.dds"
        noOfFrames = 3
        effectFile = "gfx/FX/buttonstate.lua"
        blendFrames = yes
        unknownFutureField = "keep-me"
    }
    progressbartype = {
        name = "GFX_progress"
        textureFile1 = "gfx/interface/progress.dds"
        textureFile2 = "gfx/interface/progress_bg.dds"
        horizontal = yes
        effectFile = "gfx/FX/progress.lua"
        size = { width = 237 height = 6 }
    }
    corneredTileSpriteType = {
        name = "GFX_corner"
        textureFile = "gfx/interface/corner.dds"
        borderSize = { left = 6 top = 7 right = 8 bottom = 9 }
        tilingCenter = no
    }
}
"#,
        );

        let button = index.get("GFX_button").unwrap();
        assert_eq!(button.frame_count, Some(3));
        assert!(button.raw_properties.iter().any(|prop| {
            prop.key.eq_ignore_ascii_case("effectFile") && prop.value == "gfx/FX/buttonstate.lua"
        }));
        assert!(button
            .raw_properties
            .iter()
            .any(|prop| { prop.key == "unknownFutureField" && prop.value == "keep-me" }));

        let progress = index.get("GFX_progress").unwrap();
        assert_eq!(progress.kind, GfxResourceKind::ProgressBar);
        assert_eq!(progress.size, Some(GfxSize { x: 237.0, y: 6.0 }));
        assert!(progress
            .raw_properties
            .iter()
            .any(|prop| prop.key.eq_ignore_ascii_case("horizontal") && prop.value == "yes"));

        let corner = index.get("GFX_corner").unwrap();
        assert_eq!(
            corner.border,
            Some(GfxBorder {
                left: 6.0,
                top: 7.0,
                right: 8.0,
                bottom: 9.0
            })
        );
        assert!(corner
            .raw_properties
            .iter()
            .any(|prop| prop.key.eq_ignore_ascii_case("tilingCenter") && prop.value == "no"));
    }

    #[test]
    fn real_politics_resources_hit_when_vanilla_available() {
        let Ok(path_cfg) = hoi4_paths::PathConfig::resolve(Default::default()) else {
            return;
        };
        let Some(gui_path) = path_cfg.find("interface/countrypoliticsview.gui") else {
            return;
        };
        let gui = parse_gui_file(gui_path).unwrap();
        let refs = collect_gfx_references(&gui);
        let index = GfxIndex::from_path_config(&path_cfg);
        let report = index.hit_report(refs.iter().map(String::as_str));

        assert_eq!(refs.len(), 79);
        assert!(
            report.all_hit(),
            "missing politics resources: {:?}",
            report.missing
        );
        for key in [
            "GFX_tiled_plain_bg",
            "GFX_tiled_window_1b_border",
            "GFX_player_flag",
            "GFX_flag_small2",
            "GFX_political_chart",
            "GFX_activegoal_progress",
        ] {
            assert!(index.contains(key), "missing key resource {key}");
        }
    }
}
