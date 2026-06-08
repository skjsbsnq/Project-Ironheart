use std::collections::{BTreeSet, HashMap};
use std::fmt;
use std::path::{Path, PathBuf};

use clausewitz_parser::{parse, Block, Entry, Value};

use super::diagnostics::VanillaGuiDiagnostics;
use super::error::{VanillaGuiError, VanillaGuiIssue, VanillaGuiIssueKind};

#[derive(Debug, Clone)]
pub struct GuiDocument {
    pub source: Option<PathBuf>,
    pub roots: Vec<GuiNode>,
    /// All `GFX_*` tokens in the source text, including commented references.
    /// Diagnostics use this to match vanilla audit counts; rendering still walks
    /// active parsed nodes only.
    pub gfx_tokens: Vec<String>,
    pub diagnostics: VanillaGuiDiagnostics,
}

impl GuiDocument {
    pub fn template_index(&self) -> GuiTemplateIndex<'_> {
        GuiTemplateIndex::new(&self.roots)
    }

    pub fn root_names(&self) -> Vec<&str> {
        self.roots
            .iter()
            .filter_map(|node| node.name.as_deref())
            .collect()
    }

    pub fn find_node_by_name(&self, name: &str) -> Option<&GuiNode> {
        self.roots
            .iter()
            .find_map(|node| node.find_node_by_name(name))
    }

    pub fn node_count(&self) -> usize {
        self.roots.iter().map(GuiNode::node_count).sum()
    }
}

#[derive(Debug, Clone)]
pub struct GuiNode {
    pub kind: GuiNodeKind,
    pub name: Option<String>,
    pub properties: Vec<GuiProperty>,
    pub children: Vec<GuiNode>,
    /// Original block for unsupported properties and future panel profiles.
    pub raw: Block,
}

impl GuiNode {
    pub fn prop(&self, key: &str) -> Option<&Value> {
        self.properties
            .iter()
            .find(|prop| prop.key.eq_ignore_ascii_case(key))
            .map(|prop| &prop.value)
    }

    pub fn prop_entry(&self, key: &str) -> Option<&GuiProperty> {
        self.properties
            .iter()
            .find(|prop| prop.key.eq_ignore_ascii_case(key))
    }

    pub fn string(&self, key: &str) -> Option<String> {
        self.prop(key).and_then(GuiValueExt::as_lossy_string)
    }

    pub fn f32(&self, key: &str) -> Option<f32> {
        self.prop(key).and_then(GuiValueExt::as_lossy_f32)
    }

    pub fn bool(&self, key: &str) -> Option<bool> {
        self.prop(key).and_then(GuiValueExt::as_lossy_bool)
    }

    pub fn block(&self, key: &str) -> Option<&Block> {
        match self.prop(key) {
            Some(Value::Block(block)) => Some(block),
            _ => None,
        }
    }

    pub fn child_named(&self, name: &str) -> Option<&GuiNode> {
        self.children
            .iter()
            .find_map(|child| child.find_node_by_name(name))
    }

    pub fn direct_child_named(&self, name: &str) -> Option<&GuiNode> {
        self.children
            .iter()
            .find(|child| child.name.as_deref() == Some(name))
    }

    pub fn find_node_by_name(&self, name: &str) -> Option<&GuiNode> {
        if self.name.as_deref() == Some(name) {
            return Some(self);
        }
        self.children
            .iter()
            .find_map(|child| child.find_node_by_name(name))
    }

    pub fn node_count(&self) -> usize {
        1 + self.children.iter().map(GuiNode::node_count).sum::<usize>()
    }

