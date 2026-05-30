use fontdue::{Font, FontSettings};
use hoi4_render::mapname_3d::CountryNameInstance;
use hoi4_render::province_labels::ProvinceLabel;

#[derive(Debug, Clone, Copy)]
pub struct ProvinceAtlasEntry {
    pub uv_min: [f32; 2],
    pub uv_max: [f32; 2],
    pub width_px: u32,
    pub height_px: u32,
}

pub struct ProvinceNameAtlas {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
    pub entries: Vec<Option<ProvinceAtlasEntry>>,
}

impl ProvinceNameAtlas {
    pub fn count_baked(&self) -> usize {
        self.entries.iter().filter(|e| e.is_some()).count()
    }
}

struct GlyphCache {
    font: Font,
    cache: std::collections::HashMap<char, (fontdue::Metrics, Vec<u8>)>,
}

impl GlyphCache {
    fn new(font_bytes: &[u8]) -> Option<Self> {
        let font = Font::from_bytes(font_bytes, FontSettings::default()).ok()?;
        Some(Self {
            font,
            cache: std::collections::HashMap::new(),
        })
    }

    fn get_glyph(&mut self, ch: char, size: f32) -> &(fontdue::Metrics, Vec<u8>) {
        if !self.cache.contains_key(&ch) {
            let (m, bm) = self.font.rasterize(ch, size);
            self.cache.insert(ch, (m, bm));
        }
        self.cache.get(&ch).unwrap()
    }
}

