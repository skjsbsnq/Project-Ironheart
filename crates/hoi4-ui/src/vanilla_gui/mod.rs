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
pub mod draw_command;
pub mod error;
pub mod gfx_index;
pub mod intrinsic;
pub mod layout;
pub mod profile;
pub mod renderer;
pub mod runtime;
pub mod runtime_frame;
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
pub use draw_command::{
    apply_alpha_to_tint, button_frame_for_effect, button_frame_for_state, checkbox_frame_for_state,
    collect_draw_commands_for_tree, frame_animated_frame, progress_fill_for_effect, raw_property,
    ButtonVisualState, GuiAlphaSemantics, GuiBlendMode, GuiButtonEffectPolicy, GuiDrawCommand,
    GuiDrawCommandDiagnostics, GuiDrawCommandKind, GuiDrawCommandSummary, GuiDrawState,
    GuiFallbackDrawCommand, GuiPieChartDrawCommand, GuiProgressDrawCommand,
    GuiProgressEffectPolicy, GuiSpriteDrawCommand, GuiTextDrawCommand,
    GuiUnsupportedEffectDiagnostic,
};
pub use error::{VanillaGuiError, VanillaGuiIssue, VanillaGuiIssueKind};
pub use gfx_index::{GfxIndex, GfxResource, GfxResourceKind, GfxTypeDistribution};
pub use intrinsic::{
    collect_resource_names_from_node, collect_resource_names_from_node_with_bindings,
    node_resource_name, node_resource_names, rect_is_physical_pixel_aligned, resolve_text_layout,
    snap_rect_to_physical_pixels, snap_value, FontToken, GuiControlRects, GuiFontMetrics,
    GuiIntrinsicAnchor, GuiIntrinsicDiagnostic, GuiIntrinsicSize, GuiIntrinsicSizeResolver,
    GuiIntrinsicSizeSource, GuiLayoutIntrinsics, GuiPixelSnapMode, GuiTextHorizontalAlign,
    GuiTextLayout, GuiTextVerticalAlign,
};
pub use layout::{
    compute_layout_tree, compute_layout_tree_with_path, focus_spacing_marker, grid_slots,
    link_begin_marker, link_end_marker, link_spacing_marker, national_focus_center_marker,
    position_marker, GuiDim, GuiMargin, GuiMarker, GuiMarkerLookup, GuiOrientation, GuiPoint,
    GuiRect, GuiSize, LayoutNode, LayoutOptions, ScrollSpec,
};
pub use profile::{
    bind_profile_tree, bind_profile_tree_with_path, bind_profile_tree_with_path_and_context,
    GuiInstanceContext, VanillaPanelProfile, VanillaProfileDescriptor, VanillaProfileRegistry,
    VanillaTemplateInstance,
};
pub use renderer::{
    resource_hit_rect, resource_hit_rect_centered, RenderStats, VanillaGuiRenderer,
};
pub use runtime::{
    collect_profile_gfx_references, country_diplomacy_is_excluded_template,
    country_diplomacy_no_intel_bindings, country_diplomacy_runtime_context,
    country_logistics_runtime_context, country_politics_runtime_context,
    required_focus_marker_report, vanilla_builtin_profile_descriptors,
    vanilla_builtin_runtime_context, vanilla_profile_diagnostics_markdown,
    vanilla_profiles_diagnostics_markdown, VanillaGuiDiagnosticCategory, VanillaGuiDiagnosticLine,
    VanillaGuiProfileDiagnostics, VanillaGuiRuntimeContext, VanillaGuiRuntimeLoadError,
    VanillaGuiRuntimeUnavailableReport, VanillaProfileLoadReport, COUNTRY_DECISION_DESCRIPTOR,
    COUNTRY_DECISION_GUI_FILE, COUNTRY_DECISION_PROFILE_ID, COUNTRY_DECISION_ROOT,
    COUNTRY_DIPLOMACY_DESCRIPTOR, COUNTRY_DIPLOMACY_GFX_FILE, COUNTRY_DIPLOMACY_GUI_FILE,
    COUNTRY_DIPLOMACY_PROFILE_ID, COUNTRY_DIPLOMACY_ROOT, COUNTRY_LOGISTICS_DESCRIPTOR,
    COUNTRY_LOGISTICS_GFX_FILE, COUNTRY_LOGISTICS_GUI_FILE, COUNTRY_LOGISTICS_PROFILE_ID,
    COUNTRY_LOGISTICS_ROOT, COUNTRY_POLITICS_DESCRIPTOR, COUNTRY_POLITICS_GUI_FILE,
    COUNTRY_POLITICS_PROFILE_ID, COUNTRY_POLITICS_ROOT, DECISION_REQUIRED_SPRITES,
    DIPLOMACY_EXCLUDED_TEMPLATES, DIPLOMACY_INTEL_HIDDEN_NODES, DIPLOMACY_KEY_TEMPLATES,
    DIPLOMACY_REQUIRED_SPRITES, FOCUS_REQUIRED_SPRITES, LOGISTICS_REQUIRED_SPRITES,
    NATIONAL_FOCUS_DESCRIPTOR, NATIONAL_FOCUS_GUI_FILE, NATIONAL_FOCUS_PROFILE_ID,
    NATIONAL_FOCUS_ROOT, POLITICS_REQUIRED_SPRITES,
};
pub use runtime_frame::{
    generate_runtime_instances, GuiGeneratedInstance, GuiHitRegion, GuiRuntimeFrame,
    GuiRuntimeFrameInput, GuiRuntimeInstanceSource, GuiRuntimeInstanceSpec, GuiRuntimeRootPosition,
    GuiRuntimeState, GuiScrollState, GuiTemplateDiagnostic, GuiTemplateDiagnostics, GuiTemplateRef,
    GuiTemplateRegistry, VanillaUnsupportedSemanticsCategory, VanillaUnsupportedSemanticsEntry,
    VanillaUnsupportedSemanticsRegistry,
};
pub use template_instancer::{
    template_instance_path, TemplateInstanceLayout, TemplateInstanceOptions, TemplateInstancer,
};