    pub fn path_label(&self, index: usize) -> String {
        self.name
            .clone()
            .unwrap_or_else(|| format!("{}#{index}", self.kind.as_key()))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum GuiNodeKind {
    ContainerWindow,
    Icon,
    Button,
    CheckBox,
    EditBox,
    InstantTextbox,
    GridBox,
    OverlappingElementsBox,
    Position,
    Background,
    VerticalScrollbar,
    Unknown(String),
}

impl GuiNodeKind {
    pub fn from_key(key: &str) -> Option<Self> {
        match key.to_ascii_lowercase().as_str() {
            "containerwindowtype" => Some(Self::ContainerWindow),
            "icontype" => Some(Self::Icon),
            "buttontype" => Some(Self::Button),
            "checkboxtype" => Some(Self::CheckBox),
            "editboxtype" => Some(Self::EditBox),
            "instanttextboxtype" => Some(Self::InstantTextbox),
            "gridboxtype" => Some(Self::GridBox),
            "overlappingelementsboxtype" => Some(Self::OverlappingElementsBox),
            "positiontype" => Some(Self::Position),
            "background" => Some(Self::Background),
            "verticalscrollbar" => Some(Self::VerticalScrollbar),
            _ => None,
        }
    }

    pub fn as_key(&self) -> &str {
        match self {
            Self::ContainerWindow => "containerWindowType",
            Self::Icon => "iconType",
            Self::Button => "buttonType",
            Self::CheckBox => "checkBoxType",
            Self::EditBox => "editBoxType",
            Self::InstantTextbox => "instantTextboxType",
            Self::GridBox => "gridBoxType",
            Self::OverlappingElementsBox => "OverlappingElementsBoxType",
            Self::Position => "positionType",
            Self::Background => "background",
            Self::VerticalScrollbar => "verticalScrollbar",
            Self::Unknown(key) => key.as_str(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct GuiProperty {
    pub key: String,
    pub value: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct GuiNodePath(pub Vec<String>);

impl GuiNodePath {
    pub fn root(label: impl Into<String>) -> Self {
        Self(vec![label.into()])
    }

    pub fn child(&self, label: impl Into<String>) -> Self {
        let mut next = self.0.clone();
        next.push(label.into());
        Self(next)
    }
}

impl fmt::Display for GuiNodePath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.join("."))
    }
}

pub trait GuiValueExt {
    fn as_lossy_string(&self) -> Option<String>;
    fn as_lossy_f32(&self) -> Option<f32>;
    fn as_lossy_bool(&self) -> Option<bool>;
}

impl GuiValueExt for Value {
    fn as_lossy_string(&self) -> Option<String> {
        match self {
            Value::String(value) => Some(value.clone()),
            Value::Integer(value) => Some(value.to_string()),
            Value::Float(value) => Some(trim_float(*value)),
            Value::Percent(value) => Some(format!("{value}%%")),
            Value::Bool(value) => Some(if *value { "yes" } else { "no" }.to_owned()),
            Value::Block(_) => None,
        }
    }

    fn as_lossy_f32(&self) -> Option<f32> {
        match self {
            Value::Integer(value) => Some(*value as f32),
            Value::Float(value) => Some(*value as f32),
            Value::Percent(value) => Some(*value as f32),
            Value::String(value) => value.parse::<f32>().ok(),
            Value::Bool(_) | Value::Block(_) => None,
        }
    }

    fn as_lossy_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(value) => Some(*value),
            Value::String(value) if value.eq_ignore_ascii_case("yes") => Some(true),
            Value::String(value) if value.eq_ignore_ascii_case("no") => Some(false),
            _ => None,
        }
    }
}

pub struct GuiTemplateIndex<'a> {
    by_name: HashMap<&'a str, &'a GuiNode>,
}

impl<'a> GuiTemplateIndex<'a> {
    pub fn new(roots: &'a [GuiNode]) -> Self {
        let mut by_name = HashMap::new();
        for node in roots {
            if let Some(name) = node.name.as_deref() {
                by_name.entry(name).or_insert(node);
            }
        }
        Self { by_name }
    }

    pub fn get(&self, name: &str) -> Option<&'a GuiNode> {
        self.by_name.get(name).copied()
    }

    pub fn len(&self) -> usize {
        self.by_name.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_name.is_empty()
    }

    pub fn names(&self) -> Vec<&'a str> {
        let mut names: Vec<&str> = self.by_name.keys().copied().collect();
        names.sort_unstable();
        names
    }
}

pub fn parse_gui_file(path: impl AsRef<Path>) -> Result<GuiDocument, VanillaGuiError> {
    let path = path.as_ref();
    if !path.exists() {
        return Err(VanillaGuiError::MissingGui(path.to_path_buf()));
    }
    let text = std::fs::read_to_string(path).map_err(|err| VanillaGuiError::Io {
        path: path.to_path_buf(),
        message: err.to_string(),
    })?;
    Ok(parse_gui_str(Some(path.to_path_buf()), &text))
}