pub fn bake_province_name_atlas(
    names: &[Option<String>],
    font_size: f32,
) -> Option<ProvinceNameAtlas> {
    let font_bytes = load_system_font_bytes()?;
    let mut glyph_cache = GlyphCache::new(&font_bytes)?;

    let metrics = glyph_cache.font.horizontal_line_metrics(font_size)?;
    let asc = metrics.ascent.ceil() as i32;
    let desc = (-metrics.descent).ceil() as i32;
    let line_h = (asc + desc + 2) as u32;
    let pad = 1u32;

    struct Strip {
        province_idx: usize,
        width: u32,
        bitmap: Vec<u8>,
    }
    let mut strips: Vec<Strip> = Vec::new();

    for (idx, name) in names.iter().enumerate() {
        let Some(name) = name else { continue };
        if name.is_empty() {
            continue;
        }
        let strip = render_strip_cached(&mut glyph_cache, name, font_size, line_h, asc, pad);
        if let Some(s) = strip {
            strips.push(Strip {
                province_idx: idx,
                width: s.0,
                bitmap: s.1,
            });
        }
    }

    if strips.is_empty() {
        return None;
    }

    let atlas_width = 2048u32;
    let mut entries_pos: Vec<Option<(u32, u32)>> = vec![None; names.len()];
    let mut cursor_x = 0u32;
    let mut cursor_y = 0u32;
    for s in &strips {
        if cursor_x + s.width + 2 > atlas_width {
            cursor_x = 0;
            cursor_y += line_h + 2;
        }
        entries_pos[s.province_idx] = Some((cursor_x, cursor_y));
        cursor_x += s.width + 2;
    }
    let atlas_height = (cursor_y + line_h + 2).next_power_of_two().min(4096);

    let mut data = vec![0u8; (atlas_width * atlas_height) as usize];
    let mut entries = vec![None; names.len()];

    for s in &strips {
        let Some((dst_x, dst_y)) = entries_pos[s.province_idx] else {
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
        entries[s.province_idx] = Some(ProvinceAtlasEntry {
            uv_min: [dst_x as f32 / aw, dst_y as f32 / ah],
            uv_max: [(dst_x + s.width) as f32 / aw, (dst_y + line_h) as f32 / ah],
            width_px: s.width,
            height_px: line_h,
        });
    }

    Some(ProvinceNameAtlas {
        width: atlas_width,
        height: atlas_height,
        data,
        entries,
    })
}

fn render_strip_cached(
    glyph_cache: &mut GlyphCache,
    text: &str,
    font_size: f32,
    line_h: u32,
    asc: i32,
    pad: u32,
) -> Option<(u32, Vec<u8>)> {
    let mut total_w = 0i32;
    for ch in text.chars() {
        let (m, _) = glyph_cache.get_glyph(ch, font_size);
        total_w += m.advance_width as i32;
    }
    if total_w <= 0 {
        return None;
    }
    let width = (total_w as u32 + pad * 2).max(8);
    let mut fill = vec![0u8; (width * line_h) as usize];
    let mut x_cursor = pad as i32;

    for ch in text.chars() {
        let (m, bm) = {
            let (metrics, bitmap) = glyph_cache.get_glyph(ch, font_size);
            (metrics.clone(), bitmap.clone())
        };
        if m.width > 0 && m.height > 0 {
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

    let mut buf = vec![0u8; (width * line_h) as usize];
    let outline_radius = 1i32;
    for y in 0..line_h as i32 {
        for x in 0..width as i32 {
            let idx = (y * width as i32 + x) as usize;
            if fill[idx] >= 128 {
                buf[idx] = 255;
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
                buf[idx] = 110;
            }
        }
    }

    Some((width, buf))
}

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

pub fn build_province_label_instances(
    labels: &[Option<ProvinceLabel>],
    atlas: &ProvinceNameAtlas,
    world_scale: f32,
    label_y: f32,
    min_pixel_count: u32,
) -> Vec<CountryNameInstance> {
    let n = labels.len().min(atlas.entries.len());
    let mut out: Vec<CountryNameInstance> = Vec::new();
    for id in 1..n {
        let Some(label) = labels[id] else { continue };
        let Some(entry) = atlas.entries[id] else {
            continue;
        };
        if label.pixel_count < min_pixel_count {
            continue;
        }

        let cx_world = label.centroid_px.0 * world_scale;
        let cz_world = label.centroid_px.1 * world_scale;
        let world_half_1 = label.half_extent_1 * world_scale;
        let world_half_2 = label.half_extent_2 * world_scale;

        let aspect = entry.width_px as f32 / entry.height_px.max(1) as f32;

        const MAX_WIDTH_WORLD: f32 = 2.5;
        const MAX_HEIGHT_WORLD: f32 = 0.4;

        let mut width = (world_half_1 * 0.45).max(world_scale * 3.0);
        let mut height = width / aspect.max(0.05);
        let max_height = (world_half_2 * 0.7).max(world_scale * 0.8);
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
            axis1: [label.axis1_dir.0, label.axis1_dir.1],
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

    #[test]
    fn build_province_label_instances_filters_small() {
        use hoi4_render::province_labels::ProvinceLabel;
        let labels = vec![
            None,
            Some(ProvinceLabel {
                province_id: 1,
                centroid_px: (100.0, 50.0),
                axis1_dir: (1.0, 0.0),
                half_extent_1: 20.0,
                half_extent_2: 10.0,
                pixel_count: 5000,
                is_land: true,
            }),
            Some(ProvinceLabel {
                province_id: 2,
                centroid_px: (200.0, 100.0),
                axis1_dir: (0.7, 0.7),
                half_extent_1: 5.0,
                half_extent_2: 3.0,
                pixel_count: 50,
                is_land: true,
            }),
        ];
        let atlas = ProvinceNameAtlas {
            width: 2048,
            height: 512,
            data: vec![0; 2048 * 512],
            entries: vec![
                None,
                Some(ProvinceAtlasEntry {
                    uv_min: [0.0, 0.0],
                    uv_max: [0.05, 0.02],
                    width_px: 100,
                    height_px: 20,
                }),
                Some(ProvinceAtlasEntry {
                    uv_min: [0.1, 0.0],
                    uv_max: [0.12, 0.02],
                    width_px: 40,
                    height_px: 20,
                }),
            ],
        };
        let instances = build_province_label_instances(&labels, &atlas, 0.02, 2.0, 100);
        assert_eq!(
            instances.len(),
            1,
            "only province 1 passes min_pixel_count=100"
        );
    }
}
