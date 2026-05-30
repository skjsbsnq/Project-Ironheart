//! CR-2.1.2 — Build-time SVG → PNG atlas 栅格化。
//!
//! 把 `assets/counter_icons/{00..15}_<name>.svg` 渲染到一张
//! 1024×64 RGBA8 PNG（16 列 × 64 cell × 64 px tall），输出到
//! `$OUT_DIR/counter_atlas.png`，运行时由 [`crate::counter_atlas`]
//! 通过 `include_bytes!` 嵌入并解码为 wgpu 纹理。
//!
//! ## 设计决策
//!
//! - **64×64 cell**：源 SVG viewBox 是 64×48（HOI3 长方形比例），但 cell
//!   做成正方形让纹理坐标更直观；垂直方向上下各 8px 留白（透明）。
//! - **2× hi-DPI 不在范围**：roadmap 留 feature flag `atlas-hi-dpi`
//!   做未来扩展；CR-2.1 只输出 1×。
//! - **缺文件 = 编译失败**：列表中的 SVG 缺一个就 panic，方便 CI 抓回归。
//! - **`cargo:rerun-if-changed=assets/counter_icons`**：任意 SVG 变化触发重栅格化。

use std::path::PathBuf;

/// 16 个 archetype 的 SVG 文件名（与 [`UnitArchetype`] 数值顺序对齐）。
/// 索引 N 对应 `UnitArchetype::from_u8(N)`。
const ICON_FILES: [&str; 16] = [
    "00_unknown.svg",
    "01_infantry.svg",
    "02_cavalry.svg",
    "03_motorized.svg",
    "04_mechanized.svg",
    "05_mountain.svg",
    "06_marine.svg",
    "07_paratrooper.svg",
    "08_militia.svg",
    "09_light_armor.svg",
    "10_medium_armor.svg",
    "11_heavy_armor.svg",
    "12_modern_armor.svg",
    "13_artillery.svg",
    "14_anti_tank.svg",
    "15_anti_air.svg",
];

const CELL_W: u32 = 64;
const CELL_H: u32 = 64;
/// 源 SVG 高度（viewBox 0 0 64 48）。把 SVG 垂直居中到 64×64 cell 内。
const SVG_VBOX_H: u32 = 48;
const ATLAS_W: u32 = CELL_W * 16; // 1024
const ATLAS_H: u32 = CELL_H; // 64

fn main() {
    println!("cargo:rerun-if-changed=assets/counter_icons");
    println!("cargo:rerun-if-changed=build.rs");

    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set");
    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR not set");
    let svg_dir = PathBuf::from(&manifest_dir)
        .join("assets")
        .join("counter_icons");

    // ── 渲染：每个 SVG → 64×64 RGBA8 pixmap ────────────────────────────
    let mut atlas: Vec<u8> = vec![0; (ATLAS_W * ATLAS_H * 4) as usize];

    let usvg_opts = usvg::Options::default();

    for (idx, file) in ICON_FILES.iter().enumerate() {
        let svg_path = svg_dir.join(file);
        let svg_bytes = std::fs::read(&svg_path).unwrap_or_else(|e| {
            panic!(
                "[counter_icons] missing {}: {} (CR-2.1 needs all 16 archetype SVGs)",
                svg_path.display(),
                e
            );
        });

        let tree = usvg::Tree::from_data(&svg_bytes, &usvg_opts)
            .unwrap_or_else(|e| panic!("[counter_icons] parse {} failed: {e}", svg_path.display()));

        // SVG viewBox 是 64×48；缩放保持纵横比，64×48 → 64×48（1:1），
        // 然后垂直居中到 64×64 cell（上下各 8px 透明 padding）。
        let mut pixmap = tiny_skia::Pixmap::new(CELL_W, CELL_H).expect("alloc 64x64 pixmap");
        let y_offset = ((CELL_H - SVG_VBOX_H) / 2) as f32; // (64 - 48) / 2 = 8
        let transform = tiny_skia::Transform::from_translate(0.0, y_offset);
        resvg::render(&tree, transform, &mut pixmap.as_mut());

        // ── 拷贝 pixmap 到 atlas 横向第 idx 个 cell ────────────────────
        // tiny_skia::Pixmap.data() 是 premultiplied RGBA8 row-major。
        // atlas 是 1024×64 RGBA8 row-major，dst column 起点 = idx * 64。
        let pixmap_data = pixmap.data();
        let cell_col = (idx as u32) * CELL_W;
        for row in 0..CELL_H {
            let src = (row * CELL_W * 4) as usize;
            let dst = (((row * ATLAS_W) + cell_col) * 4) as usize;
            atlas[dst..dst + (CELL_W as usize) * 4]
                .copy_from_slice(&pixmap_data[src..src + (CELL_W as usize) * 4]);
        }
    }

    // ── 编码为 PNG ─────────────────────────────────────────────────────
    let png_path = PathBuf::from(&out_dir).join("counter_atlas.png");
    let img = image::RgbaImage::from_raw(ATLAS_W, ATLAS_H, atlas)
        .expect("ATLAS_W * ATLAS_H * 4 == buffer length");
    img.save_with_format(&png_path, image::ImageFormat::Png)
        .unwrap_or_else(|e| panic!("[counter_icons] save {} failed: {e}", png_path.display()));

    println!(
        "cargo:warning=[counter_icons] atlas {}x{} -> {} ({} icons)",
        ATLAS_W,
        ATLAS_H,
        png_path.display(),
        ICON_FILES.len()
    );
}
