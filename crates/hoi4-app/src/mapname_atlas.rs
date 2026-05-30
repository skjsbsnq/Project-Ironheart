//! Phase 3.10.3 — country-name atlas baker.
//!
//! Renders one strip per country onto a single R8 atlas texture, with each
//! glyph carrying a 1-pixel black outline halo. The encoding is two-tier:
//!
//! | Atlas pixel value | Meaning                          |
//! |-------------------|----------------------------------|
//! | `[0.00, 0.04]`    | transparent (discarded by shader)|
//! | `(0.04, 0.50]`    | outline ring (rendered black)    |
//! | `(0.50, 1.00]`    | text fill (rendered white)       |
//!
//! `mapname_3d.wgsl` decodes those tiers via `smoothstep`. The fontdue
//! rasteriser is shared with the regular HUD `text_pass`; the only thing
//! special about this baker is layout (one strip per country) and the
//! outline expansion pass.
//!
//! The atlas is **not** dynamically grown — we pre-size by computing every
//! country's strip width up front, then pack greedily into rows.

use fontdue::{Font, FontSettings};

use hoi4_render::mapname_3d::CountryNameInstance;

/// One country's location in the atlas.
#[derive(Debug, Clone, Copy)]
pub struct AtlasEntry {
    pub uv_min: [f32; 2],
    pub uv_max: [f32; 2],
    /// Strip width / height in atlas pixels (used for picking a sensible
    /// world-space quad aspect ratio that doesn't squash the text).
    pub width_px: u32,
    pub height_px: u32,
}

/// Baked atlas + per-country lookup.
pub struct CountryNameAtlas {
    pub width: u32,
    pub height: u32,
    /// R8Unorm raw bytes, row-major top-down.
    pub data: Vec<u8>,
    /// `entries[country_idx]` — `Some` if the name was successfully baked.
    pub entries: Vec<Option<AtlasEntry>>,
}

impl CountryNameAtlas {
    pub fn count_baked(&self) -> usize {
        self.entries.iter().filter(|e| e.is_some()).count()
    }
}

/// Bake every supplied country name into a single atlas. `names[idx]` is
/// the displayable text for country `idx` (typically the 3-letter tag, or
/// later a localised full name); `None` skips that country.
///
/// `font_size` is the rasterisation height in atlas pixels. 64 px gives a
/// crisp result that scales down nicely; larger means more atlas memory,
/// smaller means visible aliasing on screen.
///
/// Returns `None` if no usable system font could be loaded.
pub fn bake_country_name_atlas(
    names: &[Option<String>],
    font_size: f32,
) -> Option<CountryNameAtlas> {
    let font_bytes = load_system_font_bytes()?;
    let font = Font::from_bytes(font_bytes.as_slice(), FontSettings::default()).ok()?;

    // Glyph height is the font's ascent + abs(descent) — keep some padding
    // for the outline ring (1 px on each side).
    let metrics = font.horizontal_line_metrics(font_size)?;
    let asc = metrics.ascent.ceil() as i32;
    let desc = (-metrics.descent).ceil() as i32; // positive
    let line_h = (asc + desc + 2) as u32; // +2 for outline padding
    let pad = 1u32; // outline pad on each side of every strip

    // ─── Pass 1: rasterise each name → temp R8 buffer ─────────────────────
    struct Strip {
        country_idx: usize,
        width: u32,
        bitmap: Vec<u8>, // (line_h × width)
    }
    let mut strips: Vec<Strip> = Vec::new();

    for (idx, name) in names.iter().enumerate() {
        let Some(name) = name else { continue };
        if name.is_empty() {
            continue;
        }
        let strip = render_strip(&font, name, font_size, line_h, asc, pad);
        if let Some(s) = strip {
            strips.push(Strip {
                country_idx: idx,
                width: s.0,
                bitmap: s.1,
            });
        }
    }

    if strips.is_empty() {
        return None;
    }

    // ─── Pass 2: greedy row packing ──────────────────────────────────────
    let atlas_width = 1024u32;
    // Bin width: round each strip's width up to the next 4-byte boundary
    // (R8Unorm rows in wgpu need bytes_per_row ≥ width).
    let mut entries_pos: Vec<Option<(u32, u32)>> = vec![None; names.len()];
    let mut cursor_x = 0u32;
    let mut cursor_y = 0u32;
    for s in &strips {
        if cursor_x + s.width + 2 > atlas_width {
            cursor_x = 0;
            cursor_y += line_h + 2;
        }
        entries_pos[s.country_idx] = Some((cursor_x, cursor_y));
        cursor_x += s.width + 2;
    }
    let atlas_height = (cursor_y + line_h + 2).next_power_of_two().min(2048);

    let mut data = vec![0u8; (atlas_width * atlas_height) as usize];

    // ─── Pass 3: blit each strip into atlas ──────────────────────────────
    let mut entries = vec![None; names.len()];
    for s in &strips {
        let Some((dst_x, dst_y)) = entries_pos[s.country_idx] else {
            continue;
        };
        for row in 0..line_h {
            for col in 0..s.width {
                let src = (row * s.width + col) as usize;
                let dst = ((dst_y + row) * atlas_width + dst_x + col) as usize;
                if dst < data.len() {
                    data[dst] = s.bitmap[src];
                }
            }
        }
        let aw = atlas_width as f32;
        let ah = atlas_height as f32;
        entries[s.country_idx] = Some(AtlasEntry {
            uv_min: [(dst_x) as f32 / aw, (dst_y) as f32 / ah],
            uv_max: [(dst_x + s.width) as f32 / aw, (dst_y + line_h) as f32 / ah],
            width_px: s.width,
            height_px: line_h,
        });
    }

    Some(CountryNameAtlas {
        width: atlas_width,
        height: atlas_height,
        data,
        entries,
    })
}

