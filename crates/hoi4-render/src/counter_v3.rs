//! Phase I — Counter Reboot (CR-1) — HOI3-style screen-space procedural counter
//! data generation.
//!
//! 该模块是阶段 I 的"新数据通道"——独立于旧 [`crate::units`] 共存。CR-5 阶段
//! 完成后旧 NATO 兵牌会被删除，本模块成为唯一兵牌数据源。
//!
//! ## 与旧管线的差异
//!
//! | 维度 | 旧 [`UnitCounterInstance`] | 新 [`Hoi3CounterInstance`] |
//! |---|---|---|
//! | 坐标 | 3D 世界空间贴地 | 屏幕空间像素（左上锚点） |
//! | 尺寸 | 32 字节 | 40 字节 |
//! | 实例展开 | 4 层（BG/Symbol/Ideology/Overlay）| 单层 + 叠层底牌（CR-1.5） |
//! | 渲染层级 | division/state/country | division/corps/army/army_group（CR-3） |
//! | 状态 | flags(selected/combat) | flags + organisation/strength/exp 字节级数据 |
//!
//! ## CR-1 阶段范围
//!
//! - [`Hoi3CounterInstance`]：40 字节 GPU instance（`Pod + Zeroable`）。
//! - [`generate_hoi3_counters_v0`]：每省一个 counter，无布局避让，
//!   投影直接给屏幕坐标。
//! - 叠层视觉（HOI3 招牌）：`stack_count > 1` 时 push `min(N-1, 3)` 张
//!   底牌（layer 0..3），每张错位 (+2px, +2px) ×  layer，`flags.bit4 = 1`。
//!
//! CR-2/3/4 会扩展为 OOB 聚合 + 力导向布局 + 兵种 atlas 采样。

use std::collections::{HashMap, HashSet};

use hoi4_map::Heightmap;
use hoi4_state::{CountryId, World};

use crate::units::{template_main_archetype, UnitArchetype};

/// HOI3 风格屏幕空间兵牌的 GPU instance（40 字节 `repr(C)`）。
///
/// 字段布局严格按 CR-1.1 设计（保持 4-byte 对齐，Pod 安全）：
///
/// | 偏移 | 字段 | 类型 | 含义 |
/// |---|---|---|---|
/// | 0 | `screen_pos` | `[f32; 2]` | 屏幕像素坐标（左上锚点） |
/// | 8 | `size` | `[f32; 2]` | 像素尺寸（默认 80×52） |
/// | 16 | `country_color` | `[u8; 4]` | 国家底色 RGBA（Unorm8x4 → 0..1） |
/// | 20 | `archetype` | `u8` | [`UnitArchetype`] 枚举值（0..15） |
/// | 21 | `flags` | `u8` | 见 [`flag_bits`] |
/// | 22 | `stack_count` | `u8` | 1..=255（>255 时显示 "99+"） |
/// | 23 | `organisation` | `u8` | 平均组织度 / 255 → 0..1 |
/// | 24 | `strength` | `u8` | 平均战力 / 255 → 0..1 |
/// | 25 | `experience_level` | `u8` | 0=无 / 1=老练 / 2=精锐 / 3=王牌 |
/// | 26 | `hierarchy_level` | `u8` | 0=师 / 1=军 / 2=军团 / 3=集团军群 |
/// | 27 | `_pad0` | `u8` | 4-byte 对齐填充 |
/// | 28 | `_pad1` | `[f32; 3]` | 凑足 40 字节并保留扩展空间 |
///
/// `_pad0 + _pad1` 共 13 字节填充确保 40 字节对齐到 4 字节边界，让
/// vertex buffer 直接消费。
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Hoi3CounterInstance {
    /// 屏幕像素坐标（左上锚点）。VS 中 + corner * size 得到四角 NDC 输入。
    pub screen_pos: [f32; 2],
    /// 像素尺寸（默认 80×52；军团/集团军群级 CR-3 升到 120/140）。
    pub size: [f32; 2],
    /// 国家底色 RGBA。GPU 端用 Unorm8x4 顶点格式自动除 255。
    pub country_color: [u8; 4],
    /// 兵种归类（[`UnitArchetype`] u8 枚举值，0..15）。
    pub archetype: u8,
    /// 状态位（见 [`flag_bits`]）。
    pub flags: u8,
    /// 堆叠数（1..=255；>255 在 text overlay 显示为 "99+"）。
    pub stack_count: u8,
    /// 组织度 0..255 → 0..1。
    pub organisation: u8,
    /// 战力 0..255 → 0..1。
    pub strength: u8,
    /// 经验档位：0=无 / 1=老练 / 2=精锐 / 3=王牌。
    pub experience_level: u8,
    /// 层级：0=师 / 1=军 / 2=军团 / 3=集团军群（CR-1 全部为 0）。
    pub hierarchy_level: u8,
    /// 4-byte 对齐填充（不可在 shader 中读）。
    pub _pad0: u8,
    /// 凑足 40 字节并保留扩展空间。
    pub _pad1: [f32; 3],
}

impl Hoi3CounterInstance {
    /// 默认师级 counter 像素尺寸（80×52，参照 [`docs/hoi3_counter_mockup.svg`]）。
    pub const DEFAULT_SIZE: [f32; 2] = [80.0, 52.0];

    /// 创建一个全零（视为不可见）实例。`Zeroable` 已派生，本函数仅是语义糖。
    pub fn zeroed() -> Self {
        bytemuck::Zeroable::zeroed()
    }

    /// CR-4: Extract province_id stored in _pad1[0] bits.
    pub fn province_id(&self) -> u16 {
        self._pad1[0].to_bits() as u16
    }
}

/// `Hoi3CounterInstance::flags` 各位语义。
///
/// CR-1 仅使用 `IS_UNDERLAY` + `UNDERLAY_LAYER_MASK`；其它常量预留给
/// CR-2 ~ CR-4 阶段使用。
pub mod flag_bits {
    /// bit0：被选中（CR-4 实现交互态切换）。
    pub const SELECTED: u8 = 1 << 0;
    /// bit1：在战斗中（CR-2 渲染红边外框）。
    pub const IN_COMBAT: u8 = 1 << 1;
    /// bit2：移动中（CR-2 渲染移动箭头）。
    pub const MOVING: u8 = 1 << 2;
    /// bit3：是展开的子单位（CR-4 fan-out 布局生成）。
    pub const EXPANDED_CHILD: u8 = 1 << 3;
    /// bit4：是叠层底牌（CR-1.5 — 不画 bar / 数字 / 兵种符号）。
    pub const IS_UNDERLAY: u8 = 1 << 4;
    /// bits5..6：底牌 layer index（0..3，alpha 递减表）。
    pub const UNDERLAY_LAYER_SHIFT: u8 = 5;
    /// 用 `(flags >> 5) & UNDERLAY_LAYER_MASK` 取 layer 0..3。
    pub const UNDERLAY_LAYER_MASK: u8 = 0b11;
}

