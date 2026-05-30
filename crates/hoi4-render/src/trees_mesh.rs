//! 3D mesh tree rendering — Phase 3.6.3.
//!
//! Loads Paradox .mesh tree models and renders them with GPU instancing.
//! Each tree type (deciduous, conifer, tropical) has its own mesh + texture,
//! drawn with a separate indexed draw call sharing the same pipeline.

/// GPU vertex for tree mesh geometry (vertex-rate buffer).
/// Matches the shader's MeshVertex struct.
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable, Debug)]
pub struct TreeMeshVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
}

/// Per-instance data for tree placement (instance-rate buffer).
/// Matches the shader's InstanceData struct.
///
/// 24 bytes total. Phase 3.10.2 added per-instance slope (replacing the
/// formerly opaque 4-byte pad). Field order mirrors `TreeInstance` so
/// `filter_instances_for_type` is a straight per-field copy:
/// * `pos`       Float32x3 @ 0
/// * `scale`     Float32   @ 12
/// * `tint`      Unorm8x4  @ 16
/// * `tree_type` Uint8x2   @ 20  (`.x` used; `.y` = pad)
/// * `slope`     Snorm8x2  @ 22
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable, Debug)]
pub struct TreeMeshInstance {
    pub pos: [f32; 3],
    pub scale: f32,
    pub tint: [u8; 4],
    pub tree_type: u8,
    pub _pad0: u8,
    /// Slope ∂h/∂X packed as i8 (±127 → ±0.5 world Y per world XZ unit).
    pub slope_x: i8,
    /// Slope ∂h/∂Z packed as i8.
    pub slope_z: i8,
}

/// One loaded tree mesh type ready for GPU upload.
pub struct TreeMeshData {
    pub vertices: Vec<TreeMeshVertex>,
    /// u32 indices — Paradox `tri` attribute is `i` type (4-byte int) and we keep
    /// width consistent across building/unit meshes that may exceed 64 K verts.
    pub indices: Vec<u32>,
    pub vertex_count: u32,
    pub index_count: u32,
}

/// Load a tree mesh from raw submesh data (positions, normals, UVs, indices).
/// This avoids a direct dependency on hoi4-assets from hoi4-render.
pub fn build_tree_mesh(
    positions: &[[f32; 3]],
    normals: &[[f32; 3]],
    uvs: &[[f32; 2]],
    indices: &[u32],
) -> Option<TreeMeshData> {
    if positions.is_empty() || indices.is_empty() {
        return None;
    }

    let vc = positions.len();
    let mut vertices = Vec::with_capacity(vc);

    for i in 0..vc {
        let pos = positions[i];
        let normal = if i < normals.len() {
            normals[i]
        } else {
            [0.0, 1.0, 0.0]
        };
        let uv = if i < uvs.len() { uvs[i] } else { [0.0, 0.0] };
        vertices.push(TreeMeshVertex {
            position: pos,
            normal,
            uv,
        });
    }

    Some(TreeMeshData {
        vertex_count: vc as u32,
        index_count: indices.len() as u32,
        indices: indices.to_vec(),
        vertices,
    })
}

/// Convert TreeInstance data (from the existing placement system) into
/// TreeMeshInstance data for a specific tree type.
pub fn filter_instances_for_type(
    instances: &[super::trees::TreeInstance],
    target_type: u8,
) -> Vec<TreeMeshInstance> {
    instances
        .iter()
        .filter(|t| t.tree_type == target_type)
        .map(|t| TreeMeshInstance {
            pos: t.pos,
            scale: t.scale,
            tint: t.tint,
            tree_type: t.tree_type,
            _pad0: 0,
            slope_x: t.slope_x,
            slope_z: t.slope_z,
        })
        .collect()
}

/// Phase 3.7.3：3D mesh 实例数上限。每种树类型最多 N 个最近实例（其余留给
/// billboard fallback）。N=12000 对应 1080p / 60 FPS 经验值——3 类共 36 K
/// 个 draw 实例，大致与原 25 K billboard 同量级。
pub const INSTANCE_CAP_PER_TYPE: usize = 12_000;

/// 按 stride 均匀抽样把实例数压到 cap 之内。保留确定性顺序（不打乱），让远景
/// LOD 在 shader 内做距离淡出而非 CPU 重排。
///
/// 抽样策略：若 `instances.len() > cap`，每 `n / cap` 取一个 → 最坏情况下 cap
/// 个实例覆盖全部地图区域而不是聚集一隅。
pub fn cap_instances(mut instances: Vec<TreeMeshInstance>, cap: usize) -> Vec<TreeMeshInstance> {
    if instances.len() <= cap || cap == 0 {
        return instances;
    }
    let n = instances.len();
    // step = n / cap (向上取整) 保证恰好 cap 个被选中
    let step = (n + cap - 1) / cap;
    let mut picked = Vec::with_capacity(cap);
    let mut i = 0usize;
    while i < n && picked.len() < cap {
        picked.push(instances[i]);
        i += step;
    }
    instances.clear();
    picked
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mk_inst(pos: [f32; 3]) -> TreeMeshInstance {
        TreeMeshInstance {
            pos,
            scale: 1.0,
            tint: [255, 255, 255, 255],
            tree_type: 0,
            _pad0: 0,
            slope_x: 0,
            slope_z: 0,
        }
    }

    #[test]
    fn build_tree_mesh_returns_none_on_empty() {
        assert!(build_tree_mesh(&[], &[], &[], &[]).is_none());
    }

    #[test]
    fn build_tree_mesh_packs_vertices() {
        let positions = vec![[1.0, 2.0, 3.0], [4.0, 5.0, 6.0]];
        let normals = vec![[0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
        let uvs = vec![[0.0, 0.0], [1.0, 1.0]];
        let indices = vec![0u32, 1, 0];
        let m = build_tree_mesh(&positions, &normals, &uvs, &indices).unwrap();
        assert_eq!(m.vertex_count, 2);
        assert_eq!(m.index_count, 3);
        assert_eq!(m.vertices[0].position, [1.0, 2.0, 3.0]);
        assert_eq!(m.vertices[1].uv, [1.0, 1.0]);
    }

    #[test]
    fn cap_instances_passthrough_when_below_cap() {
        let v = vec![mk_inst([0.0, 0.0, 0.0]); 100];
        let r = cap_instances(v.clone(), 200);
        assert_eq!(r.len(), 100);
    }

    #[test]
    fn cap_instances_subsamples_to_cap_or_below() {
        let mut v = Vec::new();
        for i in 0..100 {
            v.push(mk_inst([i as f32, 0.0, 0.0]));
        }
        let r = cap_instances(v, 10);
        // step = ceil(100/10) = 10 → 10 个：[0,10,20,...,90]
        assert!(r.len() <= 10);
        assert!(!r.is_empty());
        // 第一个保留
        assert_eq!(r[0].pos[0], 0.0);
    }

    #[test]
    fn cap_instances_zero_cap_returns_input_untouched() {
        let v = vec![mk_inst([0.0, 0.0, 0.0]); 5];
        let r = cap_instances(v.clone(), 0);
        assert_eq!(r.len(), 5);
    }

    #[test]
    fn instance_size_is_24_bytes() {
        // GPU vertex layout assumes 24 bytes (matches TreeInstance + slope fields).
        assert_eq!(std::mem::size_of::<TreeMeshInstance>(), 24);
    }
}
