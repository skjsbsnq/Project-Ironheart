//! Phase 3.5：`mapname` — 国家名地图标签。
//!
//! 原版 `gfx/FX/mapname.fxh` 是把国名沿着地形曲面贴上去的特殊 shader。
//! 我们采用一个等价但更简单的方案：CPU 算每国 centroid（owned-province
//! 像素重心），逐帧用 view-projection 矩阵投影到屏幕坐标，然后让现有
//! 的 [`hoi4-app::text_pass::TextPass`]（fontdue + dynamic atlas）按屏幕
//! 坐标画字。视觉效果与"shader 沿曲面贴字"几乎一致——因为相机俯视角下
//! 屏幕投影的标签 *就是* 国家中心点上方的世界文字。
//!
//! 不在本模块范围：
//! * 实际文字渲染（由 `text_pass` 完成）
//! * 字号/缩放策略（由 caller 按 zoom_factor 决定）
//!
//! 本模块只负责：
//! 1. 一次性计算 `country_idx → (world_x, world_z, owned_province_count)`
//! 2. 提供 `world_pos_for_country()` 给主循环按 zoom 决定要不要画

/// 一个国家的标签锚点。
#[derive(Debug, Clone, Copy)]
pub struct CountryLabel {
    /// World-space X (already scaled by world_scale).
    pub world_x: f32,
    /// World-space Z (already scaled by world_scale).
    pub world_z: f32,
    /// 该国拥有的省份数（用于 LOD：小国低 zoom 时不画）。
    pub province_count: u32,
}

/// 输入 `province_centroids` 是 `compute_province_centroids` 的输出
/// （以**像素坐标**为单位，长度 = max_province_id + 1）。
///
/// 输入 `province_owners` 是 `World.provinces.owners` 数组（每省的 owner
/// CountryId）。函数把每个 country index 对应的所有省份取像素均值，再
/// 按 `world_scale` 转 world 坐标。
///
/// 输出 `Vec<Option<CountryLabel>>`，长度 = `country_count`。无任何省份
/// 的国家是 `None`。
///
/// 复杂度：O(max(provinces, countries))。
pub fn compute_country_labels(
    province_centroids: &[(f32, f32)],
    province_owner_idx: &[Option<usize>],
    country_count: usize,
    world_scale: f32,
) -> Vec<Option<CountryLabel>> {
    // accumulate sum_x, sum_z, count per country
    let mut sums = vec![(0.0f32, 0.0f32, 0u32); country_count];

    let n = province_centroids.len().min(province_owner_idx.len());
    for pid in 0..n {
        let Some(owner) = province_owner_idx[pid] else {
            continue;
        };
        if owner >= country_count {
            continue;
        }
        let (px, py) = province_centroids[pid];
        if px == 0.0 && py == 0.0 {
            continue; // unmapped province (no pixels)
        }
        let entry = &mut sums[owner];
        entry.0 += px;
        entry.1 += py;
        entry.2 += 1;
    }

    sums.into_iter()
        .map(|(sx, sy, n)| {
            if n == 0 {
                None
            } else {
                Some(CountryLabel {
                    world_x: (sx / n as f32) * world_scale,
                    world_z: (sy / n as f32) * world_scale,
                    province_count: n,
                })
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_country_count() {
        let labels = compute_country_labels(&[], &[], 0, 0.02);
        assert!(labels.is_empty());
    }

    #[test]
    fn single_country_single_province() {
        let centroids = vec![(0.0, 0.0), (100.0, 50.0)]; // pid 0 unmapped, pid 1 at (100,50)
        let owners = vec![None, Some(0usize)];
        let out = compute_country_labels(&centroids, &owners, 1, 0.02);
        assert_eq!(out.len(), 1);
        let l = out[0].unwrap();
        assert!((l.world_x - 2.0).abs() < 1e-6);
        assert!((l.world_z - 1.0).abs() < 1e-6);
        assert_eq!(l.province_count, 1);
    }

    #[test]
    fn multi_province_average() {
        let centroids = vec![(0.0, 0.0), (10.0, 20.0), (30.0, 40.0), (50.0, 60.0)];
        let owners = vec![None, Some(0usize), Some(0usize), Some(0usize)];
        let out = compute_country_labels(&centroids, &owners, 1, 1.0);
        let l = out[0].unwrap();
        // mean of (10,20),(30,40),(50,60) = (30, 40)
        assert!((l.world_x - 30.0).abs() < 1e-3);
        assert!((l.world_z - 40.0).abs() < 1e-3);
        assert_eq!(l.province_count, 3);
    }

    #[test]
    fn zero_centroid_provinces_skipped() {
        let centroids = vec![(0.0, 0.0); 4];
        let owners = vec![Some(0usize); 4];
        let out = compute_country_labels(&centroids, &owners, 1, 1.0);
        assert!(out[0].is_none(), "all-zero centroids should yield no label");
    }

    #[test]
    fn unowned_provinces_ignored() {
        let centroids = vec![(0.0, 0.0), (10.0, 10.0), (50.0, 50.0)];
        let owners = vec![None, None, None];
        let out = compute_country_labels(&centroids, &owners, 2, 1.0);
        assert!(out[0].is_none());
        assert!(out[1].is_none());
    }
}
