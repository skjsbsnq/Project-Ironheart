//! Reusable vanilla HOI4 `.gui` / `.gfx` runtime subset.
//!
//! This module is intentionally inside `hoi4-ui`: the first runtime slice needs
//! direct access to egui, the existing font/theme tokens, and `IconBank`.  It does
//! not own gameplay state.  Callers provide a per-frame data snapshot through a
//! panel profile/binding layer, and the runtime turns vanilla node metadata into
//! layout, animation, hit regions, and paint primitives.
//!
//! Module boundaries:
//! - [`ast`]: tolerant `.gui` AST and template lookup.
//! - [`gfx_index`]: `interface/**/*.gfx` resource metadata index.
//! - [`layout`]: absolute/percent layout, anchors, clipping, grids, scroll specs.
//! - [`animation`]: panel show/hide state for egui immediate mode.
//! - [`renderer`]: egui paint primitives for sprites, text, buttons, bars, charts.
//! - [`binding`]: dynamic text/sprite/progress/visibility/action data.
//! - [`profile`]: panel profile trait; politics is only one future consumer.
//! - [`diagnostics`]: non-fatal load/render diagnostics.

pub mod animation;
pub mod ast;
pub mod binding;
pub mod diagnostics;
pub mod error;
pub mod gfx_index;
pub mod layout;
pub mod profile;
pub mod renderer;

pub use animation::{
    AnimationCurve, AnimationPhase, AnimationSpec, PanelAnimationStore, PanelAnimationUpdate,
};
pub use ast::{
    collect_gfx_references, parse_gui_file, parse_gui_str, GuiDocument, GuiNode, GuiNodeKind,
    GuiNodePath, GuiProperty, GuiTemplateIndex, GuiValueExt,
};
pub use binding::{GuiAction, GuiActionKind, GuiBinding, GuiBindingMap, GuiClickCommand};
pub use diagnostics::{GfxHitReport, VanillaGuiDiagnostics};
pub use error::{VanillaGuiError, VanillaGuiIssue, VanillaGuiIssueKind};
pub use gfx_index::{GfxIndex, GfxResource, GfxResourceKind, GfxTypeDistribution};
pub use layout::{
    compute_layout_tree, compute_layout_tree_with_path, grid_slots, GuiDim, GuiOrientation,
    GuiPoint, GuiRect, GuiSize, LayoutNode, LayoutOptions, ScrollSpec,
};
pub use profile::{
    bind_profile_tree, bind_profile_tree_with_path, VanillaPanelProfile, VanillaProfileDescriptor,
    VanillaProfileRegistry, VanillaTemplateInstance,
};
pub use renderer::{
    button_frame_for_state, frame_animated_frame, resource_hit_rect, ButtonVisualState, FontToken,
    RenderStats, VanillaGuiRenderer,
};
