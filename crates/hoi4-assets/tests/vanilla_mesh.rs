//! Phase 3.7.1：真实 vanilla `.mesh` 文件解析回归。
//!
//! 只在能定位真实 HOI4 安装目录时运行。无 HOI4 的 CI 环境会跳过。

use hoi4_assets::{AssetDb, FsAssetDb, PdxMesh};
use hoi4_paths::PathConfig;
use std::path::{Path, PathBuf};

/// 尝试解析 `gfx/models/mapitems/trees/beech.mesh` 并验证常识量级。
#[test]
fn parses_beech_tree_mesh() {
    let cfg = match PathConfig::resolve(Default::default()) {
        Ok(c) => c,
        Err(_) => {
            eprintln!("[pdx_mesh_vanilla] HOI4 install not found — skipping");
            return;
        }
    };
    let db = FsAssetDb::new(cfg);
    let bytes = match db.open("gfx/models/mapitems/trees/beech.mesh") {
        Ok(b) => b,
        Err(_) => {
            eprintln!("[pdx_mesh_vanilla] beech.mesh not present — skipping");
            return;
        }
    };

    let mesh = PdxMesh::parse(&bytes).expect("beech.mesh should parse");

    // 至少有一个 LOD。
    assert!(
        !mesh.meshes.is_empty(),
        "beech.mesh has zero submeshes — parser failed silently"
    );

    let sub0 = &mesh.meshes[0];
    // 顶点数应该 > 100（实测 605）。
    assert!(
        sub0.positions.len() > 100,
        "beech submesh 0 has only {} vertices — expected > 100",
        sub0.positions.len()
    );
    // 法线数 == 顶点数。
    assert_eq!(sub0.normals.len(), sub0.positions.len());
    // UV 数 == 顶点数。
    assert_eq!(sub0.uvs.len(), sub0.positions.len());
    // 索引为 3 的倍数。
    assert!(!sub0.indices.is_empty());
    assert_eq!(sub0.indices.len() % 3, 0);
    // 索引值都 < 顶点数。
    let max_idx = *sub0.indices.iter().max().unwrap();
    assert!((max_idx as usize) < sub0.positions.len());

    // 包围盒非零（树是有体积的实体）。
    let dims = [
        mesh.bounds_max[0] - mesh.bounds_min[0],
        mesh.bounds_max[1] - mesh.bounds_min[1],
        mesh.bounds_max[2] - mesh.bounds_min[2],
    ];
    assert!(
        dims[0] > 0.5 && dims[1] > 0.5 && dims[2] > 0.5,
        "beech bounds too small: {:?}",
        dims
    );
    assert!(
        dims[0] < 30.0 && dims[1] < 30.0 && dims[2] < 30.0,
        "beech bounds suspiciously large (mesh corrupt?): {:?}",
        dims
    );

    // 材质引用：至少有 shader 名 + diffuse。
    assert!(
        !sub0.material.shader.is_empty(),
        "no shader name extracted from beech material block"
    );
    assert!(
        sub0.material
            .diffuse
            .as_deref()
            .map(|s| s.contains("beech"))
            .unwrap_or(false),
        "diffuse texture should reference beech: {:?}",
        sub0.material.diffuse
    );

    println!(
        "[pdx_mesh_vanilla] beech: {} submeshes (LODs), submesh0={}v {}i, bounds=[{:.2},{:.2},{:.2}]→[{:.2},{:.2},{:.2}]",
        mesh.meshes.len(),
        sub0.positions.len(),
        sub0.indices.len(),
        mesh.bounds_min[0],
        mesh.bounds_min[1],
        mesh.bounds_min[2],
        mesh.bounds_max[0],
        mesh.bounds_max[1],
        mesh.bounds_max[2],
    );
    println!(
        "[pdx_mesh_vanilla] material: shader={} diff={:?} normal={:?} spec={:?}",
        sub0.material.shader, sub0.material.diffuse, sub0.material.normal, sub0.material.specular,
    );
    println!("[pdx_mesh_vanilla] lod_distances={:?}", mesh.lod_distances);
}

