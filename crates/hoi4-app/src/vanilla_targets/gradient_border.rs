use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use hoi4_map::ProvinceType;
use hoi4_render::sdf::compute_country_sdf;
use hoi4_state::{CountryId, World};

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
        province_pixels: &inputs.world.map.province_map.pixels,
        province_definitions: &inputs.world.map.definitions,
        controllers: &inputs.world.provinces.controllers,
        country_colors: &inputs.world.countries.colors,
        country_sdf: inputs.country_sdf,
        province_sdf: inputs.province_sdf,
        coast_sdf: inputs.coast_sdf,
    };
    GradientBorderCpuTargets {
        ch1: pack_channel1_country_color_distance(bank, width, height, plan.anchors),
        ch2: pack_channel2_gate_and_fx(bank, width, height),
        ch3: neutral_channel3(),
    }
}

pub fn generate_runtime_channels(world: &World, map_mode_code: u8) -> GradientBorderCpuTargets {
    let country_sdf = compute_country_sdf(&world.map.province_map, &world.provinces.controllers);
    let province_sdf = hoi4_render::sdf::compute_province_sdf(&world.map.province_map);
    let plan = active_producer_plan(map_mode_code_for_gradient(map_mode_code));
    let bank = GradientBorderLayerBank {
        province_pixels: &world.map.province_map.pixels,
        province_definitions: &world.map.definitions,
        controllers: &world.provinces.controllers,
        country_colors: &world.countries.colors,
        country_sdf: &country_sdf,
        province_sdf: &province_sdf,
        coast_sdf: &[],
    };
    GradientBorderCpuTargets {
        ch1: pack_channel1_country_color_distance(
            bank,
            world.map.province_map.width,
            world.map.province_map.height,
            plan.anchors,
        ),
        ch2: pack_channel2_gate_and_fx(
            bank,
            world.map.province_map.width,
            world.map.province_map.height,
        ),
        ch3: neutral_channel3(),
    }
}

pub fn producer_signature(world: &World, map_mode_code: u8) -> u64 {
    let mut h = DefaultHasher::new();
    map_mode_code.hash(&mut h);
    let anchors = active_anchor_pages(map_mode_code_for_gradient(map_mode_code));
    anchors.page0.hash(&mut h);
    anchors.page1.hash(&mut h);
    world.map.province_map.width.hash(&mut h);
    world.map.province_map.height.hash(&mut h);
    for controller in &world.provinces.controllers {
        controller.raw().hash(&mut h);
    }
    for color in &world.countries.colors {
        color.hash(&mut h);
    }
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
    province_pixels: &'a [u16],
    province_definitions: &'a [Option<hoi4_map::ProvinceDefinition>],
    controllers: &'a [CountryId],
    country_colors: &'a [[u8; 3]],
    country_sdf: &'a [u8],
    province_sdf: &'a [u8],
    coast_sdf: &'a [u8],
}

fn pack_channel1_country_color_distance(
    bank: GradientBorderLayerBank<'_>,
    source_width: u32,
    source_height: u32,
    anchors: GradientBorderAnchors,
) -> Vec<u8> {
    let mut out =
        vec![0u8; (VANILLA_GRADIENT_BORDER_WIDTH * VANILLA_GRADIENT_BORDER_HEIGHT * 4) as usize];
    write_channel1_page(
        &mut out,
        0,
        bank,
        source_width,
        source_height,
        anchors.page0,
    );
    write_channel1_page(
        &mut out,
        VANILLA_GRADIENT_BORDER_PAGE_HEIGHT,
        bank,
        source_width,
        source_height,
        anchors.page1,
    );
    out
}

fn pack_channel2_gate_and_fx(
    bank: GradientBorderLayerBank<'_>,
    source_width: u32,
    source_height: u32,
) -> Vec<u8> {
    let mut out =
        vec![0u8; (VANILLA_GRADIENT_BORDER_WIDTH * VANILLA_GRADIENT_BORDER_HEIGHT * 4) as usize];
    write_channel2_page(&mut out, 0, bank, source_width, source_height);
    write_channel2_page(
        &mut out,
        VANILLA_GRADIENT_BORDER_PAGE_HEIGHT,
        bank,
        source_width,
        source_height,
    );
    out
}

