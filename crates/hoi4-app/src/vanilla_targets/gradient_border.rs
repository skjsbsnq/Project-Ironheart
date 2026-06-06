use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use hoi4_render::sdf::compute_country_sdf;
use hoi4_state::World;

use super::VanillaRuntimeTargetInputs;

pub const VANILLA_GRADIENT_BORDER_WIDTH: u32 = 2816;
pub const VANILLA_GRADIENT_BORDER_HEIGHT: u32 = 2050;
pub const VANILLA_GRADIENT_BORDER_PAGE_HEIGHT: u32 = 1025;
pub const VANILLA_GRADIENT_BORDER_CONTENT_HEIGHT: u32 = 1024;
pub const VANILLA_GRADIENT_BORDER_CH3_WIDTH: u32 = 2;
pub const VANILLA_GRADIENT_BORDER_CH3_HEIGHT: u32 = 1;

pub struct GradientBorderCpuTargets {
    pub ch1: Vec<u8>,
    pub ch2: Vec<u8>,
    pub ch3: Vec<u8>,
}

pub fn generate(inputs: VanillaRuntimeTargetInputs<'_>) -> GradientBorderCpuTargets {
    let width = inputs.world.map.province_map.width;
    let height = inputs.world.map.province_map.height;
    let plan = active_producer_plan(map_mode_code_for_gradient(inputs.default_map_mode_code));
    let bank = GradientBorderLayerBank {
        country_sdf: inputs.country_sdf,
        province_sdf: inputs.province_sdf,
        coast_sdf: inputs.coast_sdf,
    };
    GradientBorderCpuTargets {
        ch1: pack_two_page_logical_bank(bank, width, height, plan.anchors),
        ch2: pack_two_page_logical_bank(bank, width, height, plan.anchors),
        ch3: neutral_channel3(),
    }
}

pub fn generate_runtime_channels(world: &World, map_mode_code: u8) -> GradientBorderCpuTargets {
    let country_sdf = compute_country_sdf(&world.map.province_map, &world.provinces.controllers);
    let province_sdf = hoi4_render::sdf::compute_province_sdf(&world.map.province_map);
    let plan = active_producer_plan(map_mode_code_for_gradient(map_mode_code));
    let bank = GradientBorderLayerBank {
        country_sdf: &country_sdf,
        province_sdf: &province_sdf,
        coast_sdf: &[],
    };
    GradientBorderCpuTargets {
        ch1: pack_two_page_logical_bank(
            bank,
            world.map.province_map.width,
            world.map.province_map.height,
            plan.anchors,
        ),
        ch2: pack_two_page_logical_bank(
            bank,
            world.map.province_map.width,
            world.map.province_map.height,
            plan.anchors,
        ),
        ch3: neutral_channel3(),
    }
}

