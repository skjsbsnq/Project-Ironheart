/// CR-4: Screen-space layout + hit-test for HOI3 counters.
use std::collections::HashMap;

pub struct LayoutCounter {
    pub pos: [f32; 2],
    pub size: [f32; 2],
    pub anchor: [f32; 2],
    pub province_id: u16,
}

pub struct HitRegion {
    pub rect: [f32; 4], // x, y, w, h
    pub province_id: u16,
    pub hierarchy_level: u8,
}

/// 5 iterations of overlap resolution using grid spatial hash (cell=80px).
///
/// 2026-05-20：每轮在解决重叠之外加入一个**朝 `anchor` 的回弹力**——以前算法
/// 只把兵牌互推，导致密集战线被"扇形展开"成远离原始省份的一团。现在每轮把
/// 当前 pos 朝 anchor 拉回 35%，**并在每轮末尾把偏离 anchor 的距离硬钳到
/// `MAX_OFFSET` 像素**，确保兵牌绝对不会被推过国境（如捷克兵跑到德国）。
pub fn layout_screen_space(counters: &mut [LayoutCounter], _screen_w: f32, _screen_h: f32) {
    if counters.len() < 2 {
        return;
    }
    // cell 与缩小后的师级 counter 宽度（约 56 px）匹配，保留邻域查询冗余。
    let cell = 80.0f32;
    // 每轮把 pos 往 anchor 拉回的比例。0 = 关闭回弹（旧行为），1 = 立刻回到 anchor。
    // 2026-05-20 v3：把贴地修复后，anchor 已经精确对应省份位置；
    // 适度放松到 0.4 让重叠的兵牌能稍微互相挤开。
    let anchor_pull = 0.4f32;
    // 兵牌中心相对 anchor 中心允许的最大偏移（像素）。
    // 2026-05-20 v3：贴地后维持 18px 上限——足够让两个兵牌挤开但不跨国境。
    const MAX_OFFSET_PX: f32 = 18.0;

    for _ in 0..5 {
        // Build spatial hash
        let mut grid: HashMap<(i32, i32), Vec<usize>> = HashMap::new();
        for (i, c) in counters.iter().enumerate() {
            let cx = (c.pos[0] / cell) as i32;
            let cy = (c.pos[1] / cell) as i32;
            grid.entry((cx, cy)).or_default().push(i);
        }
        // Collect push vectors
        let mut pushes: Vec<[f32; 2]> = vec![[0.0; 2]; counters.len()];
        // 2026-05-20 v4 fix —— HashMap::keys() 顺序不稳定会导致 push 累积顺序
        // 每帧不同 → 兵牌位置抖动闪烁。先 sort 让迭代确定。
        let mut keys: Vec<(i32, i32)> = grid.keys().copied().collect();
        keys.sort_unstable();
        for key in &keys {
            for dx in -1..=1 {
                for dy in -1..=1 {
                    let nk = (key.0 + dx, key.1 + dy);
                    let Some(neighbors) = grid.get(&nk) else {
                        continue;
                    };
                    let Some(cell_items) = grid.get(key) else {
                        continue;
                    };
                    for &i in cell_items {
                        for &j in neighbors {
                            if j <= i {
                                continue;
                            }
                            let a = &counters[i];
                            let b = &counters[j];
                            let ox = (a.size[0] + b.size[0]) * 0.5
                                - (b.pos[0] + b.size[0] * 0.5 - (a.pos[0] + a.size[0] * 0.5)).abs();
                            let oy = (a.size[1] + b.size[1]) * 0.5
                                - (b.pos[1] + b.size[1] * 0.5 - (a.pos[1] + a.size[1] * 0.5)).abs();
                            if ox > 0.0 && oy > 0.0 {
                                let damp = 0.5;
                                // 仅按实际重叠量推开，移除旧公式中的 +1.0 偏置——
                                // 该偏置让微重叠被放大成几像素硬推，与 anchor 回弹叠加后产生抖动。
                                let push_x =
                                    ox * 0.5 * damp * if a.pos[0] < b.pos[0] { -1.0 } else { 1.0 };
                                let push_y =
                                    oy * 0.5 * damp * if a.pos[1] < b.pos[1] { -1.0 } else { 1.0 };
                                if ox < oy {
                                    pushes[i][0] += push_x;
                                    pushes[j][0] -= push_x;
                                } else {
                                    pushes[i][1] += push_y;
                                    pushes[j][1] -= push_y;
                                }
                            }
                        }
                    }
                }
            }
        }
        // Apply pushes + anchor pull + hard clamp to MAX_OFFSET.
        for (i, c) in counters.iter_mut().enumerate() {
            c.pos[0] += pushes[i][0];
            c.pos[1] += pushes[i][1];
            // CR-4 fix (2026-05-20)：把 pos 朝 anchor（原始省份投影）回弹一个比例，
            // 避免被推开的兵牌无限漂离它实际所在的省份。
            c.pos[0] += (c.anchor[0] - c.pos[0]) * anchor_pull;
            c.pos[1] += (c.anchor[1] - c.pos[1]) * anchor_pull;
            // 硬钳：偏离 anchor 不超过 MAX_OFFSET_PX，避免跨国境（如捷克→德国）。
            let dx = c.pos[0] - c.anchor[0];
            let dy = c.pos[1] - c.anchor[1];
            let d = (dx * dx + dy * dy).sqrt();
            if d > MAX_OFFSET_PX {
                let k = MAX_OFFSET_PX / d;
                c.pos[0] = c.anchor[0] + dx * k;
                c.pos[1] = c.anchor[1] + dy * k;
            }
        }
    }
}