struct CounterBucket {
    count: u32,
    owner: CountryId,
    province_id: u16,
    archetype_votes: [u32; UnitArchetype::COUNT as usize],
    in_combat: bool,
    moving: bool,
    sum_org: f64,
    sum_org_max: f64,
    sum_str: f64,
    sum_str_max: f64,
    sum_xp: f64,
    sum_visual_x: f64,
    sum_visual_y: f64,
    visual_count: u32,
}

/// CR-2.7 — 把 vanilla 国家色映射到"高饱和、易辨识"版本。
///
/// 原版国家色（如 GER 暗灰 `[76,76,76]`）在小尺寸兵牌上太暗。
/// 本函数在 HSV 空间中 saturation +30%、value clamp ≥ 0.4，
/// 让所有国家色在 80×52 px 尺寸下仍可辨识。
fn tone_country_color(r: u8, g: u8, b: u8) -> [u8; 4] {
    let rf = r as f32 / 255.0;
    let gf = g as f32 / 255.0;
    let bf = b as f32 / 255.0;
    let max = rf.max(gf).max(bf);
    let min = rf.min(gf).min(bf);
    let delta = max - min;

    // HSV conversion
    let h = if delta < 0.001 {
        0.0
    } else if (max - rf).abs() < 0.001 {
        60.0 * (((gf - bf) / delta) % 6.0)
    } else if (max - gf).abs() < 0.001 {
        60.0 * ((bf - rf) / delta + 2.0)
    } else {
        60.0 * ((rf - gf) / delta + 4.0)
    };
    let s = if max < 0.001 { 0.0 } else { delta / max };
    let v = max;

    // V7 视觉优化：饱和度 +0.30→+0.15、最低亮度 0.4→0.35
    // 防止国家色刺眼 neon 化；vanilla HoI4 兵牌更"沉"一些。
    let new_s = (s + 0.15).min(1.0);
    let new_v = v.max(0.35);

    // HSV → RGB
    let c = new_v * new_s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = new_v - c;
    let (r1, g1, b1) = if h < 0.0 {
        (0.0, 0.0, 0.0)
    } else if h < 60.0 {
        (c, x, 0.0)
    } else if h < 120.0 {
        (x, c, 0.0)
    } else if h < 180.0 {
        (0.0, c, x)
    } else if h < 240.0 {
        (0.0, x, c)
    } else if h < 300.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };
    [
        ((r1 + m) * 255.0).round() as u8,
        ((g1 + m) * 255.0).round() as u8,
        ((b1 + m) * 255.0).round() as u8,
        255,
    ]
}

fn sample_height_bilinear(heightmap: &Heightmap, x: f32, y: f32) -> f32 {
    let w = heightmap.width;
    let h = heightmap.height;
    if w == 0 || h == 0 {
        return 0.0;
    }
    let fx = x.clamp(0.0, w.saturating_sub(1) as f32);
    let fy = y.clamp(0.0, h.saturating_sub(1) as f32);
    let x0 = fx.floor() as u32;
    let y0 = fy.floor() as u32;
    let x1 = (x0 + 1).min(w.saturating_sub(1));
    let y1 = (y0 + 1).min(h.saturating_sub(1));
    let tx = fx - x0 as f32;
    let ty = fy - y0 as f32;
    let idx = |xx: u32, yy: u32| -> usize { (yy * w + xx) as usize };
    let h00 = heightmap.pixels[idx(x0, y0)] as f32 / 255.0;
    let h10 = heightmap.pixels[idx(x1, y0)] as f32 / 255.0;
    let h01 = heightmap.pixels[idx(x0, y1)] as f32 / 255.0;
    let h11 = heightmap.pixels[idx(x1, y1)] as f32 / 255.0;
    let hx0 = h00 + (h10 - h00) * tx;
    let hx1 = h01 + (h11 - h01) * tx;
    hx0 + (hx1 - hx0) * ty
}

/// CR-2.8 — 按 camera distance 选择 counter 像素尺寸。
///
/// 2026-05-26 V7 视觉优化：兵牌整体缩小 ~28% 让地图更通透：
/// - 远（distance > 90）：32×22（最远视角，仅显示位置）
/// - 中（30..90）：从近向远线性插值
/// - 近（< 30）：52×36（贴近师级，仍可读 bar/atlas）
fn size_for_zoom(cam_distance: f32) -> [f32; 2] {
    fn lerp(a: f32, b: f32, t: f32) -> f32 {
        a + (b - a) * t.clamp(0.0, 1.0)
    }

    let (w, h) = if cam_distance <= 30.0 {
        (52.0, 36.0)
    } else if cam_distance <= 90.0 {
        let t = (cam_distance - 30.0) / 60.0;
        (lerp(52.0, 32.0, t), lerp(36.0, 22.0, t))
    } else {
        (32.0, 22.0)
    };
    [w.round(), h.round()]
}

