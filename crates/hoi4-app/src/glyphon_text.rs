//! Phase 3.8: fontdue-based font loading for text_pass.
//!
//! Provides `FontdueAtlas` — loads a system TTF font via fontdue, rasterizes
//! glyphs on demand into a dynamic R8 texture atlas, and exposes a glyph
//! metrics interface text_pass.rs consumes directly.

use fontdue::{Font, FontSettings};
use std::collections::HashMap;

/// A single rasterized glyph's position in the atlas.
#[derive(Debug, Clone, Copy)]
pub struct GlyphInfo {
    /// Position in atlas (pixels).
    pub atlas_x: u32,
    pub atlas_y: u32,
    /// Glyph bitmap size.
    pub width: u32,
    pub height: u32,
    /// Offset from cursor to top-left of bitmap.
    pub xoffset: f32,
    pub yoffset: f32,
    /// Horizontal advance after this glyph.
    pub xadvance: f32,
}

/// Dynamic glyph atlas built with fontdue.
pub struct FontdueAtlas {
    font: Font,
    font_size: f32,
    /// Atlas pixel data (R8, single channel).
    pub atlas_data: Vec<u8>,
    pub atlas_width: u32,
    pub atlas_height: u32,
    /// Current packing cursor.
    cursor_x: u32,
    cursor_y: u32,
    row_height: u32,
    /// Cached glyph info by character.
    pub glyphs: HashMap<char, GlyphInfo>,
    /// Line height (ascent + descent + gap).
    pub line_height: f32,
}

impl FontdueAtlas {
    /// Create a new atlas from a TTF/OTF font file.
    pub fn from_font_bytes(font_bytes: &[u8], size: f32) -> Option<Self> {
        let font = Font::from_bytes(font_bytes, FontSettings::default()).ok()?;

        let atlas_width = 2048u32;
        let atlas_height = 2048u32;
        let atlas_data = vec![0u8; (atlas_width * atlas_height) as usize];

        let metrics = font.horizontal_line_metrics(size)?;
        let line_height = metrics.ascent - metrics.descent + metrics.line_gap;

        Some(Self {
            font,
            font_size: size,
            atlas_data,
            atlas_width,
            atlas_height,
            cursor_x: 1, // 1px padding
            cursor_y: 1,
            row_height: 0,
            glyphs: HashMap::new(),
            line_height,
        })
    }

    /// Try to load a system font. Returns None if not found.
    pub fn from_system_font(size: f32) -> Option<Self> {
        let candidates = [
            r"C:\Windows\Fonts\msyh.ttc",
            r"C:\Windows\Fonts\segoeui.ttf",
            r"C:\Windows\Fonts\arial.ttf",
            r"C:\Windows\Fonts\consola.ttf",
            "/usr/share/fonts/truetype/wqy/wqy-microhei.ttc",
            "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
            "/usr/share/fonts/noto/NotoSans-Regular.ttf",
        ];

        for path in &candidates {
            if let Ok(bytes) = std::fs::read(path) {
                if let Some(atlas) = Self::from_font_bytes(&bytes, size) {
                    println!("[fontdue] loaded system font: {} ({} px)", path, size);
                    return Some(atlas);
                }
            }
        }

        eprintln!(
            "[fontdue] no system font found, tried: {:?}",
            &candidates[..3]
        );
        None
    }

    /// Get or rasterize a glyph. Returns None only if the atlas is full.
    pub fn get_glyph(&mut self, ch: char) -> Option<&GlyphInfo> {
        if self.glyphs.contains_key(&ch) {
            return self.glyphs.get(&ch);
        }

        // Rasterize the glyph.
        let (metrics, bitmap) = self.font.rasterize(ch, self.font_size);

        let w = metrics.width as u32;
        let h = metrics.height as u32;

        // Pack into atlas (simple left-to-right, top-to-bottom).
        if w == 0 || h == 0 {
            // Whitespace character — no bitmap, just advance.
            let info = GlyphInfo {
                atlas_x: 0,
                atlas_y: 0,
                width: 0,
                height: 0,
                xoffset: metrics.xmin as f32,
                yoffset: -(metrics.height as f32 + metrics.ymin as f32),
                xadvance: metrics.advance_width,
            };
            self.glyphs.insert(ch, info);
            return self.glyphs.get(&ch);
        }

        // Check if we need to wrap to next row.
        if self.cursor_x + w + 1 > self.atlas_width {
            self.cursor_x = 1;
            self.cursor_y += self.row_height + 1;
            self.row_height = 0;
        }

        // Check if atlas is full.
        if self.cursor_y + h + 1 > self.atlas_height {
            eprintln!("[fontdue] atlas full, cannot fit glyph '{}'", ch);
            return None;
        }

        // Copy bitmap into atlas.
        for row in 0..h {
            let src_start = (row * w) as usize;
            let dst_start = ((self.cursor_y + row) * self.atlas_width + self.cursor_x) as usize;
            let src_end = src_start + w as usize;
            let dst_end = dst_start + w as usize;
            if src_end <= bitmap.len() && dst_end <= self.atlas_data.len() {
                self.atlas_data[dst_start..dst_end].copy_from_slice(&bitmap[src_start..src_end]);
            }
        }

        let info = GlyphInfo {
            atlas_x: self.cursor_x,
            atlas_y: self.cursor_y,
            width: w,
            height: h,
            xoffset: metrics.xmin as f32,
            yoffset: -(metrics.height as f32 + metrics.ymin as f32),
            xadvance: metrics.advance_width,
        };

        self.cursor_x += w + 1;
        self.row_height = self.row_height.max(h);

        self.glyphs.insert(ch, info);
        self.glyphs.get(&ch)
    }

    /// Get advance width for a character (without rasterizing if already cached).
    pub fn advance_width(&mut self, ch: char) -> f32 {
        if let Some(g) = self.glyphs.get(&ch) {
            return g.xadvance;
        }
        // Rasterize to get metrics.
        self.get_glyph(ch)
            .map(|g| g.xadvance)
            .unwrap_or(self.font_size * 0.5)
    }

    /// Check if any new glyphs were rasterized since last upload (atlas is dirty).
    /// For simplicity, we always re-upload the full atlas each frame if there are draws.
    /// A production implementation would track dirty regions.
    pub fn atlas_bytes(&self) -> &[u8] {
        &self.atlas_data
    }
}