pub fn parse_gui_str(source: Option<PathBuf>, text: &str) -> GuiDocument {
    let block = parse(text);
    let mut roots = Vec::new();
    collect_root_nodes(&block, &mut roots);
    let gfx_tokens = collect_gfx_tokens_from_text(text);
    let mut diagnostics = VanillaGuiDiagnostics::default();
    diagnostics.loaded_gui_files = 1;
    diagnostics.node_count = roots.iter().map(GuiNode::node_count).sum();
    for node in &roots {
        collect_unknown_node_issues(node, source.as_ref(), &mut diagnostics);
    }
    GuiDocument {
        source,
        roots,
        gfx_tokens,
        diagnostics,
    }
}

pub fn collect_gfx_references(document: &GuiDocument) -> Vec<String> {
    let mut refs = BTreeSet::new();
    refs.extend(document.gfx_tokens.iter().cloned());
    for root in &document.roots {
        collect_gfx_references_node(root, &mut refs);
    }
    refs.into_iter().collect()
}

fn collect_root_nodes(block: &Block, out: &mut Vec<GuiNode>) {
    for entry in &block.entries {
        if let Some(node) = node_from_entry(entry) {
            out.push(node);
            continue;
        }
        if matches!(entry.key.as_str(), "guiTypes" | "containerTypes") {
            if let Value::Block(inner) = &entry.value {
                for child_entry in &inner.entries {
                    if let Some(node) = node_from_entry(child_entry) {
                        out.push(node);
                    }
                }
            }
        }
    }
}

fn node_from_entry(entry: &Entry) -> Option<GuiNode> {
    let Value::Block(block) = &entry.value else {
        return None;
    };
    let kind = GuiNodeKind::from_key(&entry.key).or_else(|| {
        looks_like_gui_node_key(&entry.key).then(|| GuiNodeKind::Unknown(entry.key.clone()))
    })?;
    Some(node_from_block(kind, block.clone()))
}

fn looks_like_gui_node_key(key: &str) -> bool {
    key.to_ascii_lowercase().ends_with("type")
}

fn node_from_block(kind: GuiNodeKind, block: Block) -> GuiNode {
    let mut properties = Vec::new();
    let mut children = Vec::new();
    for entry in &block.entries {
        if let Some(child) = node_from_entry(entry) {
            children.push(child);
        } else {
            properties.push(GuiProperty {
                key: entry.key.clone(),
                value: entry.value.clone(),
            });
        }
    }
    let name = properties
        .iter()
        .find(|prop| prop.key.eq_ignore_ascii_case("name"))
        .and_then(|prop| prop.value.as_lossy_string());
    GuiNode {
        kind,
        name,
        properties,
        children,
        raw: block,
    }
}

fn collect_gfx_references_node(node: &GuiNode, out: &mut BTreeSet<String>) {
    for prop in &node.properties {
        collect_gfx_references_value(&prop.value, out);
    }
    for child in &node.children {
        collect_gfx_references_node(child, out);
    }
}

fn collect_gfx_references_value(value: &Value, out: &mut BTreeSet<String>) {
    match value {
        Value::String(value) if value.starts_with("GFX_") => {
            out.insert(value.clone());
        }
        Value::Block(block) => {
            for entry in &block.entries {
                collect_gfx_references_value(&entry.value, out);
            }
            for value in &block.values {
                collect_gfx_references_value(value, out);
            }
        }
        _ => {}
    }
}

fn collect_gfx_tokens_from_text(text: &str) -> Vec<String> {
    let bytes = text.as_bytes();
    let mut out = BTreeSet::new();
    let mut i = 0usize;
    while i + 4 <= bytes.len() {
        if &bytes[i..i + 4] == b"GFX_" {
            let start = i;
            i += 4;
            while i < bytes.len() {
                let ch = bytes[i];
                if ch.is_ascii_alphanumeric() || ch == b'_' {
                    i += 1;
                } else {
                    break;
                }
            }
            if let Ok(token) = std::str::from_utf8(&bytes[start..i]) {
                out.insert(token.to_owned());
            }
        } else {
            i += 1;
        }
    }
    out.into_iter().collect()
}

