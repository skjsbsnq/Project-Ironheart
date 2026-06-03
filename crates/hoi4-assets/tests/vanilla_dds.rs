//! V5 收口（2026-05-18）：真实 vanilla DDS 文件解析回归。
//!
//! V3 时代本测试通过 `GfxIdx-removed` 间接拿到 texturefile 路径；V5 放弃 vanilla GUI
//! 路线后改为按 `docs/vanilla_assets_used.md` 白名单中的显式 DDS 路径直接打开。
//!
//! 此测试只在能定位真实 HOI4 安装目录时运行。

use hoi4_assets::{
    dds_upload_plan, AssetDb, DdsImage, FsAssetDb, MapAssetAudit, MapResRole, VanillaMapSet,
};
use hoi4_paths::PathConfig;

/// 白名单内、最稳定的若干 DDS 资产。每条都来自 vanilla `gfx/` 根目录下的
/// 不变路径（多年版本未改），用于回归 DDS 解析器本身。
const SAMPLE_DDS_PATHS: &[&str] = &[
    "gfx/loadingscreens/load_5.dds",
    "gfx/interface/colored_button_148.dds",
    "gfx/interface/icon_factory.dds",
    "gfx/interface/topbar/zoom_in.dds",
    "gfx/interface/topbar/zoom_out.dds",
];

#[test]
fn parse_vanilla_dds_files_without_panic() {
    let cfg = match PathConfig::resolve(Default::default()) {
        Ok(c) => c,
        Err(_) => {
            eprintln!("[dds_vanilla] HOI4 install not found — skipping");
            return;
        }
    };
    let db = FsAssetDb::new(cfg);

    let mut parsed = 0u32;
    let mut failed = 0u32;

    for path in SAMPLE_DDS_PATHS {
        match db.open(path) {
            Ok(bytes) => match DdsImage::parse(&bytes) {
                Ok(img) => {
                    parsed += 1;
                    assert!(img.width > 0);
                    assert!(img.height > 0);
                    assert!(img.mip_count() >= 1);
                    assert!(!img.data.is_empty());
                    println!(
                        "  [ok] {} → {}x{} {:?} mips={}",
                        path,
                        img.width,
                        img.height,
                        img.format,
                        img.mip_count()
                    );
                }
                Err(e) => {
                    failed += 1;
                    println!("  [FAIL] {} parse: {}", path, e);
                }
            },
            Err(_) => {
                // 文件不存在（语言版差异），跳过
                continue;
            }
        }
    }

    println!("[dds_vanilla] parsed={} failed={}", parsed, failed);
    if parsed == 0 {
        eprintln!("[dds_vanilla] all sample paths missing — skipping success assertion");
        return;
    }
    assert_eq!(failed, 0, "不应有解析失败");
}

#[test]
fn frame_uvs_compute_without_gfx_idx_removed() {
    use hoi4_assets::compute_frame_uvs;

    let uvs1 = compute_frame_uvs(1);
    assert_eq!(uvs1.len(), 1);
    assert!((uvs1[0].u_min - 0.0).abs() < 1e-6);
    assert!((uvs1[0].u_max - 1.0).abs() < 1e-6);

    let uvs4 = compute_frame_uvs(4);
    assert_eq!(uvs4.len(), 4);
    assert!((uvs4[0].u_min - 0.0).abs() < 1e-6);
    assert!((uvs4[3].u_max - 1.0).abs() < 1e-6);
    // 均分
    for i in 0..3 {
        let expected_max = (i + 1) as f32 / 4.0;
        assert!((uvs4[i].u_max - expected_max).abs() < 1e-5);
    }
}

#[test]
fn vanilla_map_visual_resources_parse_and_upload_plan() {
    let cfg = match PathConfig::resolve(Default::default()) {
        Ok(c) => c,
        Err(_) => {
            eprintln!("[map_visual_resources] HOI4 install not found - skipping");
            return;
        }
    };
    let db = FsAssetDb::new(cfg);
    let map_set = VanillaMapSet::load(&db).expect("required map resources should load");
    let audit = MapAssetAudit::from_map_set(&map_set);

    assert_eq!(audit.fallback, 0, "map resources should not need fallback");

    for role in [
        MapResRole::ColormapEmissive,
        MapResRole::CityLights(0),
        MapResRole::TerrainAtlas(0),
        MapResRole::TerrainAtlasNormal(0),
        MapResRole::Lean1,
        MapResRole::Lean2,
        MapResRole::ColormapWater(0),
        MapResRole::FowWaterSpec,
        MapResRole::UnderwaterTerrain(0),
        MapResRole::RiverDiffuse(0),
        MapResRole::RiverNormal(0),
        MapResRole::RiverMasks,
    ] {
        let bytes = map_set
            .bytes(role)
            .unwrap_or_else(|| panic!("missing {}", role.relative_path()));
        let dds = DdsImage::parse(bytes)
            .unwrap_or_else(|err| panic!("{} failed to parse: {err}", role.relative_path()));
        let plan = dds_upload_plan(&dds)
            .unwrap_or_else(|| panic!("{} has no direct upload plan", role.relative_path()));
        assert!(
            plan.upload_mip_count > 0,
            "{} should expose at least one uploadable mip",
            role.relative_path()
        );
    }
}
