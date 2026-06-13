use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use hoi4_assets::dds::{DdsFormat, DdsImage};
use hoi4_paths::PathConfig;
use hoi4_ui::vanilla_gui::{GfxIndex, GfxResource, GfxResourceKind};

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root")
        .to_path_buf()
}

fn write_report(name: &str, body: impl AsRef<str>) {
    let path = workspace_root()
        .join("target/finance_sprite_probe")
        .join(name);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, body.as_ref()).unwrap();
}

// Sprites the finance .gui currently references, plus 9-slice candidates we
// might swap in. The probe dumps each one's real size/border so we stop guessing.
const FINANCE_CURRENT: &[&str] = &[
    "GFX_tiled_window_1b_thin_border",
    "GFX_tiled_paper_bg2",
    "GFX_tiled_plain_bg",
    "GFX_win_header_short",
    "GFX_header_bg",
    "GFX_tab_diplomacy_bg",
    "GFX_tab_intel_ledger",
    "GFX_diplo_actions_bg",
    "GFX_diplo_relations_bg",
    "GFX_decision_category_header_bg",
    "GFX_diplo_countrylist_entry",
    "GFX_decision_item_bg",
    "GFX_prod_progress_bar3",
    "GFX_production_progressbar_frame2",
    "GFX_button_123x34",
    "GFX_closebutton",
    "GFX_resources_strip",
];

// Gate 1: tab-strip art the redesign wants for a real frame-switched tab bar.
// `GFX_tab_intel_ledger` is the named candidate (noOfFrames = 2). `_bg` is the
// single-frame backing plate behind the intel-ledger tab row.
const TAB_CANDIDATES: &[&str] = &["GFX_tab_intel_ledger", "GFX_tab_diplomacy_bg"];

// Gate 1: every 9-slice we might use for a finance section card, including the
// two overlay variants the roadmap flagged as "maybe too thick" and the flat
// `GFX_tiled_stats_bg` we are trying to retire (kept as a baseline).
const CARD_CANDIDATES: &[&str] = &[
    "GFX_tiled_generic_overlay_bg1",
    "GFX_tiled_generic_overlay_bg1_small",
    "GFX_tiled_window_small",
    "GFX_tiled_window_small_small",
    "GFX_tiled_window_1b_thin_border",
    "GFX_intel_ledger_tabbed_panel_bg",
    "GFX_tiled_decisions_bg_small",
    "GFX_tiled_plain_bg_small",
    "GFX_tiled_generic_bg_1",
    "GFX_tiled_plain_bg",
    "GFX_tiled_stats_bg",
];

// The actual (width, height) of every section-card container in the current
// finance .gui, smallest to largest. A 9-slice needs target_w >= left+right and
// target_h >= top+bottom or its corners overlap and the frame renders corrupt.
const FINANCE_CARDS: &[(u32, u32, &str)] = &[
    (166, 56, "pressure_* (overview)"),
    (508, 72, "budget_totals"),
    (508, 96, "financing / trade_impact / gdp_totals"),
    (508, 110, "forex_metrics"),
    (508, 150, "treasury_block  <- Gate 1 reference 508x150"),
    (508, 152, "debt_metrics"),
    (508, 192, "investment_block"),
    (508, 232, "construction_block"),
];

fn describe(resource: &hoi4_ui::vanilla_gui::GfxResource) -> String {
    let size = resource
        .size
        .map(|s| format!("{}x{}", s.x, s.y))
        .unwrap_or_else(|| "<none>".to_owned());
    let border = resource
        .border
        .map(|b| format!("L{} R{} T{} B{}", b.left, b.right, b.top, b.bottom))
        .unwrap_or_else(|| "<none>".to_owned());
    let frames = resource
        .frame_count
        .map(|f| f.to_string())
        .unwrap_or_else(|| "-".to_owned());
    format!(
        "kind={:<22} size={:<14} border={:<22} frames={}",
        resource.kind.label(),
        size,
        border,
        frames
    )
}

// ─── DDS color sampling (real-machine tonality, not name guessing) ───────────

fn rgb565(c: u16) -> (u8, u8, u8) {
    let r5 = ((c >> 11) & 0x1F) as u8;
    let g6 = ((c >> 5) & 0x3F) as u8;
    let b5 = (c & 0x1F) as u8;
    (
        (r5 << 3) | (r5 >> 2),
        (g6 << 2) | (g6 >> 4),
        (b5 << 3) | (b5 >> 2),
    )
}