/// CR-1.4 数据生成最小路径：每个有兵省份生成一个顶层 counter；
/// `stack_count > 1` 时再 push `min(stack_count - 1, 3)` 张底牌。
///
/// ## 参数
///
/// - `world`：当前世界状态（数据源）。
/// - `centroids`：每省份的 heightmap 像素中心（CPU 端预计算，长度 ≥ `world.provinces.count`）。
/// - `world_scale` / `height_scale`：与 [`crate::units::generate_map_symbols`] 一致——
///   `centroid_x * world_scale → world_x`，`raw_h / 255 * height_scale → world_y`。
/// - `view_proj`：当前帧 camera VP（用于世界 → NDC 投影）。
/// - `screen_w` / `screen_h`：物理像素尺寸（VS 中除以这个值得 NDC）。
/// - `visible` / `spotted`：fog-of-war 国家集 + 逐省 spotted 集（`None` = 跳过过滤）。
/// - `player`：玩家国家（决定与谁交战，影响敌军是否在 spotted 省份才显示）。
///
/// ## 返回
///
/// 顶牌在前、底牌在后的 `Vec<Hoi3CounterInstance>`。**渲染时**应先画底牌
/// 再画顶牌（在 GPU instance buffer 中**反向**排序，或 vertex shader 内
/// 按 layer 拉远）；CR-1 的 [`crate::counter_v3`] 关联 pass 选择"底牌
/// 先 push"的 buffer 顺序，所以本函数把底牌**在每个堆叠中放在顶牌之前**。
pub fn generate_hoi3_counters_v0(
    world: &World,
    centroids: &[(f32, f32)],
    world_scale: f32,
    height_scale: f32,
    view_proj: &glam::Mat4,
    screen_w: f32,
    screen_h: f32,
    selected_province_ids: &HashSet<u32>,
    cam_distance: f32,
    visible: Option<&HashSet<CountryId>>,
    spotted: Option<&HashSet<u16>>,
    player: CountryId,
    visual_centroids_override: Option<&HashMap<usize, (f32, f32)>>,
) -> Vec<Hoi3CounterInstance> {
    let max_prov = centroids.len().min(world.provinces.count);
    if max_prov == 0 || world.divisions.count == 0 {
        return Vec::new();
    }
    if screen_w <= 0.0 || screen_h <= 0.0 {
        return Vec::new();
    }

    // CR-4.7: group by (province_id, owner, corps_id) when corps data available
    // AND we're at a zoom level where corps distinction matters.
    // At division level (close zoom), group by (province, owner) only to avoid clutter.
    // 2026-05-20 v3：阈值跟 hierarchy_level_for_zoom 同步（>50 = corps+）。
    let has_corps = !world.command.corps.is_empty();
    let split_by_corps = has_corps && cam_distance > 50.0;
    let mut bucket_map: std::collections::HashMap<(u16, u16, u32), CounterBucket> =
        std::collections::HashMap::new();
    let subunits = &world.data.subunits;

    if !world.prov_div_index.is_empty() {
        let mut provinces: Vec<u16> = world.prov_div_index.keys().copied().collect();
        provinces.sort_unstable();
        for pid_u16 in provinces {
            let pid = pid_u16 as usize;
            if pid >= max_prov {
                continue;
            }
            let Some(divs) = world.prov_div_index.get(&pid_u16) else {
                continue;
            };
            for &i in divs {
                add_division_to_counter_bucket(
                    world,
                    centroids,
                    visible,
                    spotted,
                    player,
                    visual_centroids_override,
                    split_by_corps,
                    subunits,
                    max_prov,
                    i,
                    pid,
                    &mut bucket_map,
                );
            }
        }
    } else {
        for i in 0..world.divisions.count {
            let loc = world.divisions.locations[i];
            if loc.is_none() {
                continue;
            }
            let pid = loc.0 as usize;
            add_division_to_counter_bucket(
                world,
                centroids,
                visible,
                spotted,
                player,
                visual_centroids_override,
                split_by_corps,
                subunits,
                max_prov,
                i,
                pid,
                &mut bucket_map,
            );
        }
    }

    let heightmap = &world.map.heightmap;
    let hm_w = heightmap.width;
    let hm_h = heightmap.height;
    let mut out: Vec<Hoi3CounterInstance> = Vec::with_capacity(max_prov / 4);

    // CR-4.7: collect buckets grouped by province for multi-stack offset.
    let mut by_province: std::collections::HashMap<u16, Vec<CounterBucket>> =
        std::collections::HashMap::new();
    for (_, bucket) in bucket_map.into_iter() {
        if bucket.count > 0 {
            by_province
                .entry(bucket.province_id)
                .or_default()
                .push(bucket);
        }
    }
    // 2026-05-20 v6 fix —— bucket_map.into_iter() 顺序不稳定 → 同省份内 stacks 顺序
    // 每帧不同 → merge 选到的代表卡变化 → 屏幕位置抖动闪烁。按 (owner, count desc)
    // 排序让顺序确定。
    for stacks in by_province.values_mut() {
        stacks.sort_by(|a, b| b.count.cmp(&a.count).then(a.owner.0.cmp(&b.owner.0)));
    }

    // 2026-05-20 v6 fix —— HashMap::iter() 顺序不稳定导致 counter 生成顺序每帧不同，
    // merge 阶段的 "代表卡选 idxs[0]" 选到不同实例 → screen_pos 抖动 → 闪烁。
    // 这里按 province_id 排序后再迭代，让输出顺序确定。
    let mut province_keys: Vec<u16> = by_province.keys().copied().collect();
    province_keys.sort_unstable();

    for pid_u16 in &province_keys {
        let stacks = &by_province[pid_u16];
        let pid = *pid_u16 as usize;
        let (cx, cy) = if let Some(bucket) = stacks.first() {
            if bucket.visual_count > 0 {
                (
                    (bucket.sum_visual_x / bucket.visual_count as f64) as f32,
                    (bucket.sum_visual_y / bucket.visual_count as f64) as f32,
                )
            } else {
                centroids[pid]
            }
        } else {
            centroids[pid]
        };
        if cx == 0.0 && cy == 0.0 {
            continue;
        }
        let xi = (cx as u32).min(hm_w.saturating_sub(1));
        let yi = (cy as u32).min(hm_h.saturating_sub(1));
        if hm_w == 0 || hm_h == 0 {
            continue;
        }
        let raw_h = heightmap.pixels[(yi * hm_w + xi) as usize];
        if raw_h <= 95 {
            continue;
        }
        // Match terrain.wgsl's bilinear height sampling so the screen-space
        // counter anchor projects from the same surface point as the map mesh.
        let world_y = sample_height_bilinear(heightmap, cx, cy) * height_scale + 0.01;
        let world_x = cx * world_scale;
        let world_z = cy * world_scale;

        let clip = *view_proj * glam::Vec4::new(world_x, world_y, world_z, 1.0);
        if clip.w <= 0.0 {
            continue;
        }
        let nx = clip.x / clip.w;
        let ny = clip.y / clip.w;
        let nz = clip.z / clip.w;
        if nz < 0.0 || nz > 1.0 {
            continue;
        }
        let sx = (nx * 0.5 + 0.5) * screen_w;
        let sy = (1.0 - (ny * 0.5 + 0.5)) * screen_h;

        let size = size_for_zoom(cam_distance);

        for (stack_idx, bucket) in stacks.iter().enumerate() {
            // screen_pos 是左上锚点，把投影点放在 counter 中心：
            let mut top_x = sx - size[0] * 0.5;
            let mut top_y = sy - size[1] * 0.5;

            // CR-4.7: multi-stack offset
            match stack_idx {
                1 => {
                    top_x += size[0] * 0.6;
                }
                2 => {
                    top_x -= size[0] * 0.6;
                }
                3 => {
                    top_y += size[1] * 0.6;
                }
                _ => {}
            }

            // CR-2.3.5：战斗"前倾"— in_combat 时向最近敌方省份偏移 30%（屏幕空间）。
            if bucket.in_combat {
                if pid < world.map.adjacencies.len() {
                    let mut best_dx: f32 = 0.0;
                    let mut best_dy: f32 = 0.0;
                    let mut found = false;
                    for &n_raw in &world.map.adjacencies[pid] {
                        let n = n_raw as usize;
                        if n >= max_prov {
                            continue;
                        }
                        let n_owner = world.provinces.owners[n];
                        if n_owner.is_none() || n_owner == bucket.owner {
                            continue;
                        }
                        // 敌方省份（简化：非己方即敌方）
                        let (ncx, ncy) = centroids[n];
                        if ncx == 0.0 && ncy == 0.0 {
                            continue;
                        }
                        let nwx = ncx * world_scale;
                        let nwy_h =
                            sample_height_bilinear(heightmap, ncx, ncy) * height_scale + 0.01;
                        let nwz = ncy * world_scale;
                        let nclip = *view_proj * glam::Vec4::new(nwx, nwy_h, nwz, 1.0);
                        if nclip.w <= 0.0 {
                            continue;
                        }
                        let nsx = (nclip.x / nclip.w * 0.5 + 0.5) * screen_w;
                        let nsy = (1.0 - (nclip.y / nclip.w * 0.5 + 0.5)) * screen_h;
                        best_dx = nsx - sx;
                        best_dy = nsy - sy;
                        found = true;
                        break; // 取第一个找到的敌方省份即可
                    }
                    if found {
                        let lean = 0.15; // 15% 偏移（视觉"探向"敌方）
                        top_x += best_dx * lean;
                        top_y += best_dy * lean;
                    }
                }
            }

            // Keep a wide off-screen margin so counters entering/leaving the
            // viewport do not affect aggregation/layout right at the edge.
            let margin = 512.0;
            if top_x < -size[0] - margin
                || top_x > screen_w + margin
                || top_y < -size[1] - margin
                || top_y > screen_h + margin
            {
                continue;
            }

            // Country color (CR-2.7: tone for visibility)
            let owner_idx = bucket.owner.0 as usize;
            let country_color = if owner_idx < world.countries.count {
                let c = world.countries.colors[owner_idx];
                tone_country_color(c[0], c[1], c[2])
            } else {
                [180, 180, 180, 255]
            };

            // Archetype majority vote
            let mut best_idx = 0usize;
            let mut best_count = 0u32;
            for (idx, &c) in bucket.archetype_votes.iter().enumerate().skip(1) {
                if c > best_count || (c == best_count && c > 0 && idx > best_idx) {
                    best_count = c;
                    best_idx = idx;
                }
            }
            let archetype = if best_count == 0 {
                UnitArchetype::Unknown
            } else {
                UnitArchetype::from_u8(best_idx as u8)
            };

            // Average organisation / strength → 0..255
            let avg_org_norm = if bucket.sum_org_max > 0.0 {
                (bucket.sum_org / bucket.sum_org_max).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let avg_str_norm = if bucket.sum_str_max > 0.0 {
                (bucket.sum_str / bucket.sum_str_max).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let avg_xp = if bucket.count > 0 {
                (bucket.sum_xp / bucket.count as f64).clamp(0.0, 900.0)
            } else {
                0.0
            };
            let exp_level: u8 = if avg_xp >= 600.0 {
                3
            } else if avg_xp >= 300.0 {
                2
            } else if avg_xp >= 100.0 {
                1
            } else {
                0
            };

            let stack_count_clamped = bucket.count.min(255) as u8;

            let mut top_flags: u8 = 0;
            if bucket.in_combat {
                top_flags |= flag_bits::IN_COMBAT;
            }
            if selected_province_ids.contains(&(pid as u32)) {
                top_flags |= flag_bits::SELECTED;
            }
            if bucket.moving {
                top_flags |= flag_bits::MOVING;
            }

            // ── CR-1.5 — Stack underlay layers ────────────────────────────────
            //
            // 顶牌固定在 anchor，底牌向 (+2px, +2px) 错位（"探出右下角"）。
            // 仅在中/近 zoom（size >= 70px）且 stack_count > 1 时显示底牌，
            // 远 zoom 时省略底牌避免视觉混乱。
            let underlay_layers = if size[0] >= 70.0 {
                (bucket.count.saturating_sub(1)).min(3) as u8
            } else {
                0
            };
            if underlay_layers > 0 {
                // 倒序 push：layer 2 → 1 → 0（视觉上 layer 0 离顶牌最近）。
                for layer_idx_inv in 0..underlay_layers {
                    let layer = underlay_layers - 1 - layer_idx_inv; // 2 / 1 / 0
                    let off = (layer + 1) as f32 * 2.0;
                    let mut underlay_flags = flag_bits::IS_UNDERLAY;
                    underlay_flags |=
                        (layer & flag_bits::UNDERLAY_LAYER_MASK) << flag_bits::UNDERLAY_LAYER_SHIFT;
                    out.push(Hoi3CounterInstance {
                        screen_pos: [top_x + off, top_y + off],
                        size,
                        country_color,
                        archetype: archetype as u8,
                        flags: underlay_flags,
                        stack_count: stack_count_clamped,
                        organisation: (avg_org_norm * 255.0).round() as u8,
                        strength: (avg_str_norm * 255.0).round() as u8,
                        experience_level: exp_level,
                        hierarchy_level: 0,
                        _pad0: 0,
                        _pad1: [0.0; 3],
                    });
                }
            }

            // 顶牌
            out.push(Hoi3CounterInstance {
                screen_pos: [top_x, top_y],
                size,
                country_color,
                archetype: archetype as u8,
                flags: top_flags,
                stack_count: stack_count_clamped,
                organisation: (avg_org_norm * 255.0).round() as u8,
                strength: (avg_str_norm * 255.0).round() as u8,
                experience_level: exp_level,
                hierarchy_level: 0,
                _pad0: 0,
                _pad1: [f32::from_bits(pid as u32), 0.0, 0.0],
            });
        } // end for (stack_idx, bucket)
    } // end for (pid_u16, stacks)

    // 2026-05-20 v3：v0 也接屏幕格聚合，避免 cam_distance < 50 的师级视图
    // 在西欧密集战线时一个国家几十张兵牌挤在屏幕上同一像素带。
    // 近 zoom（< 30px）cell 60，中 zoom（30..50）cell 90。
    // v0 是师级视图，保留 archetype 维度（同兵种叠层、不同兵种并排）。
    // Do not screen-merge close division counters. The merge key is based on
    // projected screen distance, so an oblique camera pan can move a pair just
    // across the threshold and make only some counters jump between frames.
    if cam_distance > 45.0 {
        out = merge_by_screen_grid(out, 90.0, true);
    }

    out
}

fn add_division_to_counter_bucket(
    world: &World,
    centroids: &[(f32, f32)],
    visible: Option<&HashSet<CountryId>>,
    spotted: Option<&HashSet<u16>>,
    player: CountryId,
    visual_centroids_override: Option<&HashMap<usize, (f32, f32)>>,
    split_by_corps: bool,
    subunits: &std::collections::HashMap<String, hoi4_data::military::SubunitDef>,
    max_prov: usize,
    div_idx: usize,
    pid: usize,
    bucket_map: &mut HashMap<(u16, u16, u32), CounterBucket>,
) {
    if div_idx >= world.divisions.count || pid >= max_prov {
        return;
    }
    let owner = world.divisions.owners[div_idx];
    let owner_idx = owner.0 as usize;
    if owner.is_none() || owner_idx >= world.countries.count {
        return;
    }
    if world.divisions.locations[div_idx].0 as usize != pid {
        return;
    }
    if let Some(set) = visible {
        let at_war = !player.is_none() && world.diplomacy.at_war_with(player, owner);
        if !crate::units::visibility::is_counter_visible(set, spotted, owner, pid as u16, at_war) {
            return;
        }
    }

    let corps_key = if split_by_corps {
        world
            .command
            .div_to_corps
            .get(div_idx)
            .copied()
            .flatten()
            .map(|c| c as u32)
            .unwrap_or(u32::MAX)
    } else {
        0
    };
    let key = (pid as u16, owner.0, corps_key);

    let bucket = bucket_map.entry(key).or_insert_with(|| CounterBucket {
        count: 0,
        owner,
        province_id: pid as u16,
        archetype_votes: [0; UnitArchetype::COUNT as usize],
        in_combat: false,
        moving: false,
        sum_org: 0.0,
        sum_org_max: 0.0,
        sum_str: 0.0,
        sum_str_max: 0.0,
        sum_xp: 0.0,
        sum_visual_x: 0.0,
        sum_visual_y: 0.0,
        visual_count: 0,
    });

    bucket.count = bucket.count.saturating_add(1);
    let (vcx, vcy) = visual_centroids_override
        .and_then(|overrides| overrides.get(&div_idx).copied())
        .unwrap_or(centroids[pid]);
    if vcx != 0.0 || vcy != 0.0 {
        bucket.sum_visual_x += vcx as f64;
        bucket.sum_visual_y += vcy as f64;
        bucket.visual_count = bucket.visual_count.saturating_add(1);
    }

    let tag = &world.countries.tags[owner_idx];
    if let Some(templates) = world.data.division_templates.get(tag) {
        let tpl_idx = world.divisions.template_indices[div_idx] as usize;
        if let Some(template) = templates.get(tpl_idx) {
            let arch = template_main_archetype(template, subunits);
            bucket.archetype_votes[arch as u8 as usize] += 1;
        }
    }
    if world.divisions.in_combat[div_idx] {
        bucket.in_combat = true;
    }
    if let Some(dest) = world.divisions.destinations[div_idx] {
        if !dest.is_none() {
            bucket.moving = true;
        }
    }

    bucket.sum_org += world
        .divisions
        .organisation
        .get(div_idx)
        .copied()
        .unwrap_or(0.0) as f64;
    bucket.sum_org_max += world
        .divisions
        .max_organisation
        .get(div_idx)
        .copied()
        .unwrap_or(1.0)
        .max(1.0) as f64;
    bucket.sum_str += world
        .divisions
        .strength
        .get(div_idx)
        .copied()
        .unwrap_or(0.0) as f64;
    bucket.sum_str_max += world
        .divisions
        .max_strength
        .get(div_idx)
        .copied()
        .unwrap_or(1.0)
        .max(1.0) as f64;
    bucket.sum_xp += world
        .divisions
        .experience
        .get(div_idx)
        .copied()
        .unwrap_or(0.0) as f64;
}

/// CR-3: Determine hierarchy level from camera distance.
///
/// 2026-05-20 v3：根据实测，cam_distance ~100 已经是全欧视野，旧阈值
/// 150/250/350 永远到不了 corps 级，导致 v0 渲染 143 张顶牌覆盖整个屏幕。
/// 调低到 50/90/160 让远 zoom 自然下沉到 corps/army/army_group 聚合。
pub fn hierarchy_level_for_zoom(cam_distance: f32) -> u8 {
    if cam_distance > 160.0 {
        3
    } else if cam_distance > 90.0 {
        2
    } else if cam_distance > 50.0 {
        1
    } else {
        0
    }
}

/// CR-3: Size bonus per hierarchy level.
///
/// 2026-05-26 V7：基线缩小后同步压缩 hierarchy bonus，
/// 集团军群最大 ≈ 52+24 = 76px，避免覆盖战线。
fn hierarchy_size_bonus(level: u8) -> f32 {
    match level {
        1 => 8.0,
        2 => 16.0,
        3 => 24.0,
        _ => 0.0,
    }
}

/// CR-3: Generate counters at the appropriate hierarchy level based on zoom.
pub fn generate_hoi3_counters_cr3(
    world: &World,
    centroids: &[(f32, f32)],
    world_scale: f32,
    height_scale: f32,
    view_proj: &glam::Mat4,
    screen_w: f32,
    screen_h: f32,
    selected_province_ids: &HashSet<u32>,
    cam_distance: f32,
    visible: Option<&HashSet<CountryId>>,
    spotted: Option<&HashSet<u16>>,
    player: CountryId,
    visual_centroids_override: Option<&HashMap<usize, (f32, f32)>>,
) -> Vec<Hoi3CounterInstance> {
    // 2026-05-20 fix：全球视野（cam_distance > 250）下兵牌彼此重叠到无意义，
    // 数字也会堆成乱码。HOI3/HOI4 原版同样在最远 zoom 隐藏兵牌，仅留地图色。
    // 选中的省份兵牌仍渲染（玩家定位选中部队），其它隐藏。
    if cam_distance > 250.0 && selected_province_ids.is_empty() {
        return Vec::new();
    }

    let target_level = hierarchy_level_for_zoom(cam_distance);
    if target_level == 0 || world.command.corps.is_empty() {
        return generate_hoi3_counters_v0(
            world,
            centroids,
            world_scale,
            height_scale,
            view_proj,
            screen_w,
            screen_h,
            selected_province_ids,
            cam_distance,
            visible,
            spotted,
            player,
            visual_centroids_override,
        );
    }

    let max_prov = centroids.len().min(world.provinces.count);
    if max_prov == 0 || world.divisions.count == 0 || screen_w <= 0.0 || screen_h <= 0.0 {
        return Vec::new();
    }

    let heightmap = &world.map.heightmap;
    let hm_w = heightmap.width;
    let hm_h = heightmap.height;
    let subunits = &world.data.subunits;
    let base_size = size_for_zoom(cam_distance);
    let mut out: Vec<Hoi3CounterInstance> = Vec::new();

    // Project division to screen
    let div_screen = |i: usize| -> Option<(f32, f32)> {
        let loc = world.divisions.locations[i];
        if loc.is_none() {
            return None;
        }
        let pid = loc.0 as usize;
        if pid >= max_prov {
            return None;
        }
        let (cx, cy) = visual_centroids_override
            .and_then(|overrides| overrides.get(&i).copied())
            .unwrap_or(centroids[pid]);
        if cx == 0.0 && cy == 0.0 {
            return None;
        }
        let xi = (cx as u32).min(hm_w.saturating_sub(1));
        let yi = (cy as u32).min(hm_h.saturating_sub(1));
        if hm_w == 0 || hm_h == 0 {
            return None;
        }
        let raw_h = heightmap.pixels[(yi * hm_w + xi) as usize];
        if raw_h <= 95 {
            return None;
        }
        // Match terrain.wgsl's bilinear height sampling; nearest-pixel height
        // leaves a small vertical mismatch that becomes parallax while panning.
        let wy = sample_height_bilinear(heightmap, cx, cy) * height_scale + 0.01;
        let clip = *view_proj * glam::Vec4::new(cx * world_scale, wy, cy * world_scale, 1.0);
        if clip.w <= 0.0 {
            return None;
        }
        let nz = clip.z / clip.w;
        if nz < 0.0 || nz > 1.0 {
            return None;
        }
        Some((
            (clip.x / clip.w * 0.5 + 0.5) * screen_w,
            (1.0 - (clip.y / clip.w * 0.5 + 0.5)) * screen_h,
        ))
    };

    let div_visible = |i: usize| -> bool {
        let owner = world.divisions.owners[i];
        if owner.is_none() {
            return false;
        }
        if let Some(set) = visible {
            let loc = world.divisions.locations[i];
            if loc.is_none() {
                return false;
            }
            let at_war = !player.is_none() && world.diplomacy.at_war_with(player, owner);
            if !crate::units::visibility::is_counter_visible(set, spotted, owner, loc.0, at_war) {
                return false;
            }
        }
        true
    };

    let emit_group = |divs: &[usize], level: u8, out: &mut Vec<Hoi3CounterInstance>| {
        let mut sx_sum = 0.0f64;
        let mut sy_sum = 0.0f64;
        let mut count = 0u32;
        let mut sum_org = 0.0f64;
        let mut sum_org_max = 0.0f64;
        let mut sum_str = 0.0f64;
        let mut sum_str_max = 0.0f64;
        let mut arch_votes = [0u32; UnitArchetype::COUNT as usize];
        let mut owner = CountryId::NONE;
        let mut in_combat = false;
        let mut first_province: u16 = 0;

        for &di in divs {
            if di >= world.divisions.count {
                continue;
            }
            if !div_visible(di) {
                continue;
            }
            if let Some((sx, sy)) = div_screen(di) {
                sx_sum += sx as f64;
                sy_sum += sy as f64;
                if count == 0 {
                    let loc = world.divisions.locations[di];
                    if !loc.is_none() {
                        first_province = loc.0;
                    }
                }
                count += 1;
                owner = world.divisions.owners[di];
                if world.divisions.in_combat[di] {
                    in_combat = true;
                }
                sum_org += world.divisions.organisation.get(di).copied().unwrap_or(0.0) as f64;
                sum_org_max += world
                    .divisions
                    .max_organisation
                    .get(di)
                    .copied()
                    .unwrap_or(1.0)
                    .max(1.0) as f64;
                sum_str += world.divisions.strength.get(di).copied().unwrap_or(0.0) as f64;
                sum_str_max += world
                    .divisions
                    .max_strength
                    .get(di)
                    .copied()
                    .unwrap_or(1.0)
                    .max(1.0) as f64;
                let oi = owner.0 as usize;
                if oi < world.countries.count {
                    let tag = &world.countries.tags[oi];
                    if let Some(templates) = world.data.division_templates.get(tag) {
                        let ti = world.divisions.template_indices[di] as usize;
                        if let Some(tpl) = templates.get(ti) {
                            let a = crate::units::template_main_archetype(tpl, subunits);
                            arch_votes[a as u8 as usize] += 1;
                        }
                    }
                }
            }
        }
        if count == 0 {
            return;
        }

        let cx = (sx_sum / count as f64) as f32;
        let cy = (sy_sum / count as f64) as f32;
        let bonus = hierarchy_size_bonus(level);
        let size = [base_size[0] + bonus, base_size[1] + bonus];
        let top_x = cx - size[0] * 0.5;
        let top_y = cy - size[1] * 0.5;
        let margin = 512.0;
        if top_x < -size[0] - margin
            || top_x > screen_w + margin
            || top_y < -size[1] - margin
            || top_y > screen_h + margin
        {
            return;
        }

        let oi = owner.0 as usize;
        let country_color = if oi < world.countries.count {
            let c = world.countries.colors[oi];
            tone_country_color(c[0], c[1], c[2])
        } else {
            [180, 180, 180, 255]
        };

        let mut best_arch = 0usize;
        for (idx, &v) in arch_votes.iter().enumerate().skip(1) {
            if v > arch_votes[best_arch] {
                best_arch = idx;
            }
        }
        let archetype = if arch_votes[best_arch] == 0 {
            0u8
        } else {
            best_arch as u8
        };

        let org_norm = if sum_org_max > 0.0 {
            (sum_org / sum_org_max).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let str_norm = if sum_str_max > 0.0 {
            (sum_str / sum_str_max).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let mut flags: u8 = 0;
        if in_combat {
            flags |= flag_bits::IN_COMBAT;
        }
        if selected_province_ids.contains(&(first_province as u32)) {
            flags |= flag_bits::SELECTED;
        }

        out.push(Hoi3CounterInstance {
            screen_pos: [top_x, top_y],
            size,
            country_color,
            archetype,
            flags,
            stack_count: count.min(255) as u8,
            organisation: (org_norm * 255.0).round() as u8,
            strength: (str_norm * 255.0).round() as u8,
            experience_level: 0,
            hierarchy_level: level,
            _pad0: 0,
            _pad1: [f32::from_bits(first_province as u32), 0.0, 0.0],
        });
    };

    // Unattached divisions → level 0 individual counters
    for i in 0..world.divisions.count {
        if i >= world.command.div_to_corps.len() || world.command.div_to_corps[i].is_none() {
            if div_visible(i) {
                emit_group(&[i], 0, &mut out);
            }
        }
    }

    // Grouped counters at target level
    match target_level {
        1 => {
            for corps in &world.command.corps {
                emit_group(&corps.divisions, 1, &mut out);
            }
        }
        2 => {
            for army in &world.command.armies {
                let divs: Vec<usize> = army
                    .corps
                    .iter()
                    .filter_map(|&ci| world.command.corps.get(ci))
                    .flat_map(|c| c.divisions.iter().copied())
                    .collect();
                emit_group(&divs, 2, &mut out);
            }
        }
        3 | _ => {
            for ag in &world.command.army_groups {
                let divs: Vec<usize> = ag
                    .armies
                    .iter()
                    .filter_map(|&ai| world.command.armies.get(ai))
                    .flat_map(|a| a.corps.iter())
                    .filter_map(|&ci| world.command.corps.get(ci))
                    .flat_map(|c| c.divisions.iter().copied())
                    .collect();
                emit_group(&divs, 3, &mut out);
            }
        }
    }

    // ── 2026-05-20 fix —— 屏幕空间二次聚合 ─────────────────────────────
    //
    // hierarchy=1 的 Corps 是按 "5 个 division 一组" 顺序分的，西欧大国 30+
    // corps 投影到很近的屏幕区域时仍然密密麻麻盖满地图（见用户截图 2）。
    // 这里在 emit 之后再做一次"按 owner + 屏幕格 (cell_px) 合并"——同一国家在
    // 同一屏幕格里的兵牌全部合并成一个，stack_count 累加，screen_pos 取平均。
    //
    // cell_px 随 zoom 拉大：远视图 cell 大（聚合更激进），近视图 cell 小。
    // 2026-05-20 v3：阈值跟 hierarchy_level_for_zoom 同步（>160=AG, >90=Army, 其它=Corps）。
    // 2026-05-20 v5：远 zoom（army+/army_group）丢弃 archetype 维度强制合并成一摞，
    // 中 zoom（corps）保留 archetype 维度让不同兵种并排显示。
    let merge_cell_px: f32 = if cam_distance > 160.0 {
        240.0
    } else if cam_distance > 90.0 {
        180.0
    } else {
        130.0
    };
    let preserve_archetype = cam_distance <= 90.0;
    out = merge_by_screen_grid(out, merge_cell_px, preserve_archetype);

    out
}

/// 屏幕空间距离合并：同 owner（country_color 代理键）+ 同 archetype + 相距足够近
/// 的兵牌合并成一摞 HOI3 风格的卡牌堆（顶牌 + 错位底牌）。
///
/// 用于 `generate_hoi3_counters_cr3` 远 zoom 视图收口，避免大国 corps 过密。
/// **仅合并顶牌**（`IS_UNDERLAY == 0`）；既有底牌会被丢弃，按合并后的 stack_count
/// 重新生成。
///
/// `merge_archetype = false`：丢弃 archetype 维度，同 owner+cell 全部合并成一摞
/// （远 zoom 用，避免一个国家在同 cell 出现 N 个兵种的 N 摞重叠卡牌）。
fn merge_by_screen_grid(
    counters: Vec<Hoi3CounterInstance>,
    cell_px: f32,
    merge_archetype: bool,
) -> Vec<Hoi3CounterInstance> {
    if counters.is_empty() || cell_px <= 0.0 {
        return counters;
    }
    // 2026-05-22 fix：旧版按绝对屏幕 cell 分组，左键拖地图时兵牌整体平移，
    // 只要跨过 cell 边界就会在相邻帧拆/合，表现为闪烁抽搐。改为按兵牌之间的
    // 相对屏幕距离分组；整体平移不会改变距离，因此拖动时分组稳定。
    type Key = (u32, u8);
    let color_key = |c: &Hoi3CounterInstance| -> u32 { u32::from_le_bytes(c.country_color) };
    let key_for = |c: &Hoi3CounterInstance| -> Key {
        let arch_key = if merge_archetype { c.archetype } else { 0 };
        (color_key(c), arch_key)
    };
    let center = |c: &Hoi3CounterInstance| -> (f32, f32) {
        (
            c.screen_pos[0] + c.size[0] * 0.5,
            c.screen_pos[1] + c.size[1] * 0.5,
        )
    };

    let top_indices: Vec<usize> = counters
        .iter()
        .enumerate()
        .filter(|(_, c)| (c.flags & flag_bits::IS_UNDERLAY) == 0)
        .map(|(i, _)| i)
        .collect();

    let mut used = vec![false; counters.len()];
    let mut groups: Vec<Vec<usize>> = Vec::new();
    let threshold2 = cell_px * cell_px;
    for &i in &top_indices {
        if used[i] {
            continue;
        }
        used[i] = true;
        let seed = &counters[i];
        let seed_key = key_for(seed);
        let (sx, sy) = center(seed);
        let mut group = vec![i];
        for &j in &top_indices {
            if used[j] || key_for(&counters[j]) != seed_key {
                continue;
            }
            let (jx, jy) = center(&counters[j]);
            let dx = jx - sx;
            let dy = jy - sy;
            if dx * dx + dy * dy <= threshold2 {
                used[j] = true;
                group.push(j);
            }
        }
        groups.push(group);
    }

    let mut out: Vec<Hoi3CounterInstance> = Vec::with_capacity(counters.len());
    for idxs in &groups {
        // 计算合并后的代表卡（顶牌）。
        let mut sum_x = 0.0f32;
        let mut sum_y = 0.0f32;
        let mut sum_stack: u32 = 0;
        let mut sum_org: u32 = 0;
        let mut sum_str: u32 = 0;
        let mut max_xp: u8 = 0;
        let mut combined_flags: u8 = 0;
        // 2026-05-20 v5：在 archetype 不参与 key 时，代表卡 archetype 投票选最多的。
        let mut arch_votes: [u32; 16] = [0; 16];
        for &i in idxs {
            let c = &counters[i];
            sum_x += c.screen_pos[0] + c.size[0] * 0.5;
            sum_y += c.screen_pos[1] + c.size[1] * 0.5;
            sum_stack = sum_stack.saturating_add(c.stack_count.max(1) as u32);
            sum_org += c.organisation as u32;
            sum_str += c.strength as u32;
            if c.experience_level > max_xp {
                max_xp = c.experience_level;
            }
            // OR-merge in_combat / selected / moving；丢弃 IS_UNDERLAY / EXPANDED_CHILD 位。
            combined_flags |= c.flags & 0b0000_0111;
            let aidx = (c.archetype as usize).min(15);
            arch_votes[aidx] = arch_votes[aidx].saturating_add(c.stack_count.max(1) as u32);
        }
        let n = idxs.len() as f32;
        let avg_cx = sum_x / n;
        let avg_cy = sum_y / n;
        let mut rep = counters[idxs[0]];
        if !merge_archetype {
            // 选择 stack_count 加权投票最多的 archetype 作为代表卡。
            let mut best = 0usize;
            for (i, &v) in arch_votes.iter().enumerate() {
                if v > arch_votes[best] {
                    best = i;
                }
            }
            rep.archetype = best as u8;
        }
        rep.flags = combined_flags;
        rep.stack_count = sum_stack.min(255) as u8;
        rep.organisation = (sum_org / idxs.len() as u32).min(255) as u8;
        rep.strength = (sum_str / idxs.len() as u32).min(255) as u8;
        rep.experience_level = max_xp;
        let top_x = avg_cx - rep.size[0] * 0.5;
        let top_y = avg_cy - rep.size[1] * 0.5;
        rep.screen_pos = [top_x, top_y];

        // 2026-05-20 v4：HOI3 风格卡牌堆 — 凡是 stack_count ≥ 2 都生成最多 3 张
        // 错位底牌（layer 0..2），向 (+2px, +2px) ×  layer+1 偏移，让卡片看起来
        // "厚一摞"。底牌 push 到顶牌之前（GPU 后画顶牌 → 顶牌叠在最上）。
        // 仅在兵牌足够大（>= 50 px 宽）时生成，远 zoom 兵牌太小会糊成黑块。
        if rep.size[0] >= 50.0 {
            let underlay_layers = (rep.stack_count.saturating_sub(1)).min(3);
            for layer_idx_inv in 0..underlay_layers {
                let layer = underlay_layers - 1 - layer_idx_inv;
                let off = (layer + 1) as f32 * 2.0;
                let mut underlay_flags = flag_bits::IS_UNDERLAY;
                underlay_flags |=
                    (layer & flag_bits::UNDERLAY_LAYER_MASK) << flag_bits::UNDERLAY_LAYER_SHIFT;
                let mut underlay = rep;
                underlay.screen_pos = [top_x + off, top_y + off];
                underlay.flags = underlay_flags;
                out.push(underlay);
            }
        }

        out.push(rep);
    }
    out
}

/// CR-1.6 — 屏幕空间投影顶层中心（用于 text_pass 上层渲染堆叠数）。
///
/// 与 GPU VS 内的 `screen_pos + size/2` 等价；返回 `(center_x, center_y, size_x, size_y, stack_count)`，
/// 物理像素，**不**做 dpi 换算（调用者按需 `* (1/dpi)` → 逻辑像素）。
/// 仅返回顶牌（`flags.IS_UNDERLAY == 0`），按出现顺序保留——主循环可用 `take(N)` 限流。
pub fn collect_top_counter_screen_centers(
    counters: &[Hoi3CounterInstance],
) -> Vec<(f32, f32, f32, f32, u8)> {
    counters
        .iter()
        .filter(|c| (c.flags & flag_bits::IS_UNDERLAY) == 0)
        .map(|c| {
            (
                c.screen_pos[0] + c.size[0] * 0.5,
                c.screen_pos[1] + c.size[1] * 0.5,
                c.size[0],
                c.size[1],
                c.stack_count,
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instance_size_is_40_bytes() {
        assert_eq!(std::mem::size_of::<Hoi3CounterInstance>(), 40);
    }

    #[test]
    fn instance_alignment_is_4_bytes() {
        assert_eq!(std::mem::align_of::<Hoi3CounterInstance>(), 4);
    }

    #[test]
    fn zeroed_is_safe() {
        let z = Hoi3CounterInstance::zeroed();
        assert_eq!(z.screen_pos, [0.0, 0.0]);
        assert_eq!(z.size, [0.0, 0.0]);
        assert_eq!(z.flags, 0);
        assert_eq!(z.stack_count, 0);
        assert_eq!(z.archetype, 0);
        assert_eq!(z.hierarchy_level, 0);
    }

    #[test]
    fn flag_bits_distinct() {
        // bit0..bit4 不重叠
        assert_eq!(flag_bits::SELECTED, 1);
        assert_eq!(flag_bits::IN_COMBAT, 2);
        assert_eq!(flag_bits::MOVING, 4);
        assert_eq!(flag_bits::EXPANDED_CHILD, 8);
        assert_eq!(flag_bits::IS_UNDERLAY, 16);
        // layer 索引位与底牌标志不重叠
        let layer_mask = flag_bits::UNDERLAY_LAYER_MASK << flag_bits::UNDERLAY_LAYER_SHIFT;
        assert_eq!(layer_mask & flag_bits::IS_UNDERLAY, 0);
    }

    #[test]
    fn empty_world_yields_empty() {
        // 不构造完整 World—— 调用 generate_hoi3_counters_v0 至少有 division.count == 0
        // 的分支保护。这里只 sanity-check `collect_top_counter_screen_centers`。
        let centers = collect_top_counter_screen_centers(&[]);
        assert!(centers.is_empty());
    }

    #[test]
    fn collect_top_centers_filters_underlay() {
        let underlay = Hoi3CounterInstance {
            screen_pos: [10.0, 10.0],
            size: [80.0, 52.0],
            country_color: [255, 0, 0, 255],
            archetype: 1,
            flags: flag_bits::IS_UNDERLAY,
            stack_count: 3,
            organisation: 200,
            strength: 200,
            experience_level: 1,
            hierarchy_level: 0,
            _pad0: 0,
            _pad1: [0.0; 3],
        };
        let top = Hoi3CounterInstance {
            screen_pos: [10.0, 10.0],
            size: [80.0, 52.0],
            country_color: [255, 0, 0, 255],
            archetype: 1,
            flags: 0,
            stack_count: 3,
            organisation: 200,
            strength: 200,
            experience_level: 1,
            hierarchy_level: 0,
            _pad0: 0,
            _pad1: [0.0; 3],
        };
        let centers = collect_top_counter_screen_centers(&[underlay, top]);
        assert_eq!(centers.len(), 1);
        assert_eq!(centers[0], (50.0, 36.0, 80.0, 52.0, 3));
    }
}