fn write_channel1_page(
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
            let (src_x, src_y) = scaled_source_xy(source_width, source_height, x, y);
            let country_color =
                country_color_at_source(bank, source_width, source_height, src_x, src_y);
            let alpha = match (country_color, source) {
                (Some(_), Some(source)) => {
                    let sdf =
                        sample_sdf_at_source(source, source_width, source_height, src_x, src_y);
                    255u8.saturating_sub(sdf)
                }
                _ => 0,
            };
            let color = if source.is_some() {
                country_color.unwrap_or([0, 0, 0])
            } else {
                [0, 0, 0]
            };
            let o = (((dst_y_base + y) * VANILLA_GRADIENT_BORDER_WIDTH + x) * 4) as usize;
            write_bgra_pixel(out, o, color[0], color[1], color[2], alpha);
        }
    }
}

fn write_channel2_page(
    out: &mut [u8],
    dst_y_base: u32,
    bank: GradientBorderLayerBank<'_>,
    source_width: u32,
    source_height: u32,
) {
    for y in 0..VANILLA_GRADIENT_BORDER_PAGE_HEIGHT {
        for x in 0..VANILLA_GRADIENT_BORDER_WIDTH {
            let (src_x, src_y) = scaled_source_xy(source_width, source_height, x, y);
            let gate = if has_owned_land_controller(bank, source_width, source_height, src_x, src_y)
            {
                255
            } else {
                0
            };
            let o = (((dst_y_base + y) * VANILLA_GRADIENT_BORDER_WIDTH + x) * 4) as usize;
            write_bgra_pixel(out, o, 0, gate, 0, gate);
        }
    }
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

fn scaled_source_xy(source_width: u32, source_height: u32, x: u32, page_y: u32) -> (u32, u32) {
    if source_width == 0 || source_height == 0 {
        return (0, 0);
    }
    let src_x = ((x as u64 * source_width as u64) / VANILLA_GRADIENT_BORDER_WIDTH as u64)
        .min(source_width.saturating_sub(1) as u64) as u32;
    let content_y = page_y.min(VANILLA_GRADIENT_BORDER_CONTENT_HEIGHT - 1);
    let src_y = ((content_y as u64 * source_height as u64)
        / VANILLA_GRADIENT_BORDER_CONTENT_HEIGHT as u64)
        .min(source_height.saturating_sub(1) as u64) as u32;
    (src_x, src_y)
}

fn sample_sdf_at_source(
    source: &[u8],
    source_width: u32,
    source_height: u32,
    src_x: u32,
    src_y: u32,
) -> u8 {
    if source_width == 0 || source_height == 0 {
        return 255;
    }
    let expected = source_width as usize * source_height as usize;
    if source.len() != expected {
        return 255;
    }
    source[(src_y * source_width + src_x) as usize]
}

fn country_color_at_source(
    bank: GradientBorderLayerBank<'_>,
    source_width: u32,
    source_height: u32,
    src_x: u32,
    src_y: u32,
) -> Option<[u8; 3]> {
    let controller = controller_at_source(bank, source_width, source_height, src_x, src_y)?;
    if controller.is_none() {
        return None;
    }
    bank.country_colors.get(controller.raw() as usize).copied()
}

fn has_owned_land_controller(
    bank: GradientBorderLayerBank<'_>,
    source_width: u32,
    source_height: u32,
    src_x: u32,
    src_y: u32,
) -> bool {
    controller_at_source(bank, source_width, source_height, src_x, src_y).is_some()
}

fn controller_at_source(
    bank: GradientBorderLayerBank<'_>,
    source_width: u32,
    source_height: u32,
    src_x: u32,
    src_y: u32,
) -> Option<CountryId> {
    if source_width == 0 || source_height == 0 {
        return None;
    }
    let expected = source_width as usize * source_height as usize;
    if bank.province_pixels.len() != expected {
        return None;
    }
    let idx = (src_y * source_width + src_x) as usize;
    let province_id = *bank.province_pixels.get(idx)?;
    let def = bank
        .province_definitions
        .get(province_id as usize)
        .and_then(|def| def.as_ref())?;
    if def.province_type != ProvinceType::Land {
        return None;
    }
    let controller = bank.controllers.get(province_id as usize).copied()?;
    if controller.is_none() {
        None
    } else {
        Some(controller)
    }
}

fn write_bgra_pixel(out: &mut [u8], offset: usize, r: u8, g: u8, b: u8, a: u8) {
    out[offset] = b;
    out[offset + 1] = g;
    out[offset + 2] = r;
    out[offset + 3] = a;
}

fn neutral_channel3() -> Vec<u8> {
    vec![255, 255, 255, 255, 255, 255, 255, 255]
}

#[cfg(test)]
mod tests {
    use super::*;
    use hoi4_map::ProvinceDefinition;

    #[test]
    fn packed_gradient_channel_matches_vanilla_dimensions_and_country_color() {
        let definitions = test_definitions();
        let bank = test_bank(&definitions);
        let data = pack_channel1_country_color_distance(
            bank,
            2,
            2,
            GradientBorderAnchors { page0: 5, page1: 0 },
        );
        assert_eq!(
            data.len(),
            (VANILLA_GRADIENT_BORDER_WIDTH * VANILLA_GRADIENT_BORDER_HEIGHT * 4) as usize
        );
        assert_eq!(logical_rgba_at(&data, 0, 0), [20, 80, 160, 255]);
        assert_eq!(
            logical_rgba_at(&data, VANILLA_GRADIENT_BORDER_WIDTH - 1, 0),
            [180, 40, 20, 191]
        );
    }

    #[test]
    fn disabled_anchor_writes_transparent_page() {
        let definitions = test_definitions();
        let bank = test_bank(&definitions);
        let data = pack_channel1_country_color_distance(
            bank,
            2,
            2,
            GradientBorderAnchors {
                page0: -1,
                page1: -1,
            },
        );
        assert!(data.iter().all(|&v| v == 0));
    }

    #[test]
    fn channel2_gate_is_land_owned_only_and_fx_defaults_zero() {
        let definitions = test_definitions();
        let bank = test_bank(&definitions);
        let data = pack_channel2_gate_and_fx(bank, 2, 2);
        assert_eq!(logical_rgba_at(&data, 0, 0), [0, 255, 0, 255]);
        assert_eq!(
            logical_rgba_at(&data, 0, VANILLA_GRADIENT_BORDER_PAGE_HEIGHT - 1),
            [0, 0, 0, 0]
        );
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

    fn test_definitions() -> Vec<Option<ProvinceDefinition>> {
        let mut definitions = vec![None; 4];
        definitions[1] = Some(ProvinceDefinition {
            id: 1,
            r: 1,
            g: 0,
            b: 0,
            province_type: ProvinceType::Land,
            coastal: false,
            terrain: "plains".to_owned(),
            continent: 1,
        });
        definitions[2] = Some(ProvinceDefinition {
            id: 2,
            r: 2,
            g: 0,
            b: 0,
            province_type: ProvinceType::Land,
            coastal: false,
            terrain: "plains".to_owned(),
            continent: 1,
        });
        definitions[3] = Some(ProvinceDefinition {
            id: 3,
            r: 3,
            g: 0,
            b: 0,
            province_type: ProvinceType::Sea,
            coastal: false,
            terrain: "ocean".to_owned(),
            continent: 0,
        });
        definitions
    }

    fn test_bank<'a>(
        province_definitions: &'a [Option<ProvinceDefinition>],
    ) -> GradientBorderLayerBank<'a> {
        GradientBorderLayerBank {
            province_pixels: &[1, 2, 3, 1],
            province_definitions,
            controllers: &[CountryId::NONE, CountryId(0), CountryId(1), CountryId::NONE],
            country_colors: &[[20, 80, 160], [180, 40, 20]],
            country_sdf: &[0, 64, 128, 255],
            province_sdf: &[255, 128, 64, 0],
            coast_sdf: &[],
        }
    }

    fn logical_rgba_at(data: &[u8], x: u32, y: u32) -> [u8; 4] {
        let o = ((y * VANILLA_GRADIENT_BORDER_WIDTH + x) * 4) as usize;
        [data[o + 2], data[o + 1], data[o], data[o + 3]]
    }
}
