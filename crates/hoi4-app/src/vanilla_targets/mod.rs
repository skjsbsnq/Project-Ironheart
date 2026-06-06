pub mod fow;
pub mod gradient_border;
pub mod point_lights;
pub mod province_secondary;

use std::time::Instant;

use hoi4_render::map_mode::MapMode;
use hoi4_render::railways::compute_province_centroids;
use hoi4_state::{CountryId, World};

use crate::vanilla_resource_views::{BindingAuditEntry, BindingBlockingLevel, BindingProvenance};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeTargetChannelSemantic {
    CountryBorderGradient,
    ProvinceBorderGradient,
    StateSeaImpassableGradient,
    ProvinceSecondaryColor,
    FogOfWar,
    IntelMap,
    ProjectedShadowFow,
    MudSnow,
    PointLightData,
    PointLightIndex,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeTargetSpec {
    pub name: &'static str,
    pub format: wgpu::TextureFormat,
    pub bytes_per_pixel: u32,
    pub semantic: RuntimeTargetChannelSemantic,
    pub dimensions: &'static str,
    pub source_detail: &'static str,
    pub source_trace: &'static str,
    pub parity_status: &'static str,
    pub provenance: BindingProvenance,
    pub producer_status: &'static str,
    pub producer_equivalent: bool,
    pub blocking_level: BindingBlockingLevel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProjectedShadowFowProducerStage {
    pub order: u16,
    pub stage: &'static str,
    pub target: &'static str,
    pub source: &'static str,
    pub implemented: bool,
}

pub const PROJECTED_SHADOW_FOW_PRODUCER_STATUS: &str = "ShadowMap_ProjectFOW=missing/fallback";

pub const PROJECTED_SHADOW_FOW_PRODUCER_STAGES: [ProjectedShadowFowProducerStage; 4] = [
    ProjectedShadowFowProducerStage {
        order: 77,
        stage: "tree/projected",
        target: "ShadowMap",
        source: "tree.shader projected producer",
        implemented: false,
    },
    ProjectedShadowFowProducerStage {
        order: 78,
        stage: "terrainunlit/projected",
        target: "ShadowMap",
        source: "pdxmap.shader terrainunlit projected producer",
        implemented: false,
    },
    ProjectedShadowFowProducerStage {
        order: 79,
        stage: "shadowblur/fullres_horizontal",
        target: "ShadowMap_ProjectFOW_BlurTemp",
        source: "shadowblur.shader full-resolution blur pass 1",
        implemented: false,
    },
    ProjectedShadowFowProducerStage {
        order: 80,
        stage: "shadowblur/fullres_vertical",
        target: "ShadowMap",
        source: "shadowblur.shader full-resolution blur pass 2",
        implemented: false,
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProjectedShadowFowProducerPlan {
    pub status: &'static str,
    pub output_target: &'static str,
    pub temp_target: &'static str,
    pub output_dimensions: &'static str,
    pub temp_dimensions: &'static str,
    pub stages: &'static [ProjectedShadowFowProducerStage],
}

pub const PROJECTED_SHADOW_FOW_PRODUCER_PLAN: ProjectedShadowFowProducerPlan =
    ProjectedShadowFowProducerPlan {
        status: PROJECTED_SHADOW_FOW_PRODUCER_STATUS,
        output_target: "ShadowMap",
        temp_target: "ShadowMap_ProjectFOW_BlurTemp",
        output_dimensions: "B8G8R8A8_UNORM 2560x1600",
        temp_dimensions: "B8G8R8A8_UNORM 2560x1600",
        stages: &PROJECTED_SHADOW_FOW_PRODUCER_STAGES,
    };

pub const PHASE4_RUNTIME_TARGET_SPECS: [RuntimeTargetSpec; 8] = [
    RuntimeTargetSpec {
        name: "GradientBorderChannel1",
        format: wgpu::TextureFormat::Bgra8UnormSrgb,
        bytes_per_pixel: 4,
        semantic: RuntimeTargetChannelSemantic::CountryBorderGradient,
        dimensions: "B8G8R8A8_UNORM_SRGB 2816x2050, two vertical pages",
        source_detail: "CPU generated packed logical layer bank; page 0/page 1 use active anchors from gradient_border_map_mode_anchors.tsv, not fixed country/province/state SDF semantics",
        source_trace: "reverse_out/exports/gradient_border_channels.tsv; reverse_out/exports/gradient_border_map_mode_anchors.tsv; tools/vanilla_trace/runtime_targets.json",
        parity_status: "vanilla-backed target shape and page-anchor mapping; project-generated pixels until exact CGradientBorder layer painter is mirrored",
        provenance: BindingProvenance::ProjectGenerated,
        producer_status: "project_generated_sdf_non_parity",
        producer_equivalent: false,
        blocking_level: BindingBlockingLevel::Degraded,
    },
    RuntimeTargetSpec {
        name: "GradientBorderChannel2",
        format: wgpu::TextureFormat::Bgra8UnormSrgb,
        bytes_per_pixel: 4,
        semantic: RuntimeTargetChannelSemantic::ProvinceBorderGradient,
        dimensions: "B8G8R8A8_UNORM_SRGB 2816x2050, two vertical pages",
        source_detail: "CPU generated packed logical layer bank using the second composed RGBA stream; page 0/page 1 use active anchors from gradient_border_map_mode_anchors.tsv",
        source_trace: "reverse_out/exports/gradient_border_channels.tsv; reverse_out/exports/gradient_border_map_mode_anchors.tsv; tools/vanilla_trace/runtime_targets.json",
        parity_status: "vanilla-backed target shape and page-anchor mapping; project-generated pixels until exact CGradientBorder layer painter is mirrored",
        provenance: BindingProvenance::ProjectGenerated,
        producer_status: "project_generated_sdf_non_parity",
        producer_equivalent: false,
        blocking_level: BindingBlockingLevel::Degraded,
    },
    RuntimeTargetSpec {
        name: "GradientBorderChannel3",
        format: wgpu::TextureFormat::Bgra8UnormSrgb,
        bytes_per_pixel: 4,
        semantic: RuntimeTargetChannelSemantic::StateSeaImpassableGradient,
        dimensions: "B8G8R8A8_UNORM_SRGB 2x1 neutral fallback",
        source_detail: "Created as the representative-frame neutral fallback SRV; no full-size vanilla producer is assumed",
        source_trace: "reverse_out/exports/gradient_border_channels.tsv; tools/vanilla_trace/runtime_targets.json view5170",
        parity_status: "explicit neutral fallback for water input only; full-size Ch3 remains a non-vanilla project extension",
        provenance: BindingProvenance::NeutralFallback,
        producer_status: "neutral_fallback_non_parity",
        producer_equivalent: false,
        blocking_level: BindingBlockingLevel::Degraded,
    },
    RuntimeTargetSpec {
        name: "ProvinceSecondaryColorMap",
        format: wgpu::TextureFormat::Bgra8UnormSrgb,
        bytes_per_pixel: 4,
        semantic: RuntimeTargetChannelSemantic::ProvinceSecondaryColor,
        dimensions: "B8G8R8A8_UNORM_SRGB 2816x1024; supports full rebuild and 256x256 dirty-rect uploads",
        source_detail: "CPU generated packed per-province secondary target; RGB carries secondary overlay color and alpha gates CalculateOccupationMask diagonal stripe strength",
        source_trace: "reverse_out/exports/province_secondary_semantics.tsv; reverse_out/14_province_secondary_producer.md; tools/vanilla_trace/runtime_targets.json",
        parity_status: "vanilla-backed dimensions and RGBA semantics; project-generated gameplay overlay values",
        provenance: BindingProvenance::ProjectGenerated,
        producer_status: "project_generated_overlay_non_parity",
        producer_equivalent: false,
        blocking_level: BindingBlockingLevel::Degraded,
    },
    RuntimeTargetSpec {
        name: "IntelMap",
        format: wgpu::TextureFormat::R8Unorm,
        bytes_per_pixel: 1,
        semantic: RuntimeTargetChannelSemantic::IntelMap,
        dimensions: "A8 938x341, MAP_SIZE/6",
        source_detail: "CPU generated all-visible IntelMap placeholder at vanilla MAP_SIZE/6 dimensions",
        source_trace: "reverse_out/15_intel_map_producer.md; reverse_out/exports/mud_snow_light_fow_targets.tsv",
        parity_status: "explicit fallback: all-visible A8 placeholder until runtime intel producer is mirrored",
        provenance: BindingProvenance::NeutralFallback,
        producer_status: "all_visible_placeholder_non_parity",
        producer_equivalent: false,
        blocking_level: BindingBlockingLevel::Degraded,
    },
    RuntimeTargetSpec {
        name: "ShadowMap",
        format: wgpu::TextureFormat::Bgra8Unorm,
        bytes_per_pixel: 4,
        semantic: RuntimeTargetChannelSemantic::ProjectedShadowFow,
        dimensions: "B8G8R8A8_UNORM 2560x1600, full-res projected target followed by two blur passes",
        source_detail: "Projected packed shadow/FOW target; current producer creates a neutral full-res target and reserves the order 77-80 tree/terrain/blur chain",
        source_trace: "reverse_out/exports/shadow_fow_projected_producer.tsv; reverse_out/16_shadow_fow_projected_producer.md",
        parity_status: "explicit fallback producer: ordinary depth shadow is not used as vanilla ShadowMap",
        provenance: BindingProvenance::NeutralFallback,
        producer_status: PROJECTED_SHADOW_FOW_PRODUCER_STATUS,
        producer_equivalent: false,
        blocking_level: BindingBlockingLevel::Degraded,
    },
    RuntimeTargetSpec {
        name: "FOW",
        format: wgpu::TextureFormat::Rgba8Unorm,
        bytes_per_pixel: 4,
        semantic: RuntimeTargetChannelSemantic::FogOfWar,
        dimensions: "province-map pixels: world.map.province_map.width x height",
        source_detail: "CPU generated default visibility map with all provinces explored and visible",
        source_trace: "tools/vanilla_trace/runtime_targets.json render target/SRV census plus shader_bindings.json FOW/FOWNoise/FOWHeight family and project FOW binding",
        parity_status: "fallback: visible-all placeholder, not vanilla fog-of-war equivalent",
        provenance: BindingProvenance::NeutralFallback,
        producer_status: "visible_all_placeholder_non_parity",
        producer_equivalent: false,
        blocking_level: BindingBlockingLevel::Degraded,
    },
    RuntimeTargetSpec {
        name: "MudSnow",
        format: wgpu::TextureFormat::Bgra8Unorm,
        bytes_per_pixel: 4,
        semantic: RuntimeTargetChannelSemantic::MudSnow,
        dimensions: "B8G8R8A8_UNORM quarter-size SnowMudData: map_width/4 x map_height/4 (vanilla 1408x512 for 5632x2048)",
        source_detail: "Explicit zero fallback SnowMudData at vanilla quarter-size target dimensions; vanilla producer is CUpdateSnowMudThreaded/sub_141213280 and clears without game-state source",
        source_trace: "reverse_out/07_dynamic_map_targets.md; reverse_out/exports/mud_snow_light_fow_targets.tsv; tools/vanilla_trace/runtime_targets.json render target/SRV census plus shader_bindings.json and render_passes.json SnowMudData/SnowMudTexture resource names",
        parity_status: "fallback/non-parity: zero SnowMudData because vanilla game-state producer is not implemented",
        provenance: BindingProvenance::NeutralFallback,
        producer_status: "SnowMudData=missing_game_state_zero_fallback_non_parity",
        producer_equivalent: false,
        blocking_level: BindingBlockingLevel::Degraded,
    },
];

pub const PHASE5_RUNTIME_TARGET_SPECS: [RuntimeTargetSpec; 2] = [
    RuntimeTargetSpec {
        name: "LightDataMap",
        format: wgpu::TextureFormat::Rgba32Float,
        bytes_per_pixel: 16,
        semantic: RuntimeTargetChannelSemantic::PointLightData,
        dimensions: "384x1 texels: MAX_POINT_LIGHTS(192) * 2 RGBA32F texels per light",
        source_detail: "CPU generated point-light records from victory points, ports, airbases, buildings, and combat state; regenerated when building/combat light sources change",
        source_trace: "tools/vanilla_trace/runtime_targets.json render target/SRV census plus shader_bindings.json and render_passes.json resource name LightDataMap",
        parity_status: "trace-backed binding and project-generated light data; not copied from vanilla runtime memory",
        provenance: BindingProvenance::ProjectGenerated,
        producer_status: "project_generated_light_data",
        producer_equivalent: false,
        blocking_level: BindingBlockingLevel::Degraded,
    },
    RuntimeTargetSpec {
        name: "LightIndexMap",
        format: wgpu::TextureFormat::Rgba8Unorm,
        bytes_per_pixel: 4,
        semantic: RuntimeTargetChannelSemantic::PointLightIndex,
        dimensions: "tile grid: ceil(province_map / 16px), width aligned to 64 texels; RGBA packs up to 4 light ids with 255 sentinel",
        source_detail: "CPU generated point-light tile index sharing LightDataMap light ids; regenerated with LightDataMap when light sources change",
        source_trace: "tools/vanilla_trace/runtime_targets.json render target/SRV census plus shader_bindings.json and render_passes.json resource name LightIndexMap",
        parity_status: "trace-backed binding and project-derived 16px tile rule; not copied from vanilla runtime memory",
        provenance: BindingProvenance::ProjectGenerated,
        producer_status: "project_generated_light_index",
        producer_equivalent: false,
        blocking_level: BindingBlockingLevel::Degraded,
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeTargetPassBinding {
    pub binding: &'static str,
    pub target_name: &'static str,
    pub visual_impact: &'static str,
}

const TERRAIN_RUNTIME_TARGET_BINDINGS: [RuntimeTargetPassBinding; 9] = [
    RuntimeTargetPassBinding {
        binding: "light_data",
        target_name: "LightDataMap",
        visual_impact: "night lighting and local highlights use generated point light data",
    },
    RuntimeTargetPassBinding {
        binding: "light_index",
        target_name: "LightIndexMap",
        visual_impact: "point light lookup uses a generated tile index",
    },
    RuntimeTargetPassBinding {
        binding: "province_secondary_color",
        target_name: "ProvinceSecondaryColorMap",
        visual_impact:
            "occupation, selection, hover, and map-mode secondary tint use a runtime map target",
    },
    RuntimeTargetPassBinding {
        binding: "gradient_border_ch1",
        target_name: "GradientBorderChannel1",
        visual_impact: "country border gradient is generated from runtime ownership state",
    },
    RuntimeTargetPassBinding {
        binding: "gradient_border_ch2",
        target_name: "GradientBorderChannel2",
        visual_impact: "province border gradient is generated from runtime province topology",
    },
    RuntimeTargetPassBinding {
        binding: "gradient_border_ch3",
        target_name: "GradientBorderChannel3",
        visual_impact:
            "neutral 2x1 Ch3 fallback is bound; terrain use is not scored as vanilla parity",
    },
    RuntimeTargetPassBinding {
        binding: "ShadowMap",
        target_name: "ShadowMap",
        visual_impact:
            "terrain receives the projected shadow/FOW target instead of ordinary depth shadow",
    },
    RuntimeTargetPassBinding {
        binding: "fow",
        target_name: "FOW",
        visual_impact: "fog-of-war visibility is supplied as a runtime map target",
    },
    RuntimeTargetPassBinding {
        binding: "mud_snow",
        target_name: "MudSnow",
        visual_impact: "mud and snow masks are supplied as a runtime map target",
    },
];

const WATER_RUNTIME_TARGET_BINDINGS: [RuntimeTargetPassBinding; 9] = [
    RuntimeTargetPassBinding {
        binding: "gradient_border_ch1",
        target_name: "GradientBorderChannel1",
        visual_impact: "country border gradient is shared with terrain",
    },
    RuntimeTargetPassBinding {
        binding: "gradient_border_ch2",
        target_name: "GradientBorderChannel2",
        visual_impact: "province border gradient is shared with terrain",
    },
    RuntimeTargetPassBinding {
        binding: "gradient_border_ch3",
        target_name: "GradientBorderChannel3",
        visual_impact: "water receives the representative-frame 2x1 neutral Ch3 fallback",
    },
    RuntimeTargetPassBinding {
        binding: "ShadowMap",
        target_name: "ShadowMap",
        visual_impact: "water receives the projected shadow/FOW target metadata",
    },
    RuntimeTargetPassBinding {
        binding: "province_secondary_color",
        target_name: "ProvinceSecondaryColorMap",
        visual_impact: "water material receives selection and map-mode secondary tint",
    },
    RuntimeTargetPassBinding {
        binding: "fow",
        target_name: "FOW",
        visual_impact: "water material receives runtime fog-of-war visibility",
    },
    RuntimeTargetPassBinding {
        binding: "mud_snow",
        target_name: "MudSnow",
        visual_impact:
            "water has the shared snow/mud target available for SnowMudTexture parity work",
    },
    RuntimeTargetPassBinding {
        binding: "light_data",
        target_name: "LightDataMap",
        visual_impact: "water receives generated local night highlights",
    },
    RuntimeTargetPassBinding {
        binding: "light_index",
        target_name: "LightIndexMap",
        visual_impact: "water point light lookup uses the shared tile index",
    },
];

const RIVER_RUNTIME_TARGET_BINDINGS: [RuntimeTargetPassBinding; 8] = [
    RuntimeTargetPassBinding {
        binding: "province_secondary_color",
        target_name: "ProvinceSecondaryColorMap",
        visual_impact: "river material receives secondary overlay color and occupation stripe gate",
    },
    RuntimeTargetPassBinding {
        binding: "mud_snow",
        target_name: "MudSnow",
        visual_impact: "river material receives SnowMudTexture weather/season data",
    },
    RuntimeTargetPassBinding {
        binding: "light_data",
        target_name: "LightDataMap",
        visual_impact: "river receives generated local night highlights",
    },
    RuntimeTargetPassBinding {
        binding: "light_index",
        target_name: "LightIndexMap",
        visual_impact: "river point light lookup uses the shared tile index",
    },
    RuntimeTargetPassBinding {
        binding: "gradient_border_ch1",
        target_name: "GradientBorderChannel1",
        visual_impact: "river receives packed gradient-border channel 1",
    },
    RuntimeTargetPassBinding {
        binding: "gradient_border_ch2",
        target_name: "GradientBorderChannel2",
        visual_impact: "river receives packed gradient-border channel 2",
    },
    RuntimeTargetPassBinding {
        binding: "gradient_border_ch3",
        target_name: "GradientBorderChannel3",
        visual_impact: "river binds neutral Ch3 only as explicit fallback metadata",
    },
    RuntimeTargetPassBinding {
        binding: "ShadowMap",
        target_name: "ShadowMap",
        visual_impact:
            "river receives the projected shadow/FOW target instead of ordinary depth shadow",
    },
];

const PROJECTED_SHADOW_FOW_RUNTIME_TARGET_BINDINGS: [RuntimeTargetPassBinding; 4] = [
    RuntimeTargetPassBinding {
        binding: "IntelMap",
        target_name: "IntelMap",
        visual_impact: "projected shadow/FOW producer consumes vanilla MAP_SIZE/6 intel coverage",
    },
    RuntimeTargetPassBinding {
        binding: "ShadowMap",
        target_name: "ShadowMap",
        visual_impact: "producer writes the full-resolution packed projected shadow/FOW target",
    },
    RuntimeTargetPassBinding {
        binding: "fow_rgb_waterspec_a",
        target_name: "FOW",
        visual_impact: "producer has FOW family input available for projected target parity",
    },
    RuntimeTargetPassBinding {
        binding: "SnowMudData",
        target_name: "MudSnow",
        visual_impact: "producer has SnowMudData input available for projected target parity",
    },
];

const TREE_RUNTIME_TARGET_BINDINGS: [RuntimeTargetPassBinding; 9] = [
    RuntimeTargetPassBinding {
        binding: "gradient_border_ch1",
        target_name: "GradientBorderChannel1",
        visual_impact: "tree material shares country border gradient with terrain",
    },
    RuntimeTargetPassBinding {
        binding: "gradient_border_ch2",
        target_name: "GradientBorderChannel2",
        visual_impact: "tree material shares province border gradient with terrain",
    },
    RuntimeTargetPassBinding {
        binding: "gradient_border_ch3",
        target_name: "GradientBorderChannel3",
        visual_impact: "full-size Ch3 tree use is a non-vanilla extension; default parity binds neutral fallback only",
    },
    RuntimeTargetPassBinding {
        binding: "ShadowMap",
        target_name: "ShadowMap",
        visual_impact: "tree participates in the projected shadow/FOW producer/consumer chain",
    },
    RuntimeTargetPassBinding {
        binding: "province_secondary_color",
        target_name: "ProvinceSecondaryColorMap",
        visual_impact: "tree material receives selection and map-mode secondary tint",
    },
    RuntimeTargetPassBinding {
        binding: "fow",
        target_name: "FOW",
        visual_impact: "tree material receives runtime fog-of-war visibility",
    },
    RuntimeTargetPassBinding {
        binding: "mud_snow",
        target_name: "MudSnow",
        visual_impact: "tree material receives runtime snow masks",
    },
    RuntimeTargetPassBinding {
        binding: "light_data",
        target_name: "LightDataMap",
        visual_impact: "tree material receives generated local night highlights",
    },
    RuntimeTargetPassBinding {
        binding: "light_index",
        target_name: "LightIndexMap",
        visual_impact: "tree point light lookup uses the shared tile index",
    },
];

const PDXMESH_RUNTIME_TARGET_BINDINGS: [RuntimeTargetPassBinding; 9] = [
    RuntimeTargetPassBinding {
        binding: "gradient_border_ch1",
        target_name: "GradientBorderChannel1",
        visual_impact: "mesh material shares country border gradient with terrain",
    },
    RuntimeTargetPassBinding {
        binding: "gradient_border_ch2",
        target_name: "GradientBorderChannel2",
        visual_impact: "mesh material shares province border gradient with terrain",
    },
    RuntimeTargetPassBinding {
        binding: "gradient_border_ch3",
        target_name: "GradientBorderChannel3",
        visual_impact: "full-size Ch3 pdxmesh use is a non-vanilla extension; default parity binds neutral fallback only",
    },
    RuntimeTargetPassBinding {
        binding: "ShadowMap",
        target_name: "ShadowMap",
        visual_impact: "mesh material receives projected shadow/FOW target metadata",
    },
    RuntimeTargetPassBinding {
        binding: "province_secondary_color",
        target_name: "ProvinceSecondaryColorMap",
        visual_impact: "mesh material receives battle-plan, selection, and map-mode secondary tint",
    },
    RuntimeTargetPassBinding {
        binding: "fow",
        target_name: "FOW",
        visual_impact: "mesh material receives runtime fog-of-war visibility",
    },
    RuntimeTargetPassBinding {
        binding: "mud_snow",
        target_name: "MudSnow",
        visual_impact: "mesh material receives shared snow masks",
    },
    RuntimeTargetPassBinding {
        binding: "light_data",
        target_name: "LightDataMap",
        visual_impact: "mesh material receives generated local night highlights",
    },
    RuntimeTargetPassBinding {
        binding: "light_index",
        target_name: "LightIndexMap",
        visual_impact: "mesh point light lookup uses the shared tile index",
    },
];

pub fn runtime_target_spec(name: &str) -> Option<&'static RuntimeTargetSpec> {
    PHASE4_RUNTIME_TARGET_SPECS
        .iter()
        .chain(PHASE5_RUNTIME_TARGET_SPECS.iter())
        .find(|spec| spec.name == name)
}

pub fn runtime_target_bindings_for_pass(pass: &'static str) -> &'static [RuntimeTargetPassBinding] {
    match pass {
        "terrain" => &TERRAIN_RUNTIME_TARGET_BINDINGS,
        "water" => &WATER_RUNTIME_TARGET_BINDINGS,
        "river" => &RIVER_RUNTIME_TARGET_BINDINGS,
        "projected_fow_shadow" => &PROJECTED_SHADOW_FOW_RUNTIME_TARGET_BINDINGS,
        "tree" => &TREE_RUNTIME_TARGET_BINDINGS,
        "pdxmesh" | "object" => &PDXMESH_RUNTIME_TARGET_BINDINGS,
        _ => &[],
    }
}

pub fn runtime_target_audit_entries_for_pass(pass: &'static str) -> Vec<BindingAuditEntry> {
    let mut entries: Vec<_> = runtime_target_bindings_for_pass(pass)
        .iter()
        .filter_map(|binding| runtime_target_audit_entry(pass, binding))
        .collect();
    if pass == "projected_fow_shadow" {
        entries.extend(projected_shadow_fow_stage_audit_entries());
    }
    entries
}

fn runtime_target_source_detail(spec: &RuntimeTargetSpec) -> String {
    match spec.name {
        "GradientBorderChannel1" | "GradientBorderChannel2" => {
            let plan = gradient_border::active_producer_plan(0);
            let page0 = plan
                .page0_layer
                .map(|layer| format!("{}:{}", layer.id, layer.name))
                .unwrap_or_else(|| "disabled".to_string());
            let page1 = plan
                .page1_layer
                .map(|layer| format!("{}:{}", layer.id, layer.name))
                .unwrap_or_else(|| "disabled".to_string());
            format!(
                "{}; audit default map_mode=0 active page0={} page1={}; producer_reason={}",
                spec.source_detail, page0, page1, plan.producer_reason
            )
        }
        "ProvinceSecondaryColorMap" => {
            let (_, _, width, height) = province_secondary::full_rect();
            format!(
                "{}; producer_path full_rebuild={}x{}, dirty_rect={}x{}; producer_reason={}",
                spec.source_detail,
                width,
                height,
                province_secondary::VANILLA_PROVINCE_SECONDARY_DIRTY_RECT,
                province_secondary::VANILLA_PROVINCE_SECONDARY_DIRTY_RECT,
                province_secondary::PROVINCE_SECONDARY_FULL_REBUILD_REASON
            )
        }
        _ => spec.source_detail.to_string(),
    }
}

fn runtime_target_audit_entry(
    pass: &'static str,
    binding: &RuntimeTargetPassBinding,
) -> Option<BindingAuditEntry> {
    let spec = runtime_target_spec(binding.target_name)?;
    Some(
        BindingAuditEntry::dynamic_target(pass, binding.binding, spec.name, binding.visual_impact)
            .with_runtime_target_metadata(
                runtime_target_source_detail(spec),
                format!("{:?}; {} bytes/pixel", spec.format, spec.bytes_per_pixel),
                spec.dimensions,
                spec.source_trace,
                spec.parity_status,
            )
            .with_producer_status(
                spec.provenance,
                spec.producer_status,
                spec.producer_equivalent,
                spec.blocking_level,
            ),
    )
}

fn projected_shadow_fow_stage_audit_entries() -> impl Iterator<Item = BindingAuditEntry> {
    PROJECTED_SHADOW_FOW_PRODUCER_STAGES
        .iter()
        .map(|stage| {
            BindingAuditEntry::dynamic_target(
                "projected_fow_shadow",
                stage.stage,
                stage.target,
                "projected shadow/FOW producer stage is reserved in frame graph order",
            )
            .with_runtime_target_metadata(
                format!(
                    "order {} {} -> {}; implementation={}",
                    stage.order,
                    stage.source,
                    stage.target,
                    if stage.implemented {
                        "implemented"
                    } else {
                        "fallback_neutral"
                    }
                ),
                "Bgra8Unorm; 4 bytes/pixel",
                "B8G8R8A8_UNORM 2560x1600",
                "reverse_out/16_shadow_fow_projected_producer.md; reverse_out/exports/shadow_fow_projected_producer.tsv",
                "explicit fallback producer stage; target lifecycle is reserved but vanilla shader is not mirrored",
            )
            .with_producer_status(
                BindingProvenance::NeutralFallback,
                PROJECTED_SHADOW_FOW_PRODUCER_STATUS,
                false,
                BindingBlockingLevel::Degraded,
            )
        })
}

pub struct RuntimeTargetTexture {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    width: u32,
    height: u32,
    bytes_per_pixel: u32,
}

impl RuntimeTargetTexture {
    fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        label: &'static str,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
        bytes_per_pixel: u32,
        data: &[u8],
    ) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        let target = Self {
            texture,
            view,
            width,
            height,
            bytes_per_pixel,
        };
        target.write(queue, data);
        target
    }

    pub fn new_r8(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        label: &'static str,
        width: u32,
        height: u32,
        data: &[u8],
    ) -> Self {
        Self::new(
            device,
            queue,
            label,
            width,
            height,
            wgpu::TextureFormat::R8Unorm,
            1,
            data,
        )
    }

    pub fn new_rgba8(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        label: &'static str,
        width: u32,
        height: u32,
        data: &[u8],
    ) -> Self {
        Self::new(
            device,
            queue,
            label,
            width,
            height,
            wgpu::TextureFormat::Rgba8Unorm,
            4,
            data,
        )
    }

    pub fn new_bgra8_srgb(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        label: &'static str,
        width: u32,
        height: u32,
        data: &[u8],
    ) -> Self {
        Self::new(
            device,
            queue,
            label,
            width,
            height,
            wgpu::TextureFormat::Bgra8UnormSrgb,
            4,
            data,
        )
    }

    pub fn new_bgra8(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        label: &'static str,
        width: u32,
        height: u32,
        data: &[u8],
    ) -> Self {
        Self::new(
            device,
            queue,
            label,
            width,
            height,
            wgpu::TextureFormat::Bgra8Unorm,
            4,
            data,
        )
    }

    pub fn new_rgba32_float(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        label: &'static str,
        width: u32,
        height: u32,
        data: &[u8],
    ) -> Self {
        Self::new(
            device,
            queue,
            label,
            width,
            height,
            wgpu::TextureFormat::Rgba32Float,
            16,
            data,
        )
    }

    pub fn write(&self, queue: &wgpu::Queue, data: &[u8]) {
        let expected = self.width as usize * self.height as usize * self.bytes_per_pixel as usize;
        debug_assert_eq!(data.len(), expected);
        if data.len() != expected || self.width == 0 || self.height == 0 {
            return;
        }
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(self.width * self.bytes_per_pixel),
                rows_per_image: Some(self.height),
            },
            wgpu::Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
        );
    }

    pub fn write_rect(
        &self,
        queue: &wgpu::Queue,
        data: &[u8],
        x: u32,
        y: u32,
        width: u32,
        height: u32,
    ) {
        if x >= self.width || y >= self.height || width == 0 || height == 0 {
            return;
        }
        let width = width.min(self.width - x);
        let height = height.min(self.height - y);
        let expected = width as usize * height as usize * self.bytes_per_pixel as usize;
        debug_assert_eq!(data.len(), expected);
        if data.len() != expected {
            return;
        }
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d { x, y, z: 0 },
                aspect: wgpu::TextureAspect::All,
            },
            data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width * self.bytes_per_pixel),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
    }

    pub fn memory_bytes(&self) -> u64 {
        self.width as u64 * self.height as u64 * self.bytes_per_pixel as u64
    }
}

