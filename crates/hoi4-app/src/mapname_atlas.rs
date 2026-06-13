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

use std::collections::HashMap;
use std::path::Path;

use fontdue::{Font, FontSettings};
use hoi4_assets::TgaImage;
use hoi4_paths::PathConfig;

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
    /// True for CJK/localised names that should use less aggressive
    /// country-axis rotation and sizing than long Latin map names.
    pub contains_non_ascii: bool,
}

/// Baked atlas + per-country lookup.
pub struct CountryNameAtlas {
    pub width: u32,
    pub height: u32,
    /// R8Unorm raw bytes, row-major top-down.
    pub data: Vec<u8>,
    /// `entries[country_idx]` — `Some` if the name was successfully baked.
    pub entries: Vec<Option<AtlasEntry>>,
    /// Diagnostic only. Keeps the license boundary explicit: runtime reads the
    /// user's local vanilla font, but no font asset is copied into the repo.
    pub font_source: String,
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
    bake_country_name_atlas_with_paths(names, font_size, None)
}

pub fn bake_country_name_atlas_with_paths(
    names: &[Option<String>],
    font_size: f32,
    path_cfg: Option<&PathConfig>,
) -> Option<CountryNameAtlas> {
    let prefer_cjk_font = names_contain_non_ascii(names);
    if !prefer_cjk_font {
        if let Some(path_cfg) = path_cfg {
            if let Some(atlas) = bake_vanilla_bmfont_country_name_atlas(names, path_cfg) {
                return Some(atlas);
            }
        }
    }
    bake_system_font_country_name_atlas(names, font_size, prefer_cjk_font)
}

fn names_contain_non_ascii(names: &[Option<String>]) -> bool {
    names
        .iter()
        .flatten()
        .any(|name| name.chars().any(|ch| !ch.is_ascii()))
}

fn bake_system_font_country_name_atlas(
    names: &[Option<String>],
    font_size: f32,
    prefer_cjk_font: bool,
) -> Option<CountryNameAtlas> {
    let (font_bytes, font_source) = load_system_font_bytes(prefer_cjk_font)?;
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
    // CJK names use a larger system font than the vanilla Garamond BMFont.
    // A 1024x2048 atlas silently overflowed: later country entries had UVs
    // but no pixels, so major names like GER/FRA/SOV/ENG/USA disappeared.
    let atlas_width = if prefer_cjk_font { 4096u32 } else { 2048u32 };
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
    let atlas_height = (cursor_y + line_h + 2)
        .next_power_of_two()
        .max(64)
        .min(8192);

    let mut data = vec![0u8; (atlas_width * atlas_height) as usize];

    // ─── Pass 3: blit each strip into atlas ──────────────────────────────
    let mut entries = vec![None; names.len()];
    for s in &strips {
        let Some((dst_x, dst_y)) = entries_pos[s.country_idx] else {
            continue;
        };
        if dst_y + line_h > atlas_height || dst_x + s.width > atlas_width {
            continue;
        }
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
            contains_non_ascii: names[s.country_idx]
                .as_ref()
                .is_some_and(|name| name.chars().any(|ch| !ch.is_ascii())),
        });
    }

    Some(CountryNameAtlas {
        width: atlas_width,
        height: atlas_height,
        data,
        entries,
        font_source,
    })
}

#[derive(Debug, Clone, Copy)]
struct BitmapGlyph {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    xoffset: i32,
    yoffset: i32,
    xadvance: i32,
}

#[derive(Debug, Clone)]
struct BitmapFont {
    source_name: String,
    line_height: u32,
    base: i32,
    glyphs: HashMap<char, BitmapGlyph>,
    sheet_width: u32,
    sheet_height: u32,
    coverage: Vec<u8>,
}