/// Render a single text strip (one country) into a `(width, height)` byte
/// buffer with two-tier outline encoding. Returns `None` if the text
/// rasterised to nothing visible.
fn render_strip(
    font: &Font,
    text: &str,
    font_size: f32,
    line_h: u32,
    asc: i32,
    pad: u32,
) -> Option<(u32, Vec<u8>)> {
    // Pre-pass: walk glyphs to compute total advance width.
    let mut total_w = 0i32;
    for ch in text.chars() {
        let m = font.metrics(ch, font_size);
        total_w += m.advance_width as i32;
        // (Kerning is a polish concern — labels are 2-3 letters and the
        // country names we render are normalised tags; safe to skip.)
    }
    if total_w <= 0 {
        return None;
    }
    let width = (total_w as u32 + pad * 2).max(8);
    let mut buf = vec![0u8; (width * line_h) as usize];

    // ─── Pass A: rasterise filled glyphs to temp alpha buffer ────────────
    // `fill[idx] = u8 alpha` — used in pass B to expand the outline.
    let mut fill = vec![0u8; (width * line_h) as usize];
    let mut x_cursor = pad as i32;
    for ch in text.chars() {
        let (m, bm) = font.rasterize(ch, font_size);
        if m.width > 0 && m.height > 0 {
            // Glyph bitmap origin = (xmin, height + ymin) below baseline.
            let glyph_top = (asc - m.height as i32 - m.ymin) + pad as i32;
            let glyph_left = x_cursor + m.xmin;
            for r in 0..m.height as i32 {
                for c in 0..m.width as i32 {
                    let gx = glyph_left + c;
                    let gy = glyph_top + r;
                    if gx < 0 || gy < 0 || gx >= width as i32 || gy >= line_h as i32 {
                        continue;
                    }
                    let src = (r as u32 * m.width as u32 + c as u32) as usize;
                    let dst = (gy as u32 * width + gx as u32) as usize;
                    let a = bm[src];
                    if a > fill[dst] {
                        fill[dst] = a;
                    }
                }
            }
        }
        x_cursor += m.advance_width as i32;
    }

    // ─── Pass B: outline expansion ───────────────────────────────────────
    // For each pixel: if fill ≥ 128 → write 255 (text), else if any 8-
    // neighbour (within `outline_radius`) has fill ≥ 128 → write 100
    // (outline). Else 0.
    let outline_radius = 1i32;
    for y in 0..line_h as i32 {
        for x in 0..width as i32 {
            let idx = (y * width as i32 + x) as usize;
            if fill[idx] >= 128 {
                buf[idx] = 255;
                continue;
            }
            // Check neighbours.
            let mut max_neighbour = 0u8;
            for dy in -outline_radius..=outline_radius {
                for dx in -outline_radius..=outline_radius {
                    if dx == 0 && dy == 0 {
                        continue;
                    }
                    let nx = x + dx;
                    let ny = y + dy;
                    if nx < 0 || ny < 0 || nx >= width as i32 || ny >= line_h as i32 {
                        continue;
                    }
                    let nidx = (ny * width as i32 + nx) as usize;
                    let v = fill[nidx];
                    if v > max_neighbour {
                        max_neighbour = v;
                    }
                }
            }
            if max_neighbour >= 128 {
                // Outline pixel — leave room for shader smoothstep
                // (`v ∈ (0.04, 0.50]` → outline tier).
                buf[idx] = 110;
            }
        }
    }

    Some((width, buf))
}