pub struct GradientBorderRuntimeTargets {
    pub ch1: RuntimeTargetTexture,
    pub ch2: RuntimeTargetTexture,
    pub ch3: RuntimeTargetTexture,
}

pub struct VanillaRuntimeTargets {
    pub gradient_border: GradientBorderRuntimeTargets,
    pub province_secondary_color: RuntimeTargetTexture,
    pub fow: RuntimeTargetTexture,
    pub mud_snow: RuntimeTargetTexture,
    pub intel_map: RuntimeTargetTexture,
    pub projected_shadow_fow: RuntimeTargetTexture,
    pub projected_shadow_fow_blur_temp: RuntimeTargetTexture,
    pub light_data: RuntimeTargetTexture,
    pub light_index: RuntimeTargetTexture,
    country_border_signature: u64,
    province_secondary_signature: u64,
    mud_snow_signature: u64,
    point_light_signature: u64,
    province_centroids: Vec<(f32, f32)>,
    province_secondary_data: Vec<u8>,
    mud_snow_data: Vec<u8>,
    point_light_data: point_lights::PointLightTargetData,
    world_scale: f32,
    height_scale: f32,
}

pub struct VanillaRuntimeTargetViews<'a> {
    pub gradient_border_ch1: &'a wgpu::TextureView,
    pub gradient_border_ch2: &'a wgpu::TextureView,
    pub gradient_border_ch3: &'a wgpu::TextureView,
    pub province_secondary_color: &'a wgpu::TextureView,
    pub fow: &'a wgpu::TextureView,
    pub mud_snow: &'a wgpu::TextureView,
    pub intel_map: &'a wgpu::TextureView,
    pub projected_shadow_fow: &'a wgpu::TextureView,
    pub light_data: &'a wgpu::TextureView,
    pub light_index: &'a wgpu::TextureView,
}

