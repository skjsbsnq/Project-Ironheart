#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapFramegraphPassStatus {
    ImplementedSubmitPoint,
    ImplementedDegradedSubmitPoint,
    ReservedZeroDrawBlocker,
    BoundedFamilyRegistered,
    PostprocessSubpassRegistered,
    OutsideHudScope,
}

impl MapFramegraphPassStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ImplementedSubmitPoint => "implemented_submit_point",
            Self::ImplementedDegradedSubmitPoint => "implemented_degraded_submit_point",
            Self::ReservedZeroDrawBlocker => "reserved_zero_draw_blocker",
            Self::BoundedFamilyRegistered => "bounded_family_registered",
            Self::PostprocessSubpassRegistered => "postprocess_subpass_registered",
            Self::OutsideHudScope => "outside_hud_scope",
        }
    }

    pub const fn is_zero_draw_blocker(self) -> bool {
        matches!(self, Self::ReservedZeroDrawBlocker)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MapFramegraphPassEntry {
    pub registry_pass: &'static str,
    pub vanilla_order: &'static str,
    pub order_min: u16,
    pub order_max: u16,
    pub layer: &'static str,
    pub classification: &'static str,
    pub shader_or_effect: &'static str,
    pub target: &'static str,
    pub reads: &'static [&'static str],
    pub writes: &'static [&'static str],
    pub state_inputs: &'static [&'static str],
    pub evidence: &'static str,
    pub status: MapFramegraphPassStatus,
    pub parity_note: &'static str,
}

impl MapFramegraphPassEntry {
    pub const fn is_main_hdr_order_77_88(self) -> bool {
        self.order_min >= 77 && self.order_max <= 88
    }

    pub const fn is_postprocess_order_134_139(self) -> bool {
        self.order_min >= 134 && self.order_max <= 139
    }