fn bake_vanilla_bmfont_country_name_atlas(
    names: &[Option<String>],
    path_cfg: &PathConfig,
) -> Option<CountryNameAtlas> {
    let font = load_vanilla_map_bmfont(path_cfg)?;
    let line_h = (font.line_height + 6).max(16);
    let pad = 3u32;

    struct Strip {
        country_idx: usize,
        width: u32,
        bitmap: Vec<u8>,
    }

    let mut strips = Vec::new();
    for (idx, name) in names.iter().enumerate() {
        let Some(name) = name else { continue };
        if name.trim().is_empty() {
            continue;
        }
        if let Some((width, bitmap)) = render_bmfont_strip(&font, name, line_h, pad) {
            strips.push(Strip {
                country_idx: idx,
                width,
                bitmap,
            });
        }
    }

    if strips.is_empty() {
        return None;
    }

    let atlas_width = 1024u32;
    let mut entries_pos: Vec<Option<(u32, u32)>> = vec![None; names.len()];
    let mut cursor_x = 0u32;
    let mut cursor_y = 0u32;
    for s in &strips {
        if cursor_x > 0 && cursor_x + s.width + 2 > atlas_width {
            cursor_x = 0;
            cursor_y += line_h + 2;
        }
        if s.width + 2 > atlas_width {
            continue;
        }
        entries_pos[s.country_idx] = Some((cursor_x, cursor_y));
        cursor_x += s.width + 2;
    }

    let atlas_height = (cursor_y + line_h + 2).next_power_of_two().max(64);
    let mut data = vec![0u8; (atlas_width * atlas_height) as usize];
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
            uv_min: [dst_x as f32 / aw, dst_y as f32 / ah],
            uv_max: [(dst_x + s.width) as f32 / aw, (dst_y + line_h) as f32 / ah],
            width_px: s.width,
            height_px: line_h,
            contains_non_ascii: names[s.country_idx]
                .as_ref()
                .is_some_and(|name| name.chars().any(|ch| !ch.is_ascii())),
        });
    }

    Some(CountryNameAtlas {
        width: atlas_width,
        height: atlas_height,
        data,
        entries,
        font_source: font.source_name,
    })
}

fn load_vanilla_map_bmfont(path_cfg: &PathConfig) -> Option<BitmapFont> {
    for rel in [
        "gfx/fonts/garamond_24.fnt",
        "gfx/fonts/garamond_16_bold.fnt",
        "gfx/fonts/garamond_16.fnt",
        "gfx/fonts/garamond_14_bold.fnt",
    ] {
        if let Some(fnt_path) = path_cfg.find(rel) {
            if let Some(font) = load_bmfont_from_fnt_path(&fnt_path) {
                return Some(font);
            }
        }
    }
    None
}

fn load_bmfont_from_fnt_path(fnt_path: &Path) -> Option<BitmapFont> {
    let fnt_text = std::fs::read_to_string(fnt_path).ok()?;
    let mut line_height = 0u32;
    let mut base = 0i32;
    let mut scale_w = 0u32;
    let mut scale_h = 0u32;
    let mut glyphs = HashMap::new();

    for line in fnt_text.lines() {
        let line = line.trim();
        if line.starts_with("common ") {
            line_height = parse_attr_u32(line, "lineHeight").unwrap_or(line_height);
            base = parse_attr_i32(line, "base").unwrap_or(base);
            scale_w = parse_attr_u32(line, "scaleW").unwrap_or(scale_w);
            scale_h = parse_attr_u32(line, "scaleH").unwrap_or(scale_h);
        } else if line.starts_with("char ") {
            let id = parse_attr_u32(line, "id")?;
            let ch = char::from_u32(id)?;
            let glyph = BitmapGlyph {
                x: parse_attr_u32(line, "x")?,
                y: parse_attr_u32(line, "y")?,
                width: parse_attr_u32(line, "width")?,
                height: parse_attr_u32(line, "height")?,
                xoffset: parse_attr_i32(line, "xoffset")?,
                yoffset: parse_attr_i32(line, "yoffset")?,
                xadvance: parse_attr_i32(line, "xadvance")?,
            };
            glyphs.insert(ch, glyph);
        }
    }

    if line_height == 0 || scale_w == 0 || scale_h == 0 || glyphs.is_empty() {
        return None;
    }

    let tga_path = fnt_path.with_extension("tga");
    let tga_bytes = std::fs::read(&tga_path).ok()?;
    let image = TgaImage::parse(&tga_bytes).ok()?;
    if image.width != scale_w || image.height != scale_h {
        return None;
    }

    let uses_alpha = image.pixels.chunks_exact(4).any(|px| px[3] < 250);
    let mut coverage = vec![0u8; (image.width * image.height) as usize];
    for (idx, px) in image.pixels.chunks_exact(4).enumerate() {
        let rgb = px[0].max(px[1]).max(px[2]);
        coverage[idx] = if uses_alpha { px[3] } else { rgb };
    }

    Some(BitmapFont {
        source_name: format!("vanilla-bmfont:{}", fnt_path.display()),
        line_height,
        base,
        glyphs,
        sheet_width: image.width,
        sheet_height: image.height,
        coverage,
    })
}