pub fn build_hit_regions(counters: &[LayoutCounter]) -> Vec<HitRegion> {
    counters
        .iter()
        .map(|c| HitRegion {
            rect: [c.pos[0], c.pos[1], c.size[0], c.size[1]],
            province_id: c.province_id,
            hierarchy_level: 0,
        })
        .collect()
}

pub fn hit_test(regions: &[HitRegion], x: f32, y: f32) -> Option<u16> {
    for r in regions.iter().rev() {
        if x >= r.rect[0]
            && x <= r.rect[0] + r.rect[2]
            && y >= r.rect[1]
            && y <= r.rect[1] + r.rect[3]
        {
            return Some(r.province_id);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 2026-05-20 回归保护：当两个兵牌挤在一起时，layout 不应把任何一方
    /// 推得偏离 anchor 超过 MAX_OFFSET_PX（= 28px），防止"捷克兵跑到德国"。
    #[test]
    fn layout_does_not_drift_far_from_anchor() {
        // 三个兵牌中心几乎重合（同省份内），都被推开但都应靠近各自 anchor。
        let mut counters = vec![
            LayoutCounter {
                pos: [100.0, 100.0],
                size: [56.0, 38.0],
                anchor: [100.0, 100.0],
                province_id: 1,
            },
            LayoutCounter {
                pos: [102.0, 101.0],
                size: [56.0, 38.0],
                anchor: [102.0, 101.0],
                province_id: 2,
            },
            LayoutCounter {
                pos: [104.0, 99.0],
                size: [56.0, 38.0],
                anchor: [104.0, 99.0],
                province_id: 3,
            },
        ];
        layout_screen_space(&mut counters, 1920.0, 1080.0);
        for c in &counters {
            let dx = c.pos[0] - c.anchor[0];
            let dy = c.pos[1] - c.anchor[1];
            let d = (dx * dx + dy * dy).sqrt();
            assert!(
                d <= 20.0, // MAX_OFFSET_PX = 18，留 2px 钳位浮点容差
                "counter drifted {} px from anchor {:?}",
                d,
                c.anchor,
            );
        }
    }

    /// 单兵牌不触发任何位移（保留 anchor）。
    #[test]
    fn single_counter_no_shift() {
        let mut counters = vec![LayoutCounter {
            pos: [500.0, 500.0],
            size: [56.0, 38.0],
            anchor: [500.0, 500.0],
            province_id: 1,
        }];
        layout_screen_space(&mut counters, 1920.0, 1080.0);
        assert_eq!(counters[0].pos, [500.0, 500.0]);
    }

    /// 两个分离很远的兵牌不互相推动。
    #[test]
    fn far_apart_counters_unchanged() {
        let mut counters = vec![
            LayoutCounter {
                pos: [100.0, 100.0],
                size: [56.0, 38.0],
                anchor: [100.0, 100.0],
                province_id: 1,
            },
            LayoutCounter {
                pos: [800.0, 800.0],
                size: [56.0, 38.0],
                anchor: [800.0, 800.0],
                province_id: 2,
            },
        ];
        layout_screen_space(&mut counters, 1920.0, 1080.0);
        // 各自偏离 anchor 不应超过 1 像素（无重叠时不应有推力）。
        for c in &counters {
            let dx = (c.pos[0] - c.anchor[0]).abs();
            let dy = (c.pos[1] - c.anchor[1]).abs();
            assert!(
                dx <= 1.0 && dy <= 1.0,
                "isolated counter drifted: dx={} dy={}",
                dx,
                dy
            );
        }
    }
}