    pub const fn is_zero_draw_blocker(self) -> bool {
        self.status.is_zero_draw_blocker()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MapDebugCaptureEntry {
    pub family: &'static str,
    pub registry_pass: &'static str,
    pub capture_layer: &'static str,
    pub source_target: &'static str,
    pub output_pattern: &'static str,
    pub evidence: &'static str,
}

pub const MAP_FRAMEGRAPH_EVIDENCE: &[&str] = &[
    "C:\\Users\\19180\\Documents\\999\\text\\reverse_out\\18_map_pass_classification.md",
    "C:\\Users\\19180\\Documents\\999\\text\\reverse_out\\exports\\map_pass_table.tsv",
    "C:\\Users\\19180\\Documents\\999\\text\\reverse_out\\exports\\renderer_framegraph_spec.tsv",
    "C:\\Users\\19180\\Documents\\999\\text\\reverse_out\\exports\\postprocess_full_order.tsv",
];

pub const NON_HUD_MAP_PASS_REGISTRY: &[MapFramegraphPassEntry] = &[
    MapFramegraphPassEntry {
        registry_pass: "projected_fow_shadow",
        vanilla_order: "77..80",
        order_min: 77,
        order_max: 80,
        layer: "projected_shadow_fow",
        classification: "world_shadow_fow_producer",
        shader_or_effect: "tree/projected; terrainunlit/projected; shadowblur",
        target: "ShadowMap",
        reads: &["tree instances", "IntelMap", "FOW", "SnowMudData"],
        writes: &["ShadowMap", "ShadowMap_ProjectFOW_BlurTemp"],
        state_inputs: &["tree/object projection", "visibility", "weather"],
        evidence: "18_map_pass_classification.md; shadow_fow_projected_producer.tsv",
        status: MapFramegraphPassStatus::ImplementedSubmitPoint,
        parity_note:
            "orders 77..80 are backed by runtime tree/projected, terrainunlit/projected, and full-resolution blur producers",
    },
    MapFramegraphPassEntry {
        registry_pass: "3d_terrain",
        vanilla_order: "81",
        order_min: 81,
        order_max: 81,
        layer: "main_pdxmap_terrain",
        classification: "world_base",
        shader_or_effect: "pdxmap:terrain",
        target: "main_hdr_scene",
        reads: &[
            "TerrainDiffuse",
            "HeightNormal",
            "TerrainColorTint",
            "ProvinceSecondaryColorMap",
            "SnowMudData",
            "LightDataMap",
            "LightIndexMap",
            "ShadowMap",
            "GradientBorderChannel*",
        ],
        writes: &["main_hdr_scene", "depth_stencil"],
        state_inputs: &["camera", "date", "map mode", "FOW/Intel", "weather"],
        evidence: "map_pass_table.tsv MPT-005; pdxmap_bindings.tsv",
        status: MapFramegraphPassStatus::ImplementedSubmitPoint,
        parity_note: "submit point exists; producer equivalence is audited separately",
    },
    MapFramegraphPassEntry {
        registry_pass: "3d_border_first",
        vanilla_order: "82",
        order_min: 82,
        order_max: 82,
        layer: "borders_pre_river_water",
        classification: "world_overlay",
        shader_or_effect: "border:border",
        target: "main_hdr_scene",
        reads: &["BorderDiffuse", "ShadowMap", "SnowMudData", "IntelMap"],
        writes: &["main_hdr_scene"],
        state_inputs: &["border mode", "selected/hover/map mode"],
        evidence: "map_pass_table.tsv MPT-006; border_overlay_passes.tsv",
        status: MapFramegraphPassStatus::ImplementedSubmitPoint,
        parity_note:
            "order 82 submits pre-river/water border families into the main HDR scene",
    },
    MapFramegraphPassEntry {
        registry_pass: "3d_river",
        vanilla_order: "83",
        order_min: 83,
        order_max: 83,
        layer: "rivers",
        classification: "world_water",
        shader_or_effect: "river:river",
        target: "main_hdr_scene",
        reads: &[
            "WaterColor",
            "RiverSurface diffuse/normal/masks",
            "LEAN",
            "ProvinceSecondaryColorMap",
            "SnowMudData",
            "GradientBorderChannel*",
            "ShadowMap",
        ],
        writes: &["main_hdr_scene"],
        state_inputs: &["river geometry", "season", "FOW/Intel"],
        evidence: "map_pass_table.tsv MPT-007; river_bindings.tsv",
        status: MapFramegraphPassStatus::ImplementedSubmitPoint,
        parity_note: "independent HDR river submit point is present",
    },
    MapFramegraphPassEntry {
        registry_pass: "3d_map_layers",
        vanilla_order: "84..85",
        order_min: 84,
        order_max: 85,
        layer: "additional_terrain_layers",
        classification: "world_overlay",
        shader_or_effect: "pdxmap variants",
        target: "main_hdr_scene",
        reads: &["common terrain bank", "dynamic map bank"],
        writes: &["main_hdr_scene"],
        state_inputs: &["layer selector", "map mode"],
        evidence: "map_pass_table.tsv MPT-008; renderer_framegraph_spec.tsv",
        status: MapFramegraphPassStatus::ReservedZeroDrawBlocker,
        parity_note:
            "orders 84..85 are explicit slots, but current implementation records zero draw calls",
    },
    MapFramegraphPassEntry {
        registry_pass: "3d_water",
        vanilla_order: "86",
        order_min: 86,
        order_max: 86,
        layer: "water",
        classification: "world_water",
        shader_or_effect: "pdxwater:water",
        target: "main_hdr_scene",
        reads: &[
            "HeightTexture",
            "LEAN",
            "WaterRefraction",
            "Ice",
            "Reflection",
            "ProvinceSecondaryColorMap",
            "SnowMudData",
            "GradientBorderChannel*",
            "ShadowMap",
        ],
        writes: &["main_hdr_scene"],
        state_inputs: &[
            "water geometry",
            "refraction option",
            "quality",
            "FOW/Intel",
        ],
        evidence: "map_pass_table.tsv MPT-009; pdxwater_bindings.tsv",
        status: MapFramegraphPassStatus::ImplementedSubmitPoint,
        parity_note: "independent HDR water submit point is present",
    },
    MapFramegraphPassEntry {
        registry_pass: "3d_border_second",
        vanilla_order: "87",
        order_min: 87,
        order_max: 87,
        layer: "borders_after_water",
        classification: "world_overlay",
        shader_or_effect: "border:border",
        target: "main_hdr_scene",
        reads: &["BorderDiffuse", "ShadowMap", "SnowMudData", "IntelMap"],
        writes: &["main_hdr_scene"],
        state_inputs: &["border mode", "selected/hover/map mode"],
        evidence: "map_pass_table.tsv MPT-006; border_overlay_passes.tsv",
        status: MapFramegraphPassStatus::ImplementedSubmitPoint,
        parity_note:
            "order 87 submits post-water land border families into the main HDR scene",
    },
    MapFramegraphPassEntry {
        registry_pass: "hdr_map_overlay",
        vanilla_order: "88",
        order_min: 88,
        order_max: 88,
        layer: "HDR_map_overlay",
        classification: "world_overlay",
        shader_or_effect: "overlay hash 8abb94dafcb589ed",
        target: "main_hdr_scene",
        reads: &[
            "64x64 BC3 map texture",
            "SnowMudData",
            "IntelMap",
            "ShadowMap",
        ],
        writes: &["main_hdr_scene"],
        state_inputs: &["map overlay selector"],
        evidence: "map_pass_table.tsv MPT-010; auxiliary_overlay_pass_classification.tsv",
        status: MapFramegraphPassStatus::BoundedFamilyRegistered,
        parity_note: "order 88 is bounded in the registry; exact semantic label is non-blocking",
    },
    MapFramegraphPassEntry {
        registry_pass: "postprocess",
        vanilla_order: "134",
        order_min: 134,
        order_max: 134,
        layer: "bloom/restorescene prepass",
        classification: "postprocess",
        shader_or_effect: "bloom/restorescene prepass",
        target: "RestoreBloom",
        reads: &["MainScene half-res source"],
        writes: &["Bloom source", "postfx source"],
        state_inputs: &["HDR constants"],
        evidence: "postprocess_full_order.tsv",
        status: MapFramegraphPassStatus::PostprocessSubpassRegistered,
        parity_note: "postprocess order is represented by the postprocess chain registry pass",
    },
    MapFramegraphPassEntry {
        registry_pass: "postprocess",
        vanilla_order: "135",
        order_min: 135,
        order_max: 135,
        layer: "LuminanceDownsample",
        classification: "postprocess",
        shader_or_effect: "LuminanceDownsample",
        target: "AverageLuminance chain",
        reads: &["half-res source"],
        writes: &["log luminance"],
        state_inputs: &["HDR constants"],
        evidence: "postprocess_full_order.tsv",
        status: MapFramegraphPassStatus::PostprocessSubpassRegistered,
        parity_note: "postprocess order is represented by the postprocess chain registry pass",
    },
    MapFramegraphPassEntry {
        registry_pass: "postprocess",
        vanilla_order: "136",
        order_min: 136,
        order_max: 136,
        layer: "LuminanceGather",
        classification: "postprocess",
        shader_or_effect: "LuminanceGather",
        target: "AverageLuminance chain",
        reads: &["downsampled luminance"],
        writes: &["gathered luminance"],
        state_inputs: &["HDR constants"],
        evidence: "postprocess_full_order.tsv",
        status: MapFramegraphPassStatus::PostprocessSubpassRegistered,
        parity_note: "postprocess order is represented by the postprocess chain registry pass",
    },
    MapFramegraphPassEntry {
        registry_pass: "postprocess",
        vanilla_order: "137",
        order_min: 137,
        order_max: 137,
        layer: "LuminanceGather temporal/adapt",
        classification: "postprocess",
        shader_or_effect: "LuminanceGather temporal/adapt",
        target: "AverageLuminance chain",
        reads: &["current luminance", "last luminance"],
        writes: &["adapted luminance"],
        state_inputs: &["HDR constants", "previous frame luminance"],
        evidence: "postprocess_full_order.tsv",
        status: MapFramegraphPassStatus::PostprocessSubpassRegistered,
        parity_note: "postprocess order is represented by the postprocess chain registry pass",
    },
    MapFramegraphPassEntry {
        registry_pass: "postprocess",
        vanilla_order: "138",
        order_min: 138,
        order_max: 138,
        layer: "downsample final",
        classification: "postprocess",
        shader_or_effect: "downsample final",
        target: "AverageLuminance",
        reads: &["gathered luminance"],
        writes: &["AverageLuminance"],
        state_inputs: &["HDR constants"],
        evidence: "postprocess_full_order.tsv",
        status: MapFramegraphPassStatus::PostprocessSubpassRegistered,
        parity_note: "postprocess order is represented by the postprocess chain registry pass",
    },
    MapFramegraphPassEntry {
        registry_pass: "postprocess",
        vanilla_order: "139",
        order_min: 139,
        order_max: 139,
        layer: "restorescene",
        classification: "postprocess",
        shader_or_effect: "restorescene",
        target: "swapchain",
        reads: &["MainScene", "RestoreBloom", "ColorCube", "AverageLuminance"],
        writes: &["LDR map scene"],
        state_inputs: &["posteffect volumes", "LUT selection", "fog/day-night"],
        evidence: "postprocess_full_order.tsv; colorcube_lut_selection.tsv",
        status: MapFramegraphPassStatus::PostprocessSubpassRegistered,
        parity_note: "final LDR restore order is fixed before UI/HUD",
    },
    MapFramegraphPassEntry {
        registry_pass: "ui",
        vanilla_order: "140+",
        order_min: 140,
        order_max: 485,
        layer: "HUD_UI_tail",
        classification: "excluded_hud",
        shader_or_effect: "sprite/UI",
        target: "swapchain",
        reads: &["UI atlases", "cursor", "panels"],
        writes: &["swapchain"],
        state_inputs: &["UI state"],
        evidence: "18_map_pass_classification.md",
        status: MapFramegraphPassStatus::OutsideHudScope,
        parity_note: "excluded from non-HUD map parity gate",
    },
];

pub const MAP_DEBUG_CAPTURE_PLAN: &[MapDebugCaptureEntry] = &[
    MapDebugCaptureEntry {
        family: "terrain",
        registry_pass: "3d_terrain",
        capture_layer: "terrain",
        source_target: "main_hdr_scene terrain-only",
        output_pattern: "target/map_parity/<batch>/project/<scene>/terrain.high.png",
        evidence: "MapBaselineLayer::TerrainOnly",
    },
    MapDebugCaptureEntry {
        family: "terrain",
        registry_pass: "3d_terrain",
        capture_layer: "province_secondary",
        source_target: "ProvinceSecondaryColorMap",
        output_pattern: "target/map_parity/<batch>/project/<scene>/province_secondary.high.png",
        evidence: "MapBaselineLayer::ProvinceSecondaryDebug",
    },
    MapDebugCaptureEntry {
        family: "terrain",
        registry_pass: "3d_terrain",
        capture_layer: "terrain_river_mask",
        source_target: "terrain material river mask",
        output_pattern: "target/map_parity/<batch>/project/<scene>/terrain_river_mask.high.png",
        evidence: "MapBaselineLayer::TerrainRiverMaskDebug",
    },
    MapDebugCaptureEntry {
        family: "terrain",
        registry_pass: "3d_terrain",
        capture_layer: "fow_visibility",
        source_target: "FOW visibility texture",
        output_pattern: "target/map_parity/<batch>/project/<scene>/fow_visibility.high.png",
        evidence: "MapBaselineLayer::FowVisibilityDebug",
    },
    MapDebugCaptureEntry {
        family: "terrain",
        registry_pass: "3d_terrain",
        capture_layer: "terrain_final_before_postprocess",
        source_target: "terrain material final color before postprocess",
        output_pattern:
            "target/map_parity/<batch>/project/<scene>/terrain_final_before_postprocess.high.png",
        evidence: "MapBaselineLayer::TerrainFinalBeforePostprocessDebug",
    },
    MapDebugCaptureEntry {
        family: "water",
        registry_pass: "3d_water",
        capture_layer: "water",
        source_target: "main_hdr_scene water-only",
        output_pattern: "target/map_parity/<batch>/project/<scene>/water.high.png",
        evidence: "MapBaselineLayer::WaterOnly",
    },
    MapDebugCaptureEntry {
        family: "river",
        registry_pass: "3d_river",
        capture_layer: "rivers",
        source_target: "main_hdr_scene river-only",
        output_pattern: "target/map_parity/<batch>/project/<scene>/rivers.high.png",
        evidence: "MapBaselineLayer::RiverMask",
    },
    MapDebugCaptureEntry {
        family: "border",
        registry_pass: "3d_border_second",
        capture_layer: "borders",
        source_target: "main_hdr_scene borders-only",
        output_pattern: "target/map_parity/<batch>/project/<scene>/borders.high.png",
        evidence: "MapBaselineLayer::BordersOnly",
    },
    MapDebugCaptureEntry {
        family: "postprocess",
        registry_pass: "postprocess",
        capture_layer: "hdr",
        source_target: "MainScene HDR",
        output_pattern: "target/map_parity/<batch>/project/<scene>/hdr.high.png",
        evidence: "MapBaselineLayer::HdrRaw",
    },
    MapDebugCaptureEntry {
        family: "postprocess",
        registry_pass: "postprocess",
        capture_layer: "bloom",
        source_target: "RestoreBloom",
        output_pattern: "target/map_parity/<batch>/project/<scene>/bloom.high.png",
        evidence: "MapBaselineLayer::BloomOnly",
    },
    MapDebugCaptureEntry {
        family: "postprocess",
        registry_pass: "postprocess",
        capture_layer: "avg_luminance",
        source_target: "AverageLuminance",
        output_pattern: "target/map_parity/<batch>/project/<scene>/avg_luminance.high.png",
        evidence: "MapBaselineLayer::AvgLuminance",
    },
    MapDebugCaptureEntry {
        family: "postprocess",
        registry_pass: "postprocess",
        capture_layer: "tonemap_before",
        source_target: "RestoreScene tonemap input",
        output_pattern: "target/map_parity/<batch>/project/<scene>/tonemap_before.high.png",
        evidence: "MapBaselineLayer::TonemapBefore",
    },
    MapDebugCaptureEntry {
        family: "postprocess",
        registry_pass: "postprocess",
        capture_layer: "tonemap",
        source_target: "RestoreScene tonemap output",
        output_pattern: "target/map_parity/<batch>/project/<scene>/tonemap.high.png",
        evidence: "MapBaselineLayer::TonemapOnly",
    },
    MapDebugCaptureEntry {
        family: "postprocess",
        registry_pass: "postprocess",
        capture_layer: "lut_before",
        source_target: "ColorCube input",
        output_pattern: "target/map_parity/<batch>/project/<scene>/lut_before.high.png",
        evidence: "MapBaselineLayer::LutBefore",
    },
    MapDebugCaptureEntry {
        family: "postprocess",
        registry_pass: "postprocess",
        capture_layer: "lut_after",
        source_target: "ColorCube output",
        output_pattern: "target/map_parity/<batch>/project/<scene>/lut_after.high.png",
        evidence: "MapBaselineLayer::LutAfter",
    },
    MapDebugCaptureEntry {
        family: "postprocess",
        registry_pass: "postprocess",
        capture_layer: "postprocess_off",
        source_target: "MainScene without postprocess",
        output_pattern: "target/map_parity/<batch>/project/<scene>/postprocess_off.high.png",
        evidence: "MapBaselineLayer::PostprocessOff",
    },
];

pub fn pass_registry_entries() -> &'static [MapFramegraphPassEntry] {
    NON_HUD_MAP_PASS_REGISTRY
}

pub fn debug_capture_entries() -> &'static [MapDebugCaptureEntry] {
    MAP_DEBUG_CAPTURE_PLAN
}

