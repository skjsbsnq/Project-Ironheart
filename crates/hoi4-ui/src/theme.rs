//! Vanilla theme — V5 阶段 B.3。
//!
//! 给 [`egui::Context`] 应用一套接近 HOI4 vanilla 调性的视觉：
//!
//! - **字体**：Latin 衬线（Windows `georgia.ttf`，Garamond-alike 近似 vanilla
//!   Garamond 系列）+ CJK fallback（Windows `msyh.ttc` Microsoft YaHei）。
//!   vanilla HOI4 在 `gfx/fonts/` **不发布任何 `.ttf`**（全是 BmFont），所以
//!   B.3 走系统字体路线，不读 vanilla 字体文件。详见
//!   `docs/vanilla_assets_used.md` §5（该文件已在 B.3 同步修正）。
//! - **配色**：木纹深褐主背景 + 暖金高亮 / 提交色，对照 vanilla `tiled_window_*.dds`
//!   的色温（暖棕 + 金边）。9-slice 木纹纹理在 B.4 接入；本阶段先用纯色块
//!   把整体调性落到位。
//!
//! ## API
//!
//! ```ignore
//! let assets = hoi4_ui::theme::apply_vanilla_theme(&ui.ctx);
//! println!("[ui] vanilla theme: latin={:?} cjk={:?}",
//!     assets.latin_serif_path, assets.cjk_fallback_path);
//! ```
//!
//! 返回的 [`VanillaThemeAssets`] 记录实际命中的字体路径（用于启动 banner /
//! 调试）。如果 Latin 或 CJK 任一未找到，对应字段是 `None`，egui 会回落到
//! 自带的 Ubuntu-Light + Hack。

use std::path::PathBuf;
use std::sync::Arc;

use egui::{Color32, Context, FontData, FontDefinitions, FontFamily, Stroke, Visuals};

/// `apply_vanilla_theme` 返回的字体加载结果。
#[derive(Debug, Default, Clone)]
pub struct VanillaThemeAssets {
    /// 实际命中的 Latin 衬线字体路径，None = 全部候选都读不到。
    pub latin_serif_path: Option<PathBuf>,
    /// 实际命中的 CJK fallback 字体路径，None = 全部候选都读不到（中文会显示豆腐块）。
    pub cjk_fallback_path: Option<PathBuf>,
}

/// 把 vanilla theme（字体 + 配色）应用到 `ctx`。
pub fn apply_vanilla_theme(ctx: &Context) -> VanillaThemeAssets {
    let mut assets = VanillaThemeAssets::default();
    let mut fonts = FontDefinitions::default();

    // ─── Latin 衬线（Garamond-alike） ──────────────────────────────────────
    let latin_candidates: &[&str] = &[
        r"C:\Windows\Fonts\georgia.ttf", // 主选：Old Style 衬线，最贴近 vanilla
        r"C:\Windows\Fonts\palab.ttf",   // Palatino Bold，备选衬线
        r"C:\Windows\Fonts\times.ttf",   // Times New Roman，最后退路
        "/Library/Fonts/Georgia.ttf",    // macOS
        "/usr/share/fonts/truetype/liberation/LiberationSerif-Regular.ttf", // Linux
    ];
    if let Some((bytes, path)) = first_readable(latin_candidates) {
        fonts.font_data.insert(
            "vanilla_serif".to_owned(),
            Arc::new(FontData::from_owned(bytes)),
        );
        prepend(&mut fonts, FontFamily::Proportional, "vanilla_serif");
        assets.latin_serif_path = Some(path);
    }

    // ─── CJK fallback（中文 / 日文 / 韩文） ─────────────────────────────────
    let cjk_candidates: &[&str] = &[
        r"C:\Windows\Fonts\msyh.ttc",         // 主选：Microsoft YaHei
        r"C:\Windows\Fonts\msyh.ttf",         // 老版本命名
        r"C:\Windows\Fonts\msyhbd.ttc",       // YaHei Bold
        r"C:\Windows\Fonts\simsun.ttc",       // SimSun 退路
        r"C:\Windows\Fonts\simhei.ttf",       // SimHei 退路
        "/System/Library/Fonts/PingFang.ttc", // macOS
        "/usr/share/fonts/truetype/wqy/wqy-microhei.ttc", // Linux
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
    ];
    if let Some((bytes, path)) = first_readable(cjk_candidates) {
        let mut fd = FontData::from_owned(bytes);
        fd.tweak.scale = 1.0;
        // .ttc 文件中第 0 个 face 是 Regular，包含完整 CJK 字符集。
        fd.index = 0;
        fonts
            .font_data
            .insert("cjk_fallback".to_owned(), Arc::new(fd));
        // append（不是 prepend）：vanilla 衬线优先，CJK 仅作回退。
        append(&mut fonts, FontFamily::Proportional, "cjk_fallback");
        append(&mut fonts, FontFamily::Monospace, "cjk_fallback");
        assets.cjk_fallback_path = Some(path);
    }

    ctx.set_fonts(fonts);
    install_visuals(ctx);
    assets
}