#[test]
fn parses_pine_and_palm_tree_meshes() {
    let cfg = match PathConfig::resolve(Default::default()) {
        Ok(c) => c,
        Err(_) => {
            eprintln!("[pdx_mesh_vanilla] HOI4 install not found — skipping");
            return;
        }
    };
    let db = FsAssetDb::new(cfg);

    for path in &[
        "gfx/models/mapitems/trees/Pine_01.mesh",
        "gfx/models/mapitems/trees/palmer.mesh",
    ] {
        let Ok(bytes) = db.open(path) else {
            eprintln!("[pdx_mesh_vanilla] {} not present — skipping", path);
            continue;
        };
        let mesh = PdxMesh::parse(&bytes).unwrap_or_else(|e| {
            panic!("{} parse failed: {}", path, e);
        });
        assert!(!mesh.meshes.is_empty(), "{} has zero submeshes", path);
        let sub0 = &mesh.meshes[0];
        assert!(sub0.positions.len() > 50, "{} has too few verts", path);
        assert_eq!(sub0.indices.len() % 3, 0);
        println!(
            "[pdx_mesh_vanilla] {} OK: {} submeshes, sub0={}v {}i",
            path,
            mesh.meshes.len(),
            sub0.positions.len(),
            sub0.indices.len(),
        );
    }
}

/// 全 vanilla `gfx/models/**/*.mesh` 解析不 panic + 抽样合理性。
///
/// 这是 ROADMAP 3.7.1 的核心规模门：414 个 vanilla mesh，期望 > 95% 解析成功
/// 且每个成功的至少有 1 个 submesh + 顶点数 > 0。
#[test]
fn parses_all_vanilla_meshes_without_panic() {
    let cfg = match PathConfig::resolve(Default::default()) {
        Ok(c) => c,
        Err(_) => {
            eprintln!("[pdx_mesh_vanilla_bulk] HOI4 install not found — skipping");
            return;
        }
    };

    let game_root = cfg.game_path().to_path_buf();
    let models_root = game_root.join("gfx").join("models");
    if !models_root.exists() {
        eprintln!(
            "[pdx_mesh_vanilla_bulk] {:?} not present — skipping",
            models_root
        );
        return;
    }

    let mut all_paths: Vec<PathBuf> = Vec::new();
    walk_dir(&models_root, &mut all_paths);

    let total = all_paths.len();
    if total == 0 {
        eprintln!(
            "[pdx_mesh_vanilla_bulk] no .mesh files under {:?}",
            models_root
        );
        return;
    }

    let mut ok = 0u32;
    let mut empty = 0u32;
    let mut fail: Vec<(PathBuf, String)> = Vec::new();
    let mut total_verts = 0usize;
    let mut total_idx = 0usize;

    for path in &all_paths {
        let bytes = match std::fs::read(path) {
            Ok(b) => b,
            Err(_) => continue,
        };
        match PdxMesh::parse(&bytes) {
            Ok(m) => {
                if m.meshes.is_empty() {
                    empty += 1;
                } else {
                    ok += 1;
                    total_verts += m.vertex_count();
                    total_idx += m.index_count();
                }
            }
            Err(e) => fail.push((path.clone(), e.to_string())),
        }
    }

    println!(
        "[pdx_mesh_vanilla_bulk] total={} ok={} empty={} fail={} | verts={} idx={}",
        total,
        ok,
        empty,
        fail.len(),
        total_verts,
        total_idx,
    );

    if fail.len() > 0 {
        for (p, e) in fail.iter().take(5) {
            eprintln!("  fail: {:?}: {}", p.file_name().unwrap_or_default(), e);
        }
    }

    // 期望解析成功率高 — 解析失败应该极少（只允许 < 5%）
    let parse_rate = (ok + empty) as f64 / total as f64;
    assert!(
        parse_rate > 0.95,
        "parse rate {:.1}% < 95%",
        parse_rate * 100.0
    );
    // 至少 80% 应该提取到非空数据
    let extract_rate = ok as f64 / total as f64;
    assert!(
        extract_rate > 0.80,
        "extract rate {:.1}% < 80% (likely format bug)",
        extract_rate * 100.0
    );
}

fn walk_dir(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in rd.flatten() {
        let p = entry.path();
        if p.is_dir() {
            walk_dir(&p, out);
        } else if p.extension().and_then(|s| s.to_str()) == Some("mesh") {
            out.push(p);
        }
    }
}