/// Try common Windows / Linux system font paths in order.
fn load_system_font_bytes() -> Option<Vec<u8>> {
    let candidates = [
        r"C:\Windows\Fonts\msyh.ttc",
        r"C:\Windows\Fonts\segoeui.ttf",
        r"C:\Windows\Fonts\arial.ttf",
        r"C:\Windows\Fonts\consola.ttf",
        "/usr/share/fonts/truetype/wqy/wqy-microhei.ttc",
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "/usr/share/fonts/noto/NotoSans-Regular.ttf",
        "/Library/Fonts/Arial.ttf",
        "/System/Library/Fonts/Helvetica.ttc",
    ];
    for path in candidates {
        if let Ok(b) = std::fs::read(path) {
            return Some(b);
        }
    }
    None
}

/// Build the per-country instance buffer for the 3D label pass.
///
/// `obbs` — output of `mapname_3d::compute_country_obbs`, indexed by
/// country.
/// `atlas` — output of `bake_country_name_atlas` (per-country UV rect +
/// strip pixel size).
/// `world_scale`, `height_scale` — same constants used by the terrain mesh
/// pipeline.
/// `label_y` — fixed world-Y to lift labels above the heightmap.
///
/// Output is sparse: countries with no OBB **or** no atlas entry are
/// silently skipped. The vertex shader needs every entry to be non-zero
/// (otherwise it will draw a degenerate quad), so we filter them out here.
pub fn build_label_instances(
    obbs: &[Option<hoi4_render::mapname_3d::CountryObb>],
    atlas: &CountryNameAtlas,
    world_scale: f32,
    label_y: f32,
) -> Vec<CountryNameInstance> {
    let n = obbs.len().min(atlas.entries.len());
    let mut out: Vec<CountryNameInstance> = Vec::new();
    for idx in 0..n {
        let Some(obb) = obbs[idx] else { continue };
        let Some(entry) = atlas.entries[idx] else {
            continue;
        };

        // Convert pixel-space OBB to world-space.
        let cx_world = obb.centroid_px.0 * world_scale;
        let cz_world = obb.centroid_px.1 * world_scale;
        let world_half_1 = obb.half_extent_1 * world_scale;
        let world_half_2 = obb.half_extent_2 * world_scale;

        // Aspect of the baked text strip (W/H).
        let aspect = entry.width_px as f32 / entry.height_px.max(1) as f32;

        // Pick the rendered quad size:
        // 1. Base width = 35% of country major axis half-extent (so the
        //    label sits inside core territory and never bleeds into
        //    neighbours). Earlier 75% was way too aggressive — for big
        //    countries like the USSR it produced a label spanning almost
        //    the entire country.
        // 2. Hard cap by the country's minor axis: rendered_height ≤
        //    0.6 × half_extent_2.
        // 3. Maintain text aspect — shrink width if minor-axis cap kicks in.
        // 4. Absolute world-unit caps so even big countries don't end up
        //    with comically large labels: max width = 4.0 world units
        //    (~3.5% of map width), max height = 0.7 world units.
        const MAX_WIDTH_WORLD: f32 = 4.0;
        const MAX_HEIGHT_WORLD: f32 = 0.7;

        let mut width = (world_half_1 * 0.35).max(world_scale * 4.0);
        let mut height = width / aspect.max(0.05);
        let max_height = (world_half_2 * 0.6).max(world_scale * 1.0);
        if height > max_height {
            height = max_height;
            width = height * aspect;
        }
        if width > MAX_WIDTH_WORLD {
            width = MAX_WIDTH_WORLD;
            height = width / aspect.max(0.05);
        }
        if height > MAX_HEIGHT_WORLD {
            height = MAX_HEIGHT_WORLD;
            width = height * aspect;
        }

        out.push(CountryNameInstance {
            center: [cx_world, label_y, cz_world],
            width_world: width,
            axis1: [obb.axis1_dir.0, obb.axis1_dir.1],
            height_world: height,
            _pad0: 0.0,
            uv_min: entry.uv_min,
            uv_max: entry.uv_max,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use hoi4_render::mapname_3d::CountryObb;

    fn dummy_obb(cx: f32, cz: f32, ax: f32, ay: f32, h1: f32, h2: f32) -> CountryObb {
        CountryObb {
            centroid_px: (cx, cz),
            axis1_dir: (ax, ay),
            half_extent_1: h1,
            half_extent_2: h2,
            pixel_count: 100,
        }
    }

    fn dummy_atlas_entry(width_px: u32, height_px: u32, ux: f32, uy: f32) -> AtlasEntry {
        AtlasEntry {
            uv_min: [ux, uy],
            uv_max: [ux + 0.1, uy + 0.05],
            width_px,
            height_px,
        }
    }

    #[test]
    fn build_label_instances_filters_missing_entries() {
        let obbs = vec![
            Some(dummy_obb(100.0, 50.0, 1.0, 0.0, 30.0, 20.0)),
            Some(dummy_obb(200.0, 80.0, 0.7, 0.7, 25.0, 15.0)),
            None, // country 2 has no OBB → skip
        ];
        let atlas = CountryNameAtlas {
            width: 1024,
            height: 256,
            data: vec![0; 1024 * 256],
            entries: vec![
                Some(dummy_atlas_entry(120, 80, 0.0, 0.0)),
                None, // country 1 has no atlas entry → skip
                Some(dummy_atlas_entry(80, 80, 0.2, 0.0)),
            ],
        };
        let instances = build_label_instances(&obbs, &atlas, 0.02, 1.0);
        // Only country 0 has both → 1 instance.
        assert_eq!(instances.len(), 1);
        let inst = &instances[0];
        // World position = pixel × world_scale.
        assert!((inst.center[0] - 2.0).abs() < 1e-3);
        assert!((inst.center[1] - 1.0).abs() < 1e-3);
        assert!((inst.center[2] - 1.0).abs() < 1e-3);
        // Major axis carried through.
        assert_eq!(inst.axis1, [1.0, 0.0]);
        // Width >0, aspect respected.
        assert!(inst.width_world > 0.0 && inst.height_world > 0.0);
        let aspect = inst.width_world / inst.height_world;
        assert!(
            (aspect - 1.5).abs() < 0.5,
            "aspect should be ~120/80 = 1.5, got {aspect}"
        );
    }

    #[test]
    fn build_label_instances_height_capped_by_minor_axis() {
        // Long thin country — minor-axis cap should kick in.
        let obbs = vec![Some(dummy_obb(0.0, 0.0, 1.0, 0.0, 100.0, 5.0))];
        let atlas = CountryNameAtlas {
            width: 1024,
            height: 256,
            data: vec![0; 1024 * 256],
            entries: vec![Some(dummy_atlas_entry(100, 80, 0.0, 0.0))],
        };
        let instances = build_label_instances(&obbs, &atlas, 0.02, 1.0);
        let inst = &instances[0];
        // half_extent_2 = 5 px → world 0.1; minor cap = 0.6 × 0.1 = 0.06.
        // (Used to be 0.85 × — see Phase 3.10.3 size tuning.)
        assert!(
            inst.height_world <= 0.061,
            "height should be capped by minor axis, got {}",
            inst.height_world
        );
    }
}