/// 把 `name` 插到 `family` 列表的最前（最高优先级）。
fn prepend(fonts: &mut FontDefinitions, family: FontFamily, name: &str) {
    fonts
        .families
        .entry(family)
        .or_default()
        .insert(0, name.to_owned());
}

/// 把 `name` 追加到 `family` 列表末尾（fallback）。
fn append(fonts: &mut FontDefinitions, family: FontFamily, name: &str) {
    fonts
        .families
        .entry(family)
        .or_default()
        .push(name.to_owned());
}

/// 试遍候选列表，返回第一份能读到的字节 + 命中路径。
fn first_readable(candidates: &[&str]) -> Option<(Vec<u8>, PathBuf)> {
    for path in candidates {
        if let Ok(bytes) = std::fs::read(path) {
            return Some((bytes, PathBuf::from(path)));
        }
    }
    None
}

/// 配色：木纹暗背景 + 暖金提交。后续 B.4 9-slice 纹理接入后，这里的实色块
/// 会被纹理填充覆盖；现在这版纯色已经足够把"画面调性"落到 vanilla 区间。
fn install_visuals(ctx: &Context) {
    let mut v = Visuals::dark();

    // 木纹暗褐底（vanilla `tiled_window_transparent.dds` 的色温平均值近似）。
    v.window_fill = Color32::from_rgb(0x08, 0x0a, 0x09);
    v.panel_fill = Color32::from_rgb(0x0d, 0x10, 0x0f);
    v.extreme_bg_color = Color32::from_rgb(0x03, 0x04, 0x04);
    v.faint_bg_color = Color32::from_rgb(0x16, 0x1a, 0x19);
    v.code_bg_color = Color32::from_rgb(0x07, 0x09, 0x09);

    // 暖金强调色（vanilla 按钮 highlight / focus 节点完成色 / 进度条暖光近似）。
    let gold = Color32::from_rgb(0x8e, 0xa9, 0xaf);
    let gold_bright = Color32::from_rgb(0xc9, 0xd8, 0xd8);
    let gold_dark = Color32::from_rgb(0x35, 0x49, 0x4f);

    // hyperlink / 选中态走亮金。
    v.hyperlink_color = gold_bright;
    v.selection.bg_fill = gold_dark;
    v.selection.stroke = Stroke::new(1.0, gold_bright);
    v.warn_fg_color = gold_bright;

    // 文本主色：暖羊皮纸（vanilla tooltip / label 默认文本近似）。
    let parchment = Color32::from_rgb(0xd8, 0xd6, 0xc8);
    v.override_text_color = Some(parchment);

    // 控件三态：inactive 暗木 / hovered 暖金棕 / active 亮金。
    let stroke_dark = Stroke::new(1.0, Color32::from_rgb(0x28, 0x31, 0x31));
    let stroke_gold = Stroke::new(1.5, gold);
    let stroke_gold_bright = Stroke::new(1.5, gold_bright);

    v.widgets.noninteractive.bg_fill = Color32::from_rgb(0x12, 0x15, 0x14);
    v.widgets.noninteractive.weak_bg_fill = Color32::from_rgb(0x0d, 0x10, 0x0f);
    v.widgets.noninteractive.bg_stroke = stroke_dark;
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, parchment);

    v.widgets.inactive.bg_fill = Color32::from_rgb(0x17, 0x1c, 0x1b);
    v.widgets.inactive.weak_bg_fill = Color32::from_rgb(0x11, 0x15, 0x14);
    v.widgets.inactive.bg_stroke = stroke_dark;
    v.widgets.inactive.fg_stroke = Stroke::new(1.0, parchment);

    v.widgets.hovered.bg_fill = Color32::from_rgb(0x24, 0x31, 0x33);
    v.widgets.hovered.weak_bg_fill = Color32::from_rgb(0x1a, 0x25, 0x27);
    v.widgets.hovered.bg_stroke = stroke_gold;
    v.widgets.hovered.fg_stroke = Stroke::new(1.5, gold_bright);

    v.widgets.active.bg_fill = Color32::from_rgb(0x35, 0x49, 0x4f);
    v.widgets.active.weak_bg_fill = Color32::from_rgb(0x24, 0x31, 0x33);
    v.widgets.active.bg_stroke = stroke_gold_bright;
    v.widgets.active.fg_stroke = Stroke::new(1.5, Color32::WHITE);

    v.widgets.open.bg_fill = Color32::from_rgb(0x17, 0x1c, 0x1b);
    v.widgets.open.bg_stroke = stroke_gold;
    v.widgets.open.fg_stroke = Stroke::new(1.0, gold_bright);

    // 窗口外框：暖金 1.5px 描边 + 轻微圆角，呼应 vanilla 的「金属边框」感。
    v.window_stroke = Stroke::new(1.5, gold_dark);
    v.window_shadow.color = Color32::from_black_alpha(160);

    ctx.set_visuals(v);
}