fn collect_unknown_node_issues(
    node: &GuiNode,
    source: Option<&PathBuf>,
    diagnostics: &mut VanillaGuiDiagnostics,
) {
    if let GuiNodeKind::Unknown(kind) = &node.kind {
        let detail = format!("unknown gui node type `{kind}`");
        let issue = match source {
            Some(source) => {
                VanillaGuiIssue::with_source(VanillaGuiIssueKind::UnknownNodeType, source, detail)
            }
            None => VanillaGuiIssue::new(VanillaGuiIssueKind::UnknownNodeType, detail),
        };
        diagnostics.add_issue(issue);
    }
    for child in &node.children {
        collect_unknown_node_issues(child, source, diagnostics);
    }
}

fn trim_float(value: f64) -> String {
    let s = value.to_string();
    s.strip_suffix(".0").unwrap_or(&s).to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_gui_types_wrapper_and_indexes_templates() {
        let doc = parse_gui_str(
            None,
            r#"
guiTypes = {
    containerWindowType = {
        name = "root"
        position = { x=-606 y=78 }
        size = { width=550 height=100%% }
        background = { quadTextureSprite = "GFX_tiled_plain_bg" }
        gridboxtype = {
            name = "ideas_grid"
            slotsize = { width = 80 height = 64 }
            max_slots = { x = 7 y = 1 }
        }
    }
    containerWindowType = {
        name = "political_ideas_window"
        position = { x = -356 y = 80 }
        show_position = { x = 540 y = 80 }
    }
}
"#,
        );

        assert_eq!(doc.roots.len(), 2);
        assert_eq!(doc.template_index().len(), 2);
        assert!(doc.template_index().get("root").is_some());
        assert!(doc.find_node_by_name("ideas_grid").is_some());
        assert_eq!(
            collect_gfx_references(&doc),
            vec!["GFX_tiled_plain_bg".to_owned()]
        );
    }

    #[test]
    fn tolerates_weird_position_values() {
        let doc = parse_gui_str(
            None,
            r#"
guiTypes = {
    containerWindowType = {
        name = "odd"
        position = { x = -2 y = 1s }
    }
}
"#,
        );
        let node = doc.find_node_by_name("odd").unwrap();
        let position = node.block("position").unwrap();
        assert_eq!(position.get_int("x"), Some(-2));
        assert_eq!(position.get_string("y"), Some("1s"));
    }

    #[test]
    fn recognizes_vanilla_instant_textbox_spelling() {
        let doc = parse_gui_str(
            None,
            r#"
guiTypes = {
    containerWindowType = {
        name = "root"
        instantTextBoxType = { name = "Title" text = "Hello" }
    }
}
"#,
        );
        let root = doc.template_index().get("root").unwrap();
        let title = root.find_node_by_name("Title").unwrap();

        assert_eq!(title.kind, GuiNodeKind::InstantTextbox);
    }

    #[test]
    fn gate4_unknown_gui_node_preserves_nested_children() {
        let doc = parse_gui_str(
            None,
            r#"
guiTypes = {
    futureWidgetType = {
        name = "future_root"
        custom_property = yes
        iconType = {
            name = "future_icon"
            spriteType = "GFX_future"
        }
        instantTextboxType = {
            name = "future_label"
            text = "Future"
        }
    }
}
"#,
        );

        let root = doc.template_index().get("future_root").unwrap();
        assert_eq!(
            root.kind,
            GuiNodeKind::Unknown("futureWidgetType".to_owned())
        );
        assert!(root
            .properties
            .iter()
            .any(|property| property.key == "custom_property"));
        assert_eq!(
            root.find_node_by_name("future_icon").map(|node| &node.kind),
            Some(&GuiNodeKind::Icon)
        );
        assert_eq!(
            root.find_node_by_name("future_label")
                .map(|node| &node.kind),
            Some(&GuiNodeKind::InstantTextbox)
        );
        assert!(doc
            .diagnostics
            .issues
            .iter()
            .any(|issue| issue.kind == VanillaGuiIssueKind::UnknownNodeType
                && issue.detail.contains("futureWidgetType")));
    }

    #[test]
    fn gate5_recognizes_decision_and_focus_required_node_types() {
        let doc = parse_gui_str(
            None,
            r#"
guiTypes = {
    containerWindowType = {
        name = "root"
        checkBoxType = { name = "tracked" }
        editBoxType = { name = "search" }
        OverlappingElementsBoxType = { name = "overlap" }
        positionType = { name = "focus_spacing" position = { x = 96 y = 130 } }
    }
}
"#,
        );
        let root = doc.template_index().get("root").unwrap();

        assert_eq!(
            root.find_node_by_name("tracked").map(|node| &node.kind),
            Some(&GuiNodeKind::CheckBox)
        );
        assert_eq!(
            root.find_node_by_name("search").map(|node| &node.kind),
            Some(&GuiNodeKind::EditBox)
        );
        assert_eq!(
            root.find_node_by_name("overlap").map(|node| &node.kind),
            Some(&GuiNodeKind::OverlappingElementsBox)
        );
        assert_eq!(
            root.find_node_by_name("focus_spacing")
                .map(|node| &node.kind),
            Some(&GuiNodeKind::Position)
        );
    }

    #[test]
    fn diagnostics_keep_commented_gfx_tokens() {
        let doc = parse_gui_str(
            None,
            r#"
guiTypes = {
    containerWindowType = {
        name = "root"
        # spriteType = "GFX_commented"
        iconType = { spriteType = "GFX_active" }
    }
}
"#,
        );
        assert_eq!(
            collect_gfx_references(&doc),
            vec!["GFX_active".to_owned(), "GFX_commented".to_owned()]
        );
    }

    #[test]
    fn real_politics_gui_templates_when_vanilla_available() {
        let Ok(path_cfg) = hoi4_paths::PathConfig::resolve(Default::default()) else {
            return;
        };
        let Some(gui_path) = path_cfg.find("interface/countrypoliticsview.gui") else {
            return;
        };
        let doc = parse_gui_file(gui_path).unwrap();
        let index = doc.template_index();

        assert_eq!(doc.roots.len(), 18);
        for name in [
            "countrypoliticsview",
            "country_politics_idea_category_entry",
            "political_ideas_window",
            "political_selectable_idea_entry_list",
        ] {
            assert!(index.get(name).is_some(), "missing template {name}");
        }
    }

    #[test]
    fn gate4_decision_and_focus_real_gui_node_counts_do_not_regress() {
        let Ok(path_cfg) = hoi4_paths::PathConfig::resolve(Default::default()) else {
            return;
        };

        for (rel_path, old_parser_count, required_templates) in [
            (
                "interface/countrydecisionview.gui",
                78usize,
                &["countrydecisionview", "decision_grid", "decision_item"][..],
            ),
            (
                "interface/nationalfocusview.gui",
                123usize,
                &["nationalfocusview", "tree", "national_focus_item"][..],
            ),
        ] {
            let Some(gui_path) = path_cfg.find(rel_path) else {
                return;
            };
            let doc = parse_gui_file(gui_path).unwrap();

            assert!(
                doc.node_count() >= old_parser_count,
                "{rel_path} node count {} below old parser baseline {old_parser_count}",
                doc.node_count()
            );
            for template in required_templates {
                assert!(
                    doc.find_node_by_name(template).is_some(),
                    "{rel_path} missing {template}"
                );
            }
        }
    }

    #[test]
    fn gate5_real_decision_and_focus_required_nodes_enter_ast() {
        let Ok(path_cfg) = hoi4_paths::PathConfig::resolve(Default::default()) else {
            return;
        };

        let Some(decision_gui_path) = path_cfg.find("interface/countrydecisionview.gui") else {
            return;
        };
        let decision_doc = parse_gui_file(decision_gui_path).unwrap();
        assert!(contains_kind(&decision_doc.roots, &GuiNodeKind::CheckBox));
        assert!(contains_kind(
            &decision_doc.roots,
            &GuiNodeKind::OverlappingElementsBox
        ));

        let Some(focus_gui_path) = path_cfg.find("interface/nationalfocusview.gui") else {
            return;
        };
        let focus_doc = parse_gui_file(focus_gui_path).unwrap();
        assert!(contains_kind(&focus_doc.roots, &GuiNodeKind::EditBox));
        assert!(contains_kind(&focus_doc.roots, &GuiNodeKind::Position));
    }

    fn contains_kind(nodes: &[GuiNode], kind: &GuiNodeKind) -> bool {
        nodes
            .iter()
            .any(|node| &node.kind == kind || contains_kind(&node.children, kind))
    }
}