#[derive(Clone, Copy)]
pub struct VanillaRuntimeTargetInputs<'a> {
    pub world: &'a World,
    pub country_sdf: &'a [u8],
    pub province_sdf: &'a [u8],
    pub coast_sdf: &'a [u8],
    pub world_scale: f32,
    pub height_scale: f32,
    pub default_map_mode_code: u8,
}

#[derive(Debug, Clone, Copy)]
pub struct VanillaRuntimeTargetFrameParams {
    pub selected_province_id: u32,
    pub hovered_province_id: u32,
    pub map_mode: MapMode,
    pub player_country: Option<CountryId>,
    pub battle_plan_opacity: f32,
    pub naval_dominance_opacity: f32,
    pub occupation_opacity: f32,
    pub selected_opacity: f32,
    pub hover_opacity: f32,
    pub map_mode_overlay_opacity: f32,
    pub season_snow_offset: f32,
    pub season_blend: f32,
}

impl Default for VanillaRuntimeTargetFrameParams {
    fn default() -> Self {
        Self {
            selected_province_id: u32::MAX,
            hovered_province_id: u32::MAX,
            map_mode: MapMode::Political,
            player_country: None,
            battle_plan_opacity: 1.0,
            naval_dominance_opacity: 0.0,
            occupation_opacity: 0.0,
            selected_opacity: 1.0,
            hover_opacity: 1.0,
            map_mode_overlay_opacity: 0.0,
            season_snow_offset: 0.0,
            season_blend: 0.0,
        }
    }
}