fn luma(r: u8, g: u8, b: u8) -> u32 {
    (299 * r as u32 + 587 * g as u32 + 114 * b as u32) / 1000
}

/// Mean sRGB color of the pixels inside [x0,x1) x [y0,y1) on mip0. For BC1/BC3
/// we average the two RGB565 block endpoints (cheap and faithful for "is this
/// bronze or green"); for BGRA8 we average raw pixels. None = unsupported fmt.
fn avg_color_rect(dds: &DdsImage, x0: u32, x1: u32, y0: u32, y1: u32) -> Option<(u8, u8, u8)> {
    let mip0 = dds.mip_data(0)?;
    let (mut sr, mut sg, mut sb, mut n) = (0u64, 0u64, 0u64, 0u64);
    match dds.format {
        DdsFormat::Bc1 | DdsFormat::Bc3 => {
            let block_bytes = dds.format.block_bytes()? as usize;
            let color_off = if dds.format == DdsFormat::Bc3 { 8 } else { 0 };
            let blocks_wide = ((dds.width + 3) / 4) as usize;
            let blocks_tall = ((dds.height + 3) / 4) as usize;
            for by in 0..blocks_tall {
                let center_y = (by * 4 + 2) as u32;
                if center_y < y0 || center_y >= y1 {
                    continue;
                }
                for bx in 0..blocks_wide {
                    let center_x = (bx * 4 + 2) as u32;
                    if center_x < x0 || center_x >= x1 {
                        continue;
                    }
                    let off = (by * blocks_wide + bx) * block_bytes + color_off;
                    if off + 4 > mip0.len() {
                        continue;
                    }
                    let c0 = u16::from_le_bytes([mip0[off], mip0[off + 1]]);
                    let c1 = u16::from_le_bytes([mip0[off + 2], mip0[off + 3]]);
                    let (r0, g0, b0) = rgb565(c0);
                    let (r1, g1, b1) = rgb565(c1);
                    sr += r0 as u64 + r1 as u64;
                    sg += g0 as u64 + g1 as u64;
                    sb += b0 as u64 + b1 as u64;
                    n += 2;
                }
            }
        }
        DdsFormat::Bgra8 => {
            let w = dds.width as usize;
            for y in (y0 as usize)..(y1 as usize).min(dds.height as usize) {
                for x in (x0 as usize)..(x1 as usize).min(w) {
                    let idx = (y * w + x) * 4;
                    if idx + 3 > mip0.len() {
                        continue;
                    }
                    sb += mip0[idx] as u64;
                    sg += mip0[idx + 1] as u64;
                    sr += mip0[idx + 2] as u64;
                    n += 1;
                }
            }
        }
        _ => return None,
    }
    if n == 0 {
        None
    } else {
        Some(((sr / n) as u8, (sg / n) as u8, (sb / n) as u8))
    }
}

/// Full-width/height convenience over [`avg_color_rect`].
fn avg_color_region(dds: &DdsImage, x0: u32, x1: u32) -> Option<(u8, u8, u8)> {
    avg_color_rect(dds, x0, x1, 0, dds.height)
}

fn tonality_label(r: u8, g: u8, b: u8) -> String {
    let (rf, gf, bf) = (r as i32, g as i32, b as i32);
    let chroma = rf.max(gf).max(bf) - rf.min(gf).min(bf);
    if chroma < 16 {
        return format!("neutral grey (chroma {chroma})");
    }
    if gf > rf && gf >= bf {
        format!("GREEN-dominant g{gf}>r{rf} (chroma {chroma})")
    } else if rf >= gf && gf >= bf {
        format!("warm bronze/amber r{rf}>=g{gf}>b{bf} (chroma {chroma})")
    } else if bf > rf && bf > gf {
        format!("blue-dominant (chroma {chroma})")
    } else {
        format!("mixed r{rf} g{gf} b{bf} (chroma {chroma})")
    }
}

fn fmt_rgb(c: Option<(u8, u8, u8)>) -> String {
    c.map(|(r, g, b)| format!("({r:>3},{g:>3},{b:>3})"))
        .unwrap_or_else(|| "    n/a    ".to_owned())
}