pub fn zero_draw_blocker_entries() -> impl Iterator<Item = &'static MapFramegraphPassEntry> {
    NON_HUD_MAP_PASS_REGISTRY
        .iter()
        .filter(|entry| entry.is_zero_draw_blocker())
}

pub fn zero_draw_blocker_count() -> usize {
    zero_draw_blocker_entries().count()
}

pub fn phase2_gate_status() -> &'static str {
    if zero_draw_blocker_count() == 0 {
        "pass"
    } else {
        "blocked"
    }
}

pub fn orders_77_88_locked() -> bool {
    let required = [
        (77, 80, "projected_fow_shadow"),
        (81, 81, "3d_terrain"),
        (82, 82, "3d_border_first"),
        (83, 83, "3d_river"),
        (84, 85, "3d_map_layers"),
        (86, 86, "3d_water"),
        (87, 87, "3d_border_second"),
        (88, 88, "hdr_map_overlay"),
    ];
    required.iter().all(|(min, max, pass)| {
        NON_HUD_MAP_PASS_REGISTRY.iter().any(|entry| {
            entry.order_min == *min && entry.order_max == *max && entry.registry_pass == *pass
        })
    })
}

pub fn postprocess_orders_134_139_locked() -> bool {
    (134..=139).all(|order| {
        NON_HUD_MAP_PASS_REGISTRY
            .iter()
            .any(|entry| entry.order_min == order && entry.order_max == order)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn phase2_registry_locks_main_hdr_orders_77_to_88() {
        assert!(orders_77_88_locked());
        let order_passes: Vec<_> = pass_registry_entries()
            .iter()
            .filter(|entry| entry.is_main_hdr_order_77_88())
            .map(|entry| (entry.vanilla_order, entry.registry_pass, entry.target))
            .collect();
        assert_eq!(
            order_passes,
            vec![
                ("77..80", "projected_fow_shadow", "ShadowMap"),
                ("81", "3d_terrain", "main_hdr_scene"),
                ("82", "3d_border_first", "main_hdr_scene"),
                ("83", "3d_river", "main_hdr_scene"),
                ("84..85", "3d_map_layers", "main_hdr_scene"),
                ("86", "3d_water", "main_hdr_scene"),
                ("87", "3d_border_second", "main_hdr_scene"),
                ("88", "hdr_map_overlay", "main_hdr_scene"),
            ]
        );
    }

    #[test]
    fn phase2_registry_locks_postprocess_orders_134_to_139() {
        assert!(postprocess_orders_134_139_locked());
        let orders: Vec<_> = pass_registry_entries()
            .iter()
            .filter(|entry| entry.is_postprocess_order_134_139())
            .map(|entry| entry.order_min)
            .collect();
        assert_eq!(orders, vec![134, 135, 136, 137, 138, 139]);
        assert!(pass_registry_entries().iter().any(|entry| {
            entry.order_min == 139
                && entry.registry_pass == "postprocess"
                && entry.reads.contains(&"ColorCube")
                && entry.writes.contains(&"LDR map scene")
        }));
    }

    #[test]
    fn phase2_zero_draw_slots_are_blockers() {
        let blockers: HashSet<_> = zero_draw_blocker_entries()
            .map(|entry| entry.registry_pass)
            .collect();
        assert_eq!(zero_draw_blocker_count(), 1);
        assert!(!blockers.contains("projected_fow_shadow"));
        assert!(blockers.contains("3d_map_layers"));
        assert_eq!(phase2_gate_status(), "blocked");
    }

    #[test]
    fn phase2_debug_capture_plan_covers_required_families() {
        let families: HashSet<_> = debug_capture_entries()
            .iter()
            .map(|entry| entry.family)
            .collect();
        for required in ["terrain", "water", "river", "border", "postprocess"] {
            assert!(families.contains(required), "missing {required} capture");
        }
        assert!(debug_capture_entries()
            .iter()
            .any(|entry| entry.capture_layer == "terrain_final_before_postprocess"));
        assert!(debug_capture_entries()
            .iter()
            .any(|entry| entry.capture_layer == "lut_after"));
    }

    #[test]
    fn phase2_registry_entries_have_targets_and_io() {
        for entry in pass_registry_entries()
            .iter()
            .filter(|entry| entry.status != MapFramegraphPassStatus::OutsideHudScope)
        {
            assert!(
                !entry.target.is_empty(),
                "{} missing target",
                entry.registry_pass
            );
            assert!(
                !entry.reads.is_empty(),
                "{} missing reads",
                entry.registry_pass
            );
            assert!(
                !entry.writes.is_empty(),
                "{} missing writes",
                entry.registry_pass
            );
            assert!(
                !entry.evidence.is_empty() && !entry.parity_note.is_empty(),
                "{} missing audit evidence",
                entry.registry_pass
            );
        }
    }
}