fn parse_attr_u32(line: &str, key: &str) -> Option<u32> {
    parse_attr_value(line, key)?.parse().ok()
}

fn parse_attr_i32(line: &str, key: &str) -> Option<i32> {
    parse_attr_value(line, key)?.parse().ok()
}

fn parse_attr_value<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    let prefix = format!("{key}=");
    line.split_whitespace()
        .find_map(|token| token.strip_prefix(&prefix))
        .map(|value| value.trim_matches('"'))
}

fn render_bmfont_strip(
    font: &BitmapFont,
    text: &str,
    line_h: u32,
    pad: u32,
) -> Option<(u32, Vec<u8>)> {
    let tracking = (font.line_height as f32 * 0.20).round() as i32;
    let mut total_w = (pad * 2) as i32;
    let mut visible = false;
    let chars: Vec<char> = text.chars().collect();
    for (idx, ch) in chars.iter().enumerate() {
        let glyph = font.glyphs.get(ch).or_else(|| {
            let upper = ch.to_ascii_uppercase();
            font.glyphs.get(&upper)
        });
        if let Some(g) = glyph {
            total_w += g.xadvance.max(g.width as i32);
            if idx + 1 < chars.len() && !ch.is_whitespace() {
                total_w += tracking;
            }
            visible |= g.width > 0 && g.height > 0;
        }
    }
    if !visible || total_w <= 0 {
        return None;
    }

    let width = (total_w as u32).max(8);
    let mut fill = vec![0u8; (width * line_h) as usize];
    let mut cursor_x = pad as i32;
    for (idx, ch) in chars.iter().enumerate() {
        let glyph = font.glyphs.get(ch).or_else(|| {
            let upper = ch.to_ascii_uppercase();
            font.glyphs.get(&upper)
        });
        let Some(g) = glyph else {
            continue;
        };
        let dst_x = cursor_x + g.xoffset;
        let dst_y = pad as i32 + g.yoffset + font.base.saturating_sub(font.line_height as i32);
        blit_bitmap_glyph(font, g, dst_x, dst_y, width, line_h, &mut fill);
        cursor_x += g.xadvance.max(g.width as i32);
        if idx + 1 < chars.len() && !ch.is_whitespace() {
            cursor_x += tracking;
        }
    }

    let bitmap = encode_fill_with_outline(&fill, width, line_h, 2);
    Some((width, bitmap))
}

fn blit_bitmap_glyph(
    font: &BitmapFont,
    glyph: &BitmapGlyph,
    dst_x: i32,
    dst_y: i32,
    dst_width: u32,
    dst_height: u32,
    fill: &mut [u8],
) {
    for gy in 0..glyph.height {
        for gx in 0..glyph.width {
            let sx = glyph.x + gx;
            let sy = glyph.y + gy;
            if sx >= font.sheet_width || sy >= font.sheet_height {
                continue;
            }
            let dx = dst_x + gx as i32;
            let dy = dst_y + gy as i32;
            if dx < 0 || dy < 0 || dx >= dst_width as i32 || dy >= dst_height as i32 {
                continue;
            }
            let src = (sy * font.sheet_width + sx) as usize;
            let dst = (dy as u32 * dst_width + dx as u32) as usize;
            let cov = font.coverage[src];
            if cov > fill[dst] {
                fill[dst] = cov;
            }
        }
    }
}