fn load_dds(resource: &GfxResource, path_cfg: &PathConfig) -> Result<(PathBuf, DdsImage), String> {
    let tex = resource
        .primary_texture
        .as_deref()
        .ok_or_else(|| "no textureFile".to_owned())?;
    let path = path_cfg
        .find(tex)
        .ok_or_else(|| format!("unresolved texture {tex}"))?;
    let bytes = std::fs::read(&path).map_err(|err| err.to_string())?;
    let dds = DdsImage::parse(&bytes).map_err(|err| format!("{err:?}"))?;
    Ok((path, dds))
}

fn card_fits(border: &hoi4_ui::vanilla_gui::gfx_index::GfxBorder, w: u32, h: u32) -> bool {
    let min_w = (border.left + border.right) as u32;
    let min_h = (border.top + border.bottom) as u32;
    w >= min_w && h >= min_h
}

#[test]
fn finance_sprite_probe_report() {
    let Ok(path_cfg) = PathConfig::resolve(Default::default()) else {
        eprintln!("finance_sprite_probe: HOI4 install not available, skipping");
        return;
    };
    let gfx = GfxIndex::from_path_config(&path_cfg);

    let mut report = String::from("# Finance sprite probe (real install dimensions)\n\n");

    let _ = writeln!(report, "## 1. Currently referenced finance sprites\n");
    let _ = writeln!(report, "```");
    for name in FINANCE_CURRENT {
        match gfx.get(name) {
            Some(res) => {
                let _ = writeln!(report, "{:<38} {}", name, describe(res));
            }
            None => {
                let _ = writeln!(report, "{:<38} <MISSING IN INSTALL>", name);
            }
        }
    }
    let _ = writeln!(report, "```\n");

    // 9-slice = corneredTile WITH a declared border. Only these scale cleanly.
    let _ = writeln!(
        report,
        "## 2. All true 9-slice sprites (corneredTile + border) in install\n"
    );
    let _ = writeln!(report, "```");
    let mut nine_slice: Vec<&hoi4_ui::vanilla_gui::GfxResource> = gfx
        .iter()
        .filter(|r| r.kind == GfxResourceKind::CorneredTile && r.border.is_some())
        .collect();
    nine_slice.sort_by(|a, b| a.name.cmp(&b.name));
    for res in &nine_slice {
        let _ = writeln!(report, "{:<46} {}", res.name, describe(res));
    }
    let _ = writeln!(report, "```\n");
    let _ = writeln!(report, "total 9-slice with border: {}\n", nine_slice.len());

    // Candidate window/frame/panel/button sprites worth eyeballing for card bg.
    let _ = writeln!(report, "## 3. Window/frame/bg/button sprite name survey\n");
    let _ = writeln!(report, "```");
    let mut survey: Vec<&hoi4_ui::vanilla_gui::GfxResource> = gfx
        .iter()
        .filter(|r| {
            let n = r.name.to_ascii_lowercase();
            n.contains("window")
                || n.contains("_frame")
                || n.contains("_bg")
                || n.contains("button")
                || n.contains("_tile")
                || n.contains("tiled_")
        })
        .collect();
    survey.sort_by(|a, b| a.name.cmp(&b.name));
    for res in &survey {
        let _ = writeln!(report, "{:<48} {}", res.name, describe(res));
    }
    let _ = writeln!(report, "```\n");
    let _ = writeln!(report, "total survey: {}\n", survey.len());

    // ── Section 4: tab-strip frame analysis (Gate 1 deliverable #1) ──────────
    let _ = writeln!(
        report,
        "## 4. Tab strip frame analysis (Gate 1: noOfFrames + per-frame size + selected frame)\n"
    );
    let _ = writeln!(
        report,
        "HOI4 multi-frame sprites are horizontal strips: frame 1 = leftmost, frame N = rightmost.\n\
         A 2-frame toggle defaults to `frame = 1` at rest, so frame 2 is the selected/active art.\n\
         We corroborate by sampling each frame in top/mid/bottom bands; the selection cue is the\n\
         band with the largest inter-frame luma delta (the lit/selected tab is brighter there).\n"
    );
    for name in TAB_CANDIDATES {
        let _ = writeln!(report, "### {name}");
        let Some(res) = gfx.get(name) else {
            let _ = writeln!(report, "- <MISSING IN INSTALL>\n");
            continue;
        };
        let frames = res.frame_count.unwrap_or(1).max(1);
        let _ = writeln!(
            report,
            "- gfx_source: {}",
            res.source
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "<none>".to_owned())
        );
        let _ = writeln!(report, "- textureFile: {:?}", res.primary_texture);
        let _ = writeln!(report, "- noOfFrames: {frames}");
        match load_dds(res, &path_cfg) {
            Ok((path, dds)) => {
                let pfw = dds.width / frames;
                let _ = writeln!(
                    report,
                    "- texture: {}x{} format={:?} mips={} resolved={}",
                    dds.width,
                    dds.height,
                    dds.format,
                    dds.mip_count(),
                    path.display()
                );
                let _ = writeln!(report, "- per_frame_size: {pfw}x{}", dds.height);
                // Sample each frame in top/mid/bottom bands; the selection cue on
                // HOI4 tabs is a localized highlight (often the lit top edge),
                // which a whole-frame average washes out. Track band lumas to find it.
                let h = dds.height;
                let bands = [(0, h / 3), (h / 3, 2 * h / 3), (2 * h / 3, h)];
                let band_names = ["top", "mid", "bot"];
                let mut frame_luma: Vec<[u32; 3]> = Vec::new();
                for f in 0..frames {
                    let x0 = f * dds.width / frames;
                    let x1 = (f + 1) * dds.width / frames;
                    let (fr, fg, fb) = avg_color_region(&dds, x0, x1).unwrap_or((0, 0, 0));
                    let mut lum = [0u32; 3];
                    let mut band_str = String::new();
                    for (i, (y0, y1)) in bands.iter().enumerate() {
                        if let Some((r, g, b)) = avg_color_rect(&dds, x0, x1, *y0, *y1) {
                            lum[i] = luma(r, g, b);
                            let _ = write!(band_str, " {}={}", band_names[i], lum[i]);
                        }
                    }
                    frame_luma.push(lum);
                    let _ = writeln!(
                        report,
                        "  - frame[{f}] (1-based frame={}) x=[{x0},{x1}) avg_srgb=({fr},{fg},{fb}) luma={} bands:{band_str} :: {}",
                        f + 1,
                        luma(fr, fg, fb),
                        tonality_label(fr, fg, fb),
                    );
                }
                if frames == 2 {
                    // Selection cue = the band with the largest luma difference
                    // between the two frames; the brighter (lit) frame there is
                    // the selected/active state.
                    let mut best = (0i64, 0usize); // (|delta|, band index)
                    for i in 0..3 {
                        let d = (frame_luma[1][i] as i64 - frame_luma[0][i] as i64).abs();
                        if d > best.0 {
                            best = (d, i);
                        }
                    }
                    let sel = if frame_luma[1][best.1] >= frame_luma[0][best.1] {
                        2
                    } else {
                        1
                    };
                    let _ = writeln!(
                        report,
                        "- max inter-frame band delta: {} luma in '{}' band => brighter/lit there = frame {} (selected/active)",
                        best.0, band_names[best.1], sel
                    );
                    let _ = writeln!(
                        report,
                        "- convention cross-check: HOI4 2-frame toggles rest at `frame = 1`, so frame 2 = selected"
                    );
                } else if frames == 1 {
                    let _ = writeln!(
                        report,
                        "- single frame: backing plate only, no selected/unselected toggle"
                    );
                }
            }
            Err(err) => {
                let _ = writeln!(report, "- texture load failed: {err}");
            }
        }
        let _ = writeln!(report);
    }

    // ── Section 5: section-card 9-slice feasibility (Gate 1 deliverable #2) ──
    let _ = writeln!(
        report,
        "## 5. Section-card 9-slice feasibility on real finance card sizes (Gate 1)\n"
    );
    let _ = writeln!(
        report,
        "A 9-slice renders correctly only when card_w >= L+R AND card_h >= T+B.\n\
         Finance card heights in the current .gui: 56, 72, 96, 110, 150, 152, 192, 232 (width 508,\n\
         except the 166-wide pressure cards). PASS/FAIL per candidate per card below.\n"
    );
    for name in CARD_CANDIDATES {
        let Some(res) = gfx.get(name) else {
            let _ = writeln!(report, "### {name}\n- <MISSING IN INSTALL>\n");
            continue;
        };
        let Some(border) = res.border else {
            let _ = writeln!(
                report,
                "### {name}\n- border=<none> (not a true 9-slice; cannot frame a card)\n"
            );
            continue;
        };
        let size = res
            .size
            .map(|s| format!("{}x{}", s.x, s.y))
            .unwrap_or_else(|| "<none>".to_owned());
        let _ = writeln!(
            report,
            "### {name}  size={size} border=L{} R{} T{} B{}  (needs w>={}, h>={})",
            border.left,
            border.right,
            border.top,
            border.bottom,
            (border.left + border.right) as u32,
            (border.top + border.bottom) as u32,
        );
        let mut all = true;
        for (w, h, label) in FINANCE_CARDS {
            let ok = card_fits(&border, *w, *h);
            all &= ok;
            let _ = writeln!(
                report,
                "  - {}  {}x{}  {label}",
                if ok { "PASS" } else { "FAIL" },
                w,
                h
            );
        }
        let _ = writeln!(
            report,
            "  => VERDICT: {} all finance cards\n",
            if all { "FITS" } else { "DOES NOT FIT" }
        );
    }

    // ── Section 6: card-bg tonality (resolves the "military green?" claim) ───
    let _ = writeln!(
        report,
        "## 6. Card background tonality (mean sRGB, resolves \"military green?\")\n"
    );
    let _ = writeln!(
        report,
        "`edge` = top border strip, `center` = inner fill — the two regions a card actually shows.\n"
    );
    let _ = writeln!(report, "```");
    for name in CARD_CANDIDATES {
        let Some(res) = gfx.get(name) else {
            let _ = writeln!(report, "{name:<40} <MISSING IN INSTALL>");
            continue;
        };
        match load_dds(res, &path_cfg) {
            Ok((_, dds)) => {
                let whole = avg_color_region(&dds, 0, dds.width);
                // Split into 9-slice regions using the sprite's own border (or a
                // 1/6 fallback) so we read the visible frame edge + fill, not the
                // whole sheet which is dominated by dark tiling fill.
                let (bt, bl, br, bb) = res
                    .border
                    .map(|b| (b.top as u32, b.left as u32, b.right as u32, b.bottom as u32))
                    .unwrap_or((dds.height / 6, dds.width / 6, dds.width / 6, dds.height / 6));
                let edge = avg_color_rect(&dds, 0, dds.width, 0, bt.clamp(1, dds.height));
                let cx0 = bl.min(dds.width.saturating_sub(1));
                let cx1 = dds.width.saturating_sub(br).max(cx0 + 1);
                let cy0 = bt.min(dds.height.saturating_sub(1));
                let cy1 = dds.height.saturating_sub(bb).max(cy0 + 1);
                let center = avg_color_rect(&dds, cx0, cx1, cy0, cy1);
                let label = whole
                    .map(|(r, g, b)| tonality_label(r, g, b))
                    .unwrap_or_else(|| "<unsupported>".to_owned());
                let _ = writeln!(
                    report,
                    "{name:<40} {:>5?} {:>4}x{:<4} whole={} edge={} center={} :: {}",
                    dds.format,
                    dds.width,
                    dds.height,
                    fmt_rgb(whole),
                    fmt_rgb(edge),
                    fmt_rgb(center),
                    label
                );
            }
            Err(err) => {
                let _ = writeln!(report, "{name:<40} <load failed: {err}>");
            }
        }
    }
    let _ = writeln!(report, "```\n");

    write_report("finance_sprite_probe.md", &report);
    eprintln!("finance_sprite_probe: wrote target/finance_sprite_probe/finance_sprite_probe.md");

    // Gate 1 guards: only assert when the install actually carries these sprites
    // (intel ledger is DLC content), so a base-game install still produces the
    // report without a hard failure.
    assert!(report.contains("## 4. Tab strip frame analysis"));
    assert!(report.contains("## 5. Section-card 9-slice feasibility"));
    assert!(report.contains("## 6. Card background tonality"));
    if let Some(res) = gfx.get("GFX_tab_intel_ledger") {
        assert_eq!(
            res.frame_count,
            Some(2),
            "GFX_tab_intel_ledger must expose 2 frames for selected/unselected toggle"
        );
    }
}