impl VanillaRuntimeTargets {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        inputs: VanillaRuntimeTargetInputs<'_>,
    ) -> Self {
        let width = inputs.world.map.province_map.width;
        let height = inputs.world.map.province_map.height;
        let gradient_cpu = gradient_border::generate(inputs);
        let country_border_signature =
            gradient_border::producer_signature(inputs.world, inputs.default_map_mode_code);
        let frame_params = VanillaRuntimeTargetFrameParams::default();
        let province_secondary_data = province_secondary::generate(inputs.world, &frame_params);
        let province_secondary_signature =
            province_secondary::signature(inputs.world, &frame_params);
        let mud_snow_data = fow::generate_neutral_snow_mud(width, height);
        let mud_snow_signature = fow::mud_snow_signature(&frame_params);
        let fow_data = fow::generate_default_fow(width, height);
        let province_centroids = compute_province_centroids(&inputs.world.map.province_map);
        let point_light_data = point_lights::generate_with_centroids(inputs, &province_centroids);
        let point_light_signature = point_lights::signature(inputs.world);

        Self {
            gradient_border: GradientBorderRuntimeTargets {
                ch1: RuntimeTargetTexture::new_bgra8_srgb(
                    device,
                    queue,
                    "GradientBorderChannel1",
                    gradient_border::VANILLA_GRADIENT_BORDER_WIDTH,
                    gradient_border::VANILLA_GRADIENT_BORDER_HEIGHT,
                    &gradient_cpu.ch1,
                ),
                ch2: RuntimeTargetTexture::new_bgra8_srgb(
                    device,
                    queue,
                    "GradientBorderChannel2",
                    gradient_border::VANILLA_GRADIENT_BORDER_WIDTH,
                    gradient_border::VANILLA_GRADIENT_BORDER_HEIGHT,
                    &gradient_cpu.ch2,
                ),
                ch3: RuntimeTargetTexture::new_bgra8_srgb(
                    device,
                    queue,
                    "GradientBorderChannel3",
                    gradient_border::VANILLA_GRADIENT_BORDER_CH3_WIDTH,
                    gradient_border::VANILLA_GRADIENT_BORDER_CH3_HEIGHT,
                    &gradient_cpu.ch3,
                ),
            },
            province_secondary_color: RuntimeTargetTexture::new_bgra8_srgb(
                device,
                queue,
                "ProvinceSecondaryColorMap",
                province_secondary::VANILLA_PROVINCE_SECONDARY_WIDTH,
                province_secondary::VANILLA_PROVINCE_SECONDARY_HEIGHT,
                &province_secondary_data,
            ),
            fow: RuntimeTargetTexture::new_rgba8(device, queue, "FOW", width, height, &fow_data),
            mud_snow: RuntimeTargetTexture::new_bgra8(
                device,
                queue,
                "MudSnow",
                fow::snow_mud_dimensions(width, height).0,
                fow::snow_mud_dimensions(width, height).1,
                &mud_snow_data,
            ),
            intel_map: RuntimeTargetTexture::new_r8(
                device,
                queue,
                "IntelMap",
                fow::VANILLA_INTEL_MAP_WIDTH,
                fow::VANILLA_INTEL_MAP_HEIGHT,
                &fow::generate_default_intel_map(),
            ),
            projected_shadow_fow: RuntimeTargetTexture::new_bgra8(
                device,
                queue,
                "ShadowMap",
                fow::VANILLA_PROJECTED_SHADOW_WIDTH,
                fow::VANILLA_PROJECTED_SHADOW_HEIGHT,
                &fow::generate_neutral_projected_shadow_fow(),
            ),
            projected_shadow_fow_blur_temp: RuntimeTargetTexture::new_bgra8(
                device,
                queue,
                "ShadowMap_ProjectFOW_BlurTemp",
                fow::VANILLA_PROJECTED_SHADOW_WIDTH,
                fow::VANILLA_PROJECTED_SHADOW_HEIGHT,
                &fow::generate_neutral_projected_shadow_fow(),
            ),
            light_data: RuntimeTargetTexture::new_rgba32_float(
                device,
                queue,
                "LightDataMap",
                point_light_data.light_data_width,
                1,
                &point_light_data.light_data,
            ),
            light_index: RuntimeTargetTexture::new_rgba8(
                device,
                queue,
                "LightIndexMap",
                point_light_data.light_index_width,
                point_light_data.light_index_height,
                &point_light_data.light_index,
            ),
            country_border_signature,
            province_secondary_signature,
            mud_snow_signature,
            point_light_signature,
            province_centroids,
            province_secondary_data,
            mud_snow_data,
            point_light_data,
            world_scale: inputs.world_scale,
            height_scale: inputs.height_scale,
        }
    }

    pub fn views(&self) -> VanillaRuntimeTargetViews<'_> {
        VanillaRuntimeTargetViews {
            gradient_border_ch1: &self.gradient_border.ch1.view,
            gradient_border_ch2: &self.gradient_border.ch2.view,
            gradient_border_ch3: &self.gradient_border.ch3.view,
            province_secondary_color: &self.province_secondary_color.view,
            fow: &self.fow.view,
            mud_snow: &self.mud_snow.view,
            intel_map: &self.intel_map.view,
            projected_shadow_fow: &self.projected_shadow_fow.view,
            light_data: &self.light_data.view,
            light_index: &self.light_index.view,
        }
    }

    pub fn memory_bytes(&self) -> u64 {
        self.gradient_border.ch1.memory_bytes()
            + self.gradient_border.ch2.memory_bytes()
            + self.gradient_border.ch3.memory_bytes()
            + self.province_secondary_color.memory_bytes()
            + self.fow.memory_bytes()
            + self.mud_snow.memory_bytes()
            + self.intel_map.memory_bytes()
            + self.projected_shadow_fow.memory_bytes()
            + self.projected_shadow_fow_blur_temp.memory_bytes()
            + self.light_data.memory_bytes()
            + self.light_index.memory_bytes()
    }

    pub fn cache_entry_count(&self) -> usize {
        // Three gradient targets plus secondary, FOW, mud/snow, IntelMap,
        // projected ShadowMap, its full-resolution blur temp, light data, and
        // light index.
        11
    }

    pub fn projected_shadow_fow_producer_plan(&self) -> &'static ProjectedShadowFowProducerPlan {
        &PROJECTED_SHADOW_FOW_PRODUCER_PLAN
    }

    pub fn update_frame(
        &mut self,
        queue: &wgpu::Queue,
        world: &World,
        params: &VanillaRuntimeTargetFrameParams,
    ) {
        let profile_started = Instant::now();
        let country_sig_started = Instant::now();
        let map_mode_code = province_secondary::map_mode_code(params.map_mode);
        let country_border_signature = gradient_border::producer_signature(world, map_mode_code);
        let profile_country_sig_ms = country_sig_started.elapsed().as_secs_f32() * 1000.0;
        let mut profile_country_update_ms = 0.0;
        if country_border_signature != self.country_border_signature {
            let update_started = Instant::now();
            let gradient = gradient_border::generate_runtime_channels(world, map_mode_code);
            self.gradient_border.ch1.write(queue, &gradient.ch1);
            self.gradient_border.ch2.write(queue, &gradient.ch2);
            self.gradient_border.ch3.write(queue, &gradient.ch3);
            self.country_border_signature = country_border_signature;
            profile_country_update_ms = update_started.elapsed().as_secs_f32() * 1000.0;
        }

        let secondary_sig_started = Instant::now();
        let secondary_signature = province_secondary::signature(world, params);
        let profile_secondary_sig_ms = secondary_sig_started.elapsed().as_secs_f32() * 1000.0;
        let mut profile_secondary_update_ms = 0.0;
        if secondary_signature != self.province_secondary_signature {
            let update_started = Instant::now();
            self.province_secondary_data = province_secondary::generate(world, params);
            self.province_secondary_color
                .write(queue, &self.province_secondary_data);
            self.province_secondary_signature = secondary_signature;
            profile_secondary_update_ms = update_started.elapsed().as_secs_f32() * 1000.0;
        }

        let mud_sig_started = Instant::now();
        let mud_snow_signature = fow::mud_snow_signature(params);
        let profile_mud_sig_ms = mud_sig_started.elapsed().as_secs_f32() * 1000.0;
        let mut profile_mud_update_ms = 0.0;
        if mud_snow_signature != self.mud_snow_signature {
            let update_started = Instant::now();
            self.mud_snow_data = fow::generate_neutral_snow_mud(
                world.map.province_map.width,
                world.map.province_map.height,
            );
            self.mud_snow.write(queue, &self.mud_snow_data);
            self.mud_snow_signature = mud_snow_signature;
            profile_mud_update_ms = update_started.elapsed().as_secs_f32() * 1000.0;
        }

        let light_sig_started = Instant::now();
        let point_light_signature = point_lights::signature(world);
        let profile_light_sig_ms = light_sig_started.elapsed().as_secs_f32() * 1000.0;
        let mut profile_light_update_ms = 0.0;
        if point_light_signature != self.point_light_signature {
            let update_started = Instant::now();
            self.point_light_data = point_lights::generate_with_centroids(
                VanillaRuntimeTargetInputs {
                    world,
                    country_sdf: &[],
                    province_sdf: &[],
                    coast_sdf: &[],
                    world_scale: self.world_scale,
                    height_scale: self.height_scale,
                    default_map_mode_code: province_secondary::map_mode_code(params.map_mode),
                },
                &self.province_centroids,
            );
            self.light_data
                .write(queue, &self.point_light_data.light_data);
            self.light_index
                .write(queue, &self.point_light_data.light_index);
            self.point_light_signature = point_light_signature;
            profile_light_update_ms = update_started.elapsed().as_secs_f32() * 1000.0;
        }
        let profile_total_ms = profile_started.elapsed().as_secs_f32() * 1000.0;
        if profile_total_ms >= 25.0 {
            println!(
                "[vanilla-prof] total={:.2}ms country_sig={:.2}ms country_update={:.2}ms secondary_sig={:.2}ms secondary_update={:.2}ms mud_sig={:.2}ms mud_update={:.2}ms light_sig={:.2}ms light_update={:.2}ms",
                profile_total_ms,
                profile_country_sig_ms,
                profile_country_update_ms,
                profile_secondary_sig_ms,
                profile_secondary_update_ms,
                profile_mud_sig_ms,
                profile_mud_update_ms,
                profile_light_sig_ms,
                profile_light_update_ms,
            );
        }
    }

    pub fn update_province_secondary_dirty_rect(
        &mut self,
        queue: &wgpu::Queue,
        world: &World,
        params: &VanillaRuntimeTargetFrameParams,
        province_id: u16,
    ) -> bool {
        let Some((x, y, width, height)) =
            province_secondary::dirty_rect_for_province(world, province_id)
        else {
            return false;
        };
        if width == 0 || height == 0 {
            return false;
        }
        self.province_secondary_data = province_secondary::generate(world, params);
        let rect =
            province_secondary::copy_rect(&self.province_secondary_data, x, y, width, height);
        self.province_secondary_color
            .write_rect(queue, &rect, x, y, width, height);
        self.province_secondary_signature = province_secondary::signature(world, params);
        true
    }

    pub fn binding_audit_entries_for_pass(&self, pass: &'static str) -> Vec<BindingAuditEntry> {
        runtime_target_audit_entries_for_pass(pass)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn phase4_runtime_target_specs_cover_required_targets() {
        let names: HashSet<&'static str> = PHASE4_RUNTIME_TARGET_SPECS
            .iter()
            .map(|spec| spec.name)
            .collect();
        for required in [
            "GradientBorderChannel1",
            "GradientBorderChannel2",
            "GradientBorderChannel3",
            "ProvinceSecondaryColorMap",
            "IntelMap",
            "ShadowMap",
            "FOW",
            "MudSnow",
        ] {
            assert!(
                names.contains(required),
                "missing runtime target spec: {required}"
            );
        }
    }

    #[test]
    fn phase4_runtime_target_formats_match_shader_bindings() {
        let r8 = PHASE4_RUNTIME_TARGET_SPECS
            .iter()
            .filter(|spec| spec.format == wgpu::TextureFormat::R8Unorm)
            .count();
        let bgra8_srgb = PHASE4_RUNTIME_TARGET_SPECS
            .iter()
            .filter(|spec| spec.format == wgpu::TextureFormat::Bgra8UnormSrgb)
            .count();
        let bgra8 = PHASE4_RUNTIME_TARGET_SPECS
            .iter()
            .filter(|spec| spec.format == wgpu::TextureFormat::Bgra8Unorm)
            .count();
        let rgba8 = PHASE4_RUNTIME_TARGET_SPECS
            .iter()
            .filter(|spec| spec.format == wgpu::TextureFormat::Rgba8Unorm)
            .count();
        assert_eq!(r8, 1);
        assert_eq!(bgra8_srgb, 4);
        assert_eq!(bgra8, 2);
        assert_eq!(rgba8, 1);
        assert!(PHASE4_RUNTIME_TARGET_SPECS
            .iter()
            .all(|spec| spec.bytes_per_pixel == 1 || spec.bytes_per_pixel == 4));
    }

    #[test]
    fn phase5_runtime_target_specs_cover_point_lights() {
        let names: HashSet<&'static str> = PHASE5_RUNTIME_TARGET_SPECS
            .iter()
            .map(|spec| spec.name)
            .collect();
        assert!(names.contains("LightDataMap"));
        assert!(names.contains("LightIndexMap"));
        assert!(PHASE5_RUNTIME_TARGET_SPECS.iter().any(|spec| {
            spec.semantic == RuntimeTargetChannelSemantic::PointLightData
                && spec.format == wgpu::TextureFormat::Rgba32Float
                && spec.bytes_per_pixel == 16
        }));
        assert!(PHASE5_RUNTIME_TARGET_SPECS.iter().any(|spec| {
            spec.semantic == RuntimeTargetChannelSemantic::PointLightIndex
                && spec.format == wgpu::TextureFormat::Rgba8Unorm
                && spec.bytes_per_pixel == 4
        }));
    }

    #[test]
    fn runtime_target_specs_have_stable_cache_shape() {
        assert_eq!(
            PHASE4_RUNTIME_TARGET_SPECS.len() + PHASE5_RUNTIME_TARGET_SPECS.len(),
            10
        );
        let bytes_per_pixel: u32 = PHASE4_RUNTIME_TARGET_SPECS
            .iter()
            .chain(PHASE5_RUNTIME_TARGET_SPECS.iter())
            .map(|spec| spec.bytes_per_pixel)
            .sum();
        assert_eq!(bytes_per_pixel, 49);
    }

    #[test]
    fn runtime_target_specs_are_traceable_and_explicit_about_parity() {
        for spec in PHASE4_RUNTIME_TARGET_SPECS
            .iter()
            .chain(PHASE5_RUNTIME_TARGET_SPECS.iter())
        {
            assert!(
                spec.source_trace.contains("runtime_targets.json")
                    || spec.source_trace.contains("reverse_out/"),
                "{} missing trace provenance",
                spec.name
            );
            assert!(
                !spec.source_detail.is_empty()
                    && !spec.dimensions.is_empty()
                    && !spec.parity_status.is_empty(),
                "{} missing audit metadata",
                spec.name
            );
        }

        for fallback in ["FOW", "MudSnow"] {
            let spec = runtime_target_spec(fallback).unwrap();
            assert!(
                spec.parity_status.contains("fallback"),
                "{fallback} must explicitly declare fallback status"
            );
        }
    }

    #[test]
    fn shared_pass_bindings_cover_terrain_water_tree_and_objects() {
        for (pass, expected_len) in [
            ("terrain", 9),
            ("water", 9),
            ("river", 8),
            ("tree", 9),
            ("pdxmesh", 9),
        ] {
            let entries = runtime_target_audit_entries_for_pass(pass);
            assert_eq!(
                entries.len(),
                expected_len,
                "{pass} runtime target coverage changed"
            );
            for entry in entries {
                assert_eq!(
                    entry.source_name,
                    runtime_target_spec(&entry.source_name).unwrap().name
                );
                assert!(
                    entry.resource_format.is_some(),
                    "{pass}.{} missing format",
                    entry.binding
                );
                assert!(
                    entry.resource_dimensions.is_some(),
                    "{pass}.{} missing dimensions",
                    entry.binding
                );
                assert!(
                    entry.source_trace.as_deref().is_some_and(|trace| {
                        trace.contains("runtime_targets.json") || trace.contains("reverse_out/")
                    }),
                    "{pass}.{} missing trace provenance",
                    entry.binding
                );
            }
        }

        let producer_entries = runtime_target_audit_entries_for_pass("projected_fow_shadow");
        assert_eq!(producer_entries.len(), 8);
        assert!(producer_entries
            .iter()
            .any(|entry| entry.source_name == "IntelMap"));
        assert!(producer_entries
            .iter()
            .any(|entry| entry.source_name == "ShadowMap"));
        assert!(producer_entries.iter().any(|entry| {
            entry.binding == "shadowblur/fullres_horizontal"
                && entry.source_name == "ShadowMap_ProjectFOW_BlurTemp"
        }));
        assert!(producer_entries.iter().any(|entry| {
            entry.producer_status.as_deref() == Some(PROJECTED_SHADOW_FOW_PRODUCER_STATUS)
                && entry.source_detail.as_deref().is_some_and(|detail| {
                    detail.contains("order 77") || detail.contains("order 80")
                })
        }));
    }

    #[test]
    fn phase_f_audit_reports_gradient_and_secondary_producer_details() {
        let entries = runtime_target_audit_entries_for_pass("terrain");
        let detail_for = |name: &str| {
            entries
                .iter()
                .find(|entry| entry.source_name == name)
                .and_then(|entry| entry.source_detail.as_deref())
                .unwrap_or_else(|| panic!("missing audit detail for {name}"))
        };
        let gradient = detail_for("GradientBorderChannel1");
        assert!(gradient.contains("active page0=5:map_mode_primary_border"));
        assert!(gradient.contains("page1=0:country_controller_border"));
        assert!(gradient.contains("producer_reason="));

        let secondary = detail_for("ProvinceSecondaryColorMap");
        assert!(secondary.contains("full_rebuild=2816x1024"));
        assert!(secondary.contains("dirty_rect=256x256"));
        assert!(secondary.contains("alpha gates CalculateOccupationMask"));
    }

    #[test]
    fn projected_shadow_fow_producer_plan_reserves_orders_77_to_80() {
        let plan = PROJECTED_SHADOW_FOW_PRODUCER_PLAN;
        assert_eq!(plan.status, PROJECTED_SHADOW_FOW_PRODUCER_STATUS);
        assert_eq!(plan.output_target, "ShadowMap");
        assert_eq!(plan.temp_target, "ShadowMap_ProjectFOW_BlurTemp");
        assert_eq!(plan.stages.len(), 4);
        assert_eq!(
            plan.stages
                .iter()
                .map(|stage| stage.order)
                .collect::<Vec<_>>(),
            vec![77, 78, 79, 80]
        );
        assert!(plan.stages.iter().all(|stage| !stage.implemented));
        assert!(plan
            .stages
            .iter()
            .any(|stage| stage.stage == "terrainunlit/projected"));
        assert!(plan
            .stages
            .iter()
            .any(|stage| stage.stage == "shadowblur/fullres_vertical"));
    }
}