fn encode_fill_with_outline(fill: &[u8], width: u32, height: u32, outline_radius: i32) -> Vec<u8> {
    let mut out = vec![0u8; (width * height) as usize];
    for y in 0..height as i32 {
        for x in 0..width as i32 {
            let idx = (y * width as i32 + x) as usize;
            if fill[idx] >= 32 {
                out[idx] = (150u16 + (fill[idx] as u16 / 2)).min(255) as u8;
                continue;
            }

            let mut max_neighbour = 0u8;
            for dy in -outline_radius..=outline_radius {
                for dx in -outline_radius..=outline_radius {
                    if dx == 0 && dy == 0 {
                        continue;
                    }
                    let nx = x + dx;
                    let ny = y + dy;
                    if nx < 0 || ny < 0 || nx >= width as i32 || ny >= height as i32 {
                        continue;
                    }
                    let nidx = (ny * width as i32 + nx) as usize;
                    max_neighbour = max_neighbour.max(fill[nidx]);
                }
            }
            if max_neighbour >= 48 {
                out[idx] = 105;
            }
        }
    }
    out
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
    let tracking = (font_size * 0.18).round() as i32;
    let chars: Vec<char> = text.chars().collect();
    for (idx, ch) in chars.iter().enumerate() {
        let m = font.metrics(*ch, font_size);
        total_w += m.advance_width as i32;
        if idx + 1 < chars.len() && !ch.is_whitespace() {
            total_w += tracking;
        }
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
    for (idx, ch) in chars.iter().enumerate() {
        let (m, bm) = font.rasterize(*ch, font_size);
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
        if idx + 1 < chars.len() && !ch.is_whitespace() {
            x_cursor += tracking;
        }
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
fn load_system_font_bytes(prefer_cjk_font: bool) -> Option<(Vec<u8>, String)> {
    let cjk_candidates = [
        r"C:\Windows\Fonts\msyh.ttc",
        r"C:\Windows\Fonts\msyhbd.ttc",
        r"C:\Windows\Fonts\simsun.ttc",
        r"C:\Windows\Fonts\simhei.ttf",
        r"C:\Windows\Fonts\Deng.ttf",
        r"C:\Windows\Fonts\Dengb.ttf",
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/opentype/noto/NotoSerifCJK-Regular.ttc",
        "/usr/share/fonts/truetype/wqy/wqy-microhei.ttc",
        "/usr/share/fonts/truetype/wqy/wqy-zenhei.ttc",
        "/System/Library/Fonts/PingFang.ttc",
        "/System/Library/Fonts/STHeiti Light.ttc",
    ];
    let serif_candidates = [
        r"C:\Windows\Fonts\georgiab.ttf",
        r"C:\Windows\Fonts\georgia.ttf",
        r"C:\Windows\Fonts\timesbd.ttf",
        r"C:\Windows\Fonts\times.ttf",
        r"C:\Windows\Fonts\BOOKOSB.TTF",
        r"C:\Windows\Fonts\BOOKOS.TTF",
        r"C:\Windows\Fonts\cambria.ttc",
        r"C:\Windows\Fonts\consola.ttf",
        r"C:\Windows\Fonts\segoeui.ttf",
        r"C:\Windows\Fonts\arial.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "/usr/share/fonts/noto/NotoSans-Regular.ttf",
        "/Library/Fonts/Arial.ttf",
        "/System/Library/Fonts/Helvetica.ttc",
    ];

    if prefer_cjk_font {
        for path in cjk_candidates {
            if let Ok(b) = std::fs::read(path) {
                return Some((b, format!("system-cjk-fontdue:{path}")));
            }
        }
    }
    for path in serif_candidates {
        if let Ok(b) = std::fs::read(path) {
            return Some((b, format!("system-serif-fontdue:{path}")));
        }
    }
    if !prefer_cjk_font {
        for path in cjk_candidates {
            if let Ok(b) = std::fs::read(path) {
                return Some((b, format!("system-cjk-fontdue:{path}")));
            }
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
        let is_cjk = entry.contains_non_ascii;
        let shape_ratio = world_half_1 / world_half_2.max(world_scale * 0.5);
        let mut axis1 = [obb.axis1_dir.0, obb.axis1_dir.1];

        // CJK country names are visually compact. On broad countries the raw
        // PCA axis often follows a coastline diagonal, which makes labels look
        // off-center instead of like map text. Keep thin countries aligned to
        // their long axis, but use a horizontal baseline for broad shapes.
        if is_cjk && shape_ratio < 2.8 {
            axis1 = [1.0, 0.0];
        }

        // Pick the rendered quad size:
        // 1. Base width = 35% of country major axis half-extent (so the
        //    label sits inside core territory and never bleeds into
        //    neighbours). Earlier 75% was way too aggressive — for big
        //    countries like the USSR it produced a label spanning almost
        //    the entire country.
        // 2. Hard cap by the country's minor axis: rendered_height ≤
        //    0.6 × half_extent_2.
        // 3. Maintain text aspect — shrink width if minor-axis cap kicks in.
        // 4. Absolute world-unit caps. Phase 5 renders full country names,
        //    so the old tag-sized cap made majors look like tiny province
        //    labels. These are half-extents: total max label width is ~15w.
        let base_width_factor = if is_cjk { 0.34 } else { 0.46 };
        let min_width = if is_cjk {
            world_scale * 4.5
        } else {
            world_scale * 6.0
        };
        let minor_height_factor = if is_cjk { 0.50 } else { 0.72 };
        let max_width_world = if is_cjk { 5.4 } else { 7.5 };
        let max_height_world = if is_cjk { 0.62 } else { 1.05 };

        let mut width = (world_half_1 * base_width_factor).max(min_width);
        let mut height = width / aspect.max(0.05);
        let max_height = (world_half_2 * minor_height_factor).max(world_scale * 1.1);
        if height > max_height {
            height = max_height;
            width = height * aspect;
        }
        if width > max_width_world {
            width = max_width_world;
            height = width / aspect.max(0.05);
        }
        if height > max_height_world {
            height = max_height_world;
            width = height * aspect;
        }

        out.push(CountryNameInstance {
            center: [cx_world, label_y, cz_world],
            width_world: width,
            axis1,
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
            contains_non_ascii: false,
        }
    }

    fn dummy_cjk_atlas_entry(width_px: u32, height_px: u32, ux: f32, uy: f32) -> AtlasEntry {
        AtlasEntry {
            contains_non_ascii: true,
            ..dummy_atlas_entry(width_px, height_px, ux, uy)
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
            font_source: "test".to_owned(),
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
            font_source: "test".to_owned(),
            entries: vec![Some(dummy_atlas_entry(100, 80, 0.0, 0.0))],
        };
        let instances = build_label_instances(&obbs, &atlas, 0.02, 1.0);
        let inst = &instances[0];
        // half_extent_2 = 5 px -> world 0.1; minor cap = 0.72 x 0.1 = 0.072.
        assert!(
            inst.height_world <= 0.073,
            "height should be capped by minor axis, got {}",
            inst.height_world
        );
    }

    #[test]
    fn bmfont_attr_parser_reads_signed_values() {
        let line =
            "char id=65 x=72 y=96 width=16 height=15 xoffset=-2 yoffset=4 xadvance=14 page=0";
        assert_eq!(parse_attr_u32(line, "id"), Some(65));
        assert_eq!(parse_attr_i32(line, "xoffset"), Some(-2));
        assert_eq!(parse_attr_u32(line, "width"), Some(16));
    }

    #[test]
    fn non_ascii_country_names_prefer_system_font_path() {
        let names = vec![
            Some("德意志国".to_owned()),
            Some("UNITED KINGDOM".to_owned()),
        ];
        assert!(names_contain_non_ascii(&names));
    }

    #[test]
    fn cjk_broad_country_label_uses_horizontal_baseline() {
        let obbs = vec![Some(dummy_obb(0.0, 0.0, 0.65, 0.76, 80.0, 45.0))];
        let atlas = CountryNameAtlas {
            width: 1024,
            height: 256,
            data: vec![0; 1024 * 256],
            font_source: "test".to_owned(),
            entries: vec![Some(dummy_cjk_atlas_entry(220, 64, 0.0, 0.0))],
        };
        let instances = build_label_instances(&obbs, &atlas, 0.02, 1.0);
        assert_eq!(instances[0].axis1, [1.0, 0.0]);
    }

    #[test]
    fn ascii_country_label_preserves_pca_baseline() {
        let obbs = vec![Some(dummy_obb(0.0, 0.0, 0.65, 0.76, 80.0, 45.0))];
        let atlas = CountryNameAtlas {
            width: 1024,
            height: 256,
            data: vec![0; 1024 * 256],
            font_source: "test".to_owned(),
            entries: vec![Some(dummy_atlas_entry(360, 64, 0.0, 0.0))],
        };
        let instances = build_label_instances(&obbs, &atlas, 0.02, 1.0);
        assert!((instances[0].axis1[0] - 0.65).abs() < 1e-5);
        assert!((instances[0].axis1[1] - 0.76).abs() < 1e-5);
    }
}