pub fn producer_signature(_world: &World, map_mode_code: u8) -> u64 {
    let mut h = DefaultHasher::new();
    map_mode_code.hash(&mut h);
    let anchors = active_anchor_pages(map_mode_code_for_gradient(map_mode_code));
    anchors.page0.hash(&mut h);
    anchors.page1.hash(&mut h);
    h.finish()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GradientBorderAnchors {
    pub page0: i16,
    pub page1: i16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GradientBorderLayerSource {
    CountryController,
    ProvinceBoundary,
    Coast,
    NeutralFallback,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GradientBorderLogicalLayer {
    pub id: i16,
    pub name: &'static str,
    pub source: GradientBorderLayerSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GradientBorderProducerPlan {
    pub anchors: GradientBorderAnchors,
    pub page0_layer: Option<GradientBorderLogicalLayer>,
    pub page1_layer: Option<GradientBorderLogicalLayer>,
    pub producer_reason: &'static str,
}

pub const GRADIENT_BORDER_LOGICAL_LAYER_COUNT: usize = 23;

pub const GRADIENT_BORDER_LOGICAL_LAYERS: [GradientBorderLogicalLayer;
    GRADIENT_BORDER_LOGICAL_LAYER_COUNT] = [
    GradientBorderLogicalLayer {
        id: 0,
        name: "country_controller_border",
        source: GradientBorderLayerSource::CountryController,
    },
    GradientBorderLogicalLayer {
        id: 1,
        name: "province_border",
        source: GradientBorderLayerSource::ProvinceBoundary,
    },
    GradientBorderLogicalLayer {
        id: 2,
        name: "coast_or_impassable_edge",
        source: GradientBorderLayerSource::Coast,
    },
    GradientBorderLogicalLayer {
        id: 3,
        name: "province_overlay_edge_a",
        source: GradientBorderLayerSource::ProvinceBoundary,
    },
    GradientBorderLogicalLayer {
        id: 4,
        name: "country_overlay_edge_a",
        source: GradientBorderLayerSource::CountryController,
    },
    GradientBorderLogicalLayer {
        id: 5,
        name: "map_mode_primary_border",
        source: GradientBorderLayerSource::CountryController,
    },
    GradientBorderLogicalLayer {
        id: 6,
        name: "country_overlay_edge_b",
        source: GradientBorderLayerSource::CountryController,
    },
    GradientBorderLogicalLayer {
        id: 7,
        name: "country_overlay_edge_c",
        source: GradientBorderLayerSource::CountryController,
    },
    GradientBorderLogicalLayer {
        id: 8,
        name: "country_overlay_edge_d",
        source: GradientBorderLayerSource::CountryController,
    },
    GradientBorderLogicalLayer {
        id: 9,
        name: "province_overlay_edge_b",
        source: GradientBorderLayerSource::ProvinceBoundary,
    },
    GradientBorderLogicalLayer {
        id: 10,
        name: "country_overlay_edge_e",
        source: GradientBorderLayerSource::CountryController,
    },
    GradientBorderLogicalLayer {
        id: 11,
        name: "province_overlay_edge_c",
        source: GradientBorderLayerSource::ProvinceBoundary,
    },
    GradientBorderLogicalLayer {
        id: 12,
        name: "country_overlay_edge_f",
        source: GradientBorderLayerSource::CountryController,
    },
    GradientBorderLogicalLayer {
        id: 13,
        name: "country_overlay_edge_g",
        source: GradientBorderLayerSource::CountryController,
    },
    GradientBorderLogicalLayer {
        id: 14,
        name: "province_overlay_edge_d",
        source: GradientBorderLayerSource::ProvinceBoundary,
    },
    GradientBorderLogicalLayer {
        id: 15,
        name: "province_overlay_edge_e",
        source: GradientBorderLayerSource::ProvinceBoundary,
    },
    GradientBorderLogicalLayer {
        id: 16,
        name: "country_overlay_edge_h",
        source: GradientBorderLayerSource::CountryController,
    },
    GradientBorderLogicalLayer {
        id: 17,
        name: "province_overlay_edge_f",
        source: GradientBorderLayerSource::ProvinceBoundary,
    },
    GradientBorderLogicalLayer {
        id: 18,
        name: "country_overlay_edge_i",
        source: GradientBorderLayerSource::CountryController,
    },
    GradientBorderLogicalLayer {
        id: 19,
        name: "province_overlay_edge_g",
        source: GradientBorderLayerSource::ProvinceBoundary,
    },
    GradientBorderLogicalLayer {
        id: 20,
        name: "country_overlay_edge_j",
        source: GradientBorderLayerSource::CountryController,
    },
    GradientBorderLogicalLayer {
        id: 21,
        name: "province_overlay_edge_h",
        source: GradientBorderLayerSource::ProvinceBoundary,
    },
    GradientBorderLogicalLayer {
        id: 22,
        name: "province_overlay_edge_i",
        source: GradientBorderLayerSource::ProvinceBoundary,
    },
];

pub fn active_producer_plan(map_mode_id: i16) -> GradientBorderProducerPlan {
    let anchors = active_anchor_pages(map_mode_id);
    GradientBorderProducerPlan {
        anchors,
        page0_layer: logical_layer(anchors.page0),
        page1_layer: logical_layer(anchors.page1),
        producer_reason:
            "active pages come from gradient_border_map_mode_anchors.tsv; pixels are project-generated logical-layer SDF fallbacks",
    }
}

pub fn logical_layer(id: i16) -> Option<GradientBorderLogicalLayer> {
    GRADIENT_BORDER_LOGICAL_LAYERS
        .iter()
        .copied()
        .find(|layer| layer.id == id)
}

pub fn active_anchor_pages(map_mode_id: i16) -> GradientBorderAnchors {
    let (page0, page1) = match map_mode_id {
        -1 => (-1, -1),
        0 => (5, 0),
        1 => (5, -1),
        2 => (5, 0),
        3 => (0, 10),
        4 => (0, 2),
        5 => (4, 3),
        6 | 7 => (0, 12),
        8 => (0, -1),
        9 => (1, -1),
        10 | 11 => (0, -1),
        12 => (7, 0),
        13 => (8, 0),
        14 => (0, -1),
        15 => (9, 0),
        16 => (0, -1),
        17 => (0, 5),
        18..=21 => (0, -1),
        22 => (0, 19),
        23 => (1, -1),
        24 => (-1, -1),
        25 => (2, -1),
        26 => (5, -1),
        27 => (6, -1),
        28 | 29 => (0, -1),
        30 => (1, 11),
        31 => (0, 13),
        32 => (5, -1),
        33 => (14, -1),
        34 => (17, -1),
        35 => (18, -1),
        36 => (20, -1),
        37 => (21, -1),
        38 => (0, -1),
        39 => (0, 22),
        _ => (0, -1),
    };
    GradientBorderAnchors { page0, page1 }
}

fn map_mode_code_for_gradient(code: u8) -> i16 {
    code as i16
}

#[derive(Clone, Copy)]
struct GradientBorderLayerBank<'a> {
    country_sdf: &'a [u8],
    province_sdf: &'a [u8],
    coast_sdf: &'a [u8],
}

fn pack_two_page_logical_bank(
    bank: GradientBorderLayerBank<'_>,
    source_width: u32,
    source_height: u32,
    anchors: GradientBorderAnchors,
) -> Vec<u8> {
    let mut out =
        vec![255u8; (VANILLA_GRADIENT_BORDER_WIDTH * VANILLA_GRADIENT_BORDER_HEIGHT * 4) as usize];
    write_page(
        &mut out,
        0,
        bank,
        source_width,
        source_height,
        anchors.page0,
    );
    write_page(
        &mut out,
        VANILLA_GRADIENT_BORDER_PAGE_HEIGHT,
        bank,
        source_width,
        source_height,
        anchors.page1,
    );
    out
}

fn write_page(
    out: &mut [u8],
    dst_y_base: u32,
    bank: GradientBorderLayerBank<'_>,
    source_width: u32,
    source_height: u32,
    layer_id: i16,
) {
    let layer = logical_layer(layer_id);
    let source = layer.and_then(|layer| source_for_layer(bank, layer.source));
    for y in 0..VANILLA_GRADIENT_BORDER_PAGE_HEIGHT {
        for x in 0..VANILLA_GRADIENT_BORDER_WIDTH {
            let value = source
                .map(|source| sample_scaled_sdf(source, source_width, source_height, x, y))
                .unwrap_or(255);
            let stored = encode_linear_unorm_for_srgb_texture(value);
            let o = (((dst_y_base + y) * VANILLA_GRADIENT_BORDER_WIDTH + x) * 4) as usize;
            out[o] = stored;
            out[o + 1] = stored;
            out[o + 2] = stored;
            out[o + 3] = 255;
        }
    }
}

fn encode_linear_unorm_for_srgb_texture(value: u8) -> u8 {
    let linear = value as f32 / 255.0;
    let encoded = if linear <= 0.003_130_8 {
        linear * 12.92
    } else {
        1.055 * linear.powf(1.0 / 2.4) - 0.055
    };
    (encoded.clamp(0.0, 1.0) * 255.0).round() as u8
}

fn source_for_layer(
    bank: GradientBorderLayerBank<'_>,
    source: GradientBorderLayerSource,
) -> Option<&[u8]> {
    match source {
        GradientBorderLayerSource::CountryController => Some(bank.country_sdf),
        GradientBorderLayerSource::ProvinceBoundary => Some(bank.province_sdf),
        GradientBorderLayerSource::Coast => Some(bank.coast_sdf),
        GradientBorderLayerSource::NeutralFallback => None,
    }
    .filter(|source| !source.is_empty())
}

fn sample_scaled_sdf(
    source: &[u8],
    source_width: u32,
    source_height: u32,
    x: u32,
    page_y: u32,
) -> u8 {
    if source_width == 0 || source_height == 0 {
        return 255;
    }
    let expected = source_width as usize * source_height as usize;
    if source.len() != expected {
        return 255;
    }
    let src_x = ((x as u64 * source_width as u64) / VANILLA_GRADIENT_BORDER_WIDTH as u64)
        .min(source_width.saturating_sub(1) as u64) as u32;
    let content_y = page_y.min(VANILLA_GRADIENT_BORDER_CONTENT_HEIGHT - 1);
    let src_y = ((content_y as u64 * source_height as u64)
        / VANILLA_GRADIENT_BORDER_CONTENT_HEIGHT as u64)
        .min(source_height.saturating_sub(1) as u64) as u32;
    source[(src_y * source_width + src_x) as usize]
}

fn neutral_channel3() -> Vec<u8> {
    vec![255, 255, 255, 255, 255, 255, 255, 255]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packed_gradient_channel_matches_vanilla_dimensions() {
        let bank = GradientBorderLayerBank {
            country_sdf: &[0, 64, 128, 255],
            province_sdf: &[255, 128, 64, 0],
            coast_sdf: &[],
        };
        let data =
            pack_two_page_logical_bank(bank, 2, 2, GradientBorderAnchors { page0: 5, page1: 0 });
        assert_eq!(
            data.len(),
            (VANILLA_GRADIENT_BORDER_WIDTH * VANILLA_GRADIENT_BORDER_HEIGHT * 4) as usize
        );
        assert_eq!(data[3], 255);
        assert_eq!(data[7], 255);
    }

    #[test]
    fn disabled_anchor_writes_flat_far_page() {
        let bank = GradientBorderLayerBank {
            country_sdf: &[0, 64, 128, 255],
            province_sdf: &[255, 128, 64, 0],
            coast_sdf: &[],
        };
        let data = pack_two_page_logical_bank(
            bank,
            2,
            2,
            GradientBorderAnchors {
                page0: -1,
                page1: -1,
            },
        );
        assert!(data.iter().all(|&v| v == 255));
    }

    #[test]
    fn packed_gradient_values_are_srgb_encoded_for_linear_sampling() {
        assert_eq!(encode_linear_unorm_for_srgb_texture(0), 0);
        assert_eq!(encode_linear_unorm_for_srgb_texture(255), 255);
        assert!(encode_linear_unorm_for_srgb_texture(64) > 64);
        assert!(encode_linear_unorm_for_srgb_texture(128) > 128);
    }

    #[test]
    fn channel3_is_two_by_one_neutral_bgra() {
        assert_eq!(neutral_channel3(), vec![255; 8]);
    }

    #[test]
    fn active_anchor_pages_cover_reverse_table_examples() {
        assert_eq!(
            active_anchor_pages(0),
            GradientBorderAnchors { page0: 5, page1: 0 }
        );
        assert_eq!(
            active_anchor_pages(30),
            GradientBorderAnchors {
                page0: 1,
                page1: 11
            }
        );
        assert_eq!(
            active_anchor_pages(39),
            GradientBorderAnchors {
                page0: 0,
                page1: 22
            }
        );
    }

    #[test]
    fn logical_layers_cover_0_to_22() {
        assert_eq!(
            GRADIENT_BORDER_LOGICAL_LAYERS.len(),
            GRADIENT_BORDER_LOGICAL_LAYER_COUNT
        );
        for id in 0..GRADIENT_BORDER_LOGICAL_LAYER_COUNT as i16 {
            assert_eq!(logical_layer(id).map(|layer| layer.id), Some(id));
        }
        assert!(logical_layer(GRADIENT_BORDER_LOGICAL_LAYER_COUNT as i16).is_none());
    }

    #[test]
    fn active_producer_plan_reports_active_layers_and_reason() {
        let plan = active_producer_plan(0);
        assert_eq!(plan.anchors, GradientBorderAnchors { page0: 5, page1: 0 });
        assert_eq!(plan.page0_layer.map(|layer| layer.id), Some(5));
        assert_eq!(plan.page1_layer.map(|layer| layer.id), Some(0));
        assert!(plan.producer_reason.contains("active pages"));
    }

    #[test]
    fn political_map_primary_border_uses_country_controller_not_province_grid() {
        let plan = active_producer_plan(0);
        assert_eq!(
            plan.page0_layer.map(|layer| layer.source),
            Some(GradientBorderLayerSource::CountryController)
        );
        assert_eq!(
            plan.page1_layer.map(|layer| layer.source),
            Some(GradientBorderLayerSource::CountryController)
        );
    }

    #[test]
    fn producer_signature_tracks_map_mode_anchor_changes() {
        assert_ne!(active_anchor_pages(0), active_anchor_pages(1));
    }
}
