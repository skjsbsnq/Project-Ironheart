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
pub mod runtime;
pub mod template_instancer;

pub use animation::{
    clear_panel_close_request, panel_animation_store_id, panel_close_requested_id,
    request_panel_close, update_panel_animation, AnimationCurve, AnimationPhase, AnimationSpec,
    PanelAnimationStore, PanelAnimationUpdate,
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
    compute_layout_tree, compute_layout_tree_with_path, focus_spacing_marker, grid_slots,
    link_begin_marker, link_end_marker, link_spacing_marker, national_focus_center_marker,
    position_marker, GuiDim, GuiMargin, GuiMarker, GuiMarkerLookup, GuiOrientation, GuiPoint,
    GuiRect, GuiSize, LayoutNode, LayoutOptions, ScrollSpec,
};
pub use profile::{
    bind_profile_tree, bind_profile_tree_with_path, VanillaPanelProfile, VanillaProfileDescriptor,
    VanillaProfileRegistry, VanillaTemplateInstance,
};
pub use renderer::{
    button_frame_for_state, checkbox_frame_for_state, frame_animated_frame, resource_hit_rect,
    resource_hit_rect_centered, ButtonVisualState, FontToken, RenderStats, VanillaGuiRenderer,
};
pub use runtime::{
    collect_profile_gfx_references, country_politics_runtime_context, required_focus_marker_report,
    vanilla_builtin_profile_descriptors, vanilla_builtin_runtime_context,
    vanilla_profile_diagnostics_markdown, vanilla_profiles_diagnostics_markdown,
    VanillaGuiDiagnosticCategory, VanillaGuiDiagnosticLine, VanillaGuiProfileDiagnostics,
    VanillaGuiRuntimeContext, VanillaGuiRuntimeLoadError, VanillaGuiRuntimeUnavailableReport,
    VanillaProfileLoadReport, COUNTRY_DECISION_DESCRIPTOR, COUNTRY_DECISION_GUI_FILE,
    COUNTRY_DECISION_PROFILE_ID, COUNTRY_DECISION_ROOT, COUNTRY_POLITICS_DESCRIPTOR,
    COUNTRY_POLITICS_GUI_FILE, COUNTRY_POLITICS_PROFILE_ID, COUNTRY_POLITICS_ROOT,
    DECISION_REQUIRED_SPRITES, FOCUS_REQUIRED_SPRITES, NATIONAL_FOCUS_DESCRIPTOR,
    NATIONAL_FOCUS_GUI_FILE, NATIONAL_FOCUS_PROFILE_ID, NATIONAL_FOCUS_ROOT,
    POLITICS_REQUIRED_SPRITES,
};
pub use template_instancer::{
    template_instance_path, TemplateInstanceLayout, TemplateInstanceOptions, TemplateInstancer,
};
