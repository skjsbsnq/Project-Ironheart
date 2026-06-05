// Phase I — CR-1.3 + CR-2.1 — HOI3 风格屏幕空间兵牌 shader。
//
// 视觉范围（CR-2.1 完成后）：
//   * 圆角矩形 SDF（半径 6px）
//   * 国家底色填充 + 顶部高光渐变（白色，强度 0.35 → 0.0）
//   * 1px 黑色内描边
//   * 兵种 atlas 采样（@group(1)），染色到高亮度对比色覆盖在底色上
//   * 叠层底牌（flags.IS_UNDERLAY）：仅画底框 + 高光，**不采样 atlas**
//
// 后续阶段扩展：
//   * CR-2.2 — org & str bar
//   * CR-2.3 — 选中金框 / 战斗红边 / 移动箭头
//   * CR-2.4 — 经验星标
//   * CR-3   — 顶部 NATO 规模线（III / XXX / XXXX）

struct Uniforms {
    // x=screen_w, y=screen_h（物理像素，与 Hoi3CounterInstance.screen_pos 一致）。
    screen_size: vec2<f32>,
    opacity: f32,
    time_secs: f32,
    view_proj: mat4x4<f32>,
};

@group(0) @binding(0) var<uniform> u: Uniforms;

// CR-2.1.3 — 兵种 icon atlas（1024×64 RGBA8，16 cell × 64 px）。
// `texture_2d<f32>`：因为我们以 sRGB 视图上传，采样自动 gamma 解码。
@group(1) @binding(0) var counter_atlas: texture_2d<f32>;
@group(1) @binding(1) var counter_atlas_sampler: sampler;

struct VsIn {
    // 顶点：corner ∈ [0,1]²（左上 / 右下展开 6 顶点 quad）。
    @location(0) corner: vec2<f32>,
    // ── instance attributes ─────────────────────────────────────────────
    @location(1) screen_pos: vec2<f32>,        // 像素左上锚点
    @location(2) size: vec2<f32>,              // 像素尺寸
    @location(3) country_color: vec4<f32>,     // Unorm8x4 → 0..1
    // archetype | flags | stack_count | organisation（Uint8x4）
    @location(4) state_pack: vec4<u32>,
    // strength | experience_level | hierarchy_level | _pad0（Uint8x4）
    @location(5) hierarchy_pack: vec4<u32>,
    @location(6) screen_offset: vec2<f32>,
    @location(7) world_pos: vec3<f32>,
    @location(8) motion_delta: vec3<f32>,
    @location(9) motion_times: vec2<f32>,
};

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,                  // [0,1]² counter 内坐标
    @location(1) country_color: vec4<f32>,
    @location(2) @interpolate(flat) state_pack: vec4<u32>,
    @location(3) @interpolate(flat) hierarchy_pack: vec4<u32>,
    @location(4) size_px: vec2<f32>,
};

@vertex
fn vs_main(in: VsIn) -> VsOut {
    var motion_t = 0.0;
    if (in.motion_times.y > 0.001) {
        motion_t = clamp((u.time_secs - in.motion_times.x) / in.motion_times.y, 0.0, 1.0);
    }
    let world = in.world_pos + in.motion_delta * motion_t;
    let anchor_clip = u.view_proj * vec4<f32>(world, 1.0);
    var anchor_px = vec2<f32>(-100000.0, -100000.0);
    if (anchor_clip.w > 0.0) {
        let ndc_anchor = anchor_clip.xyz / anchor_clip.w;
        if (ndc_anchor.z >= 0.0 && ndc_anchor.z <= 1.0) {
            anchor_px = vec2<f32>(
                (ndc_anchor.x * 0.5 + 0.5) * max(u.screen_size.x, 1.0),
                (1.0 - (ndc_anchor.y * 0.5 + 0.5)) * max(u.screen_size.y, 1.0)
            );
        }
    }
    let pixel = anchor_px + in.screen_offset + in.corner * in.size;
    // 屏幕像素 → NDC（Y 翻转：屏幕 Y 向下，NDC Y 向上）。
    let ndc = vec2<f32>(
        (pixel.x / max(u.screen_size.x, 1.0)) * 2.0 - 1.0,
        1.0 - (pixel.y / max(u.screen_size.y, 1.0)) * 2.0
    );
    var out: VsOut;
    // 兵牌位于屏幕空间，永远在最近平面附近（z=0），depth_compare 关 / 深度只读。
    out.clip = vec4<f32>(ndc, 0.0, 1.0);
    out.uv = in.corner;
    out.country_color = in.country_color;
    out.state_pack = in.state_pack;
    out.hierarchy_pack = in.hierarchy_pack;
    out.size_px = in.size;
    return out;
}

// 圆角矩形 SDF（中心在 (0,0)，半宽半高 = `half`，圆角半径 r）。
// 返回值：< 0 = 内部，= 0 = 边界，> 0 = 外部。
fn sdf_rounded_rect(p: vec2<f32>, half: vec2<f32>, r: f32) -> f32 {
    let q = abs(p) - (half - vec2<f32>(r));
    let outside = max(q, vec2<f32>(0.0));
    return length(outside) + min(max(q.x, q.y), 0.0) - r;
}

// CR-2.1.3 — 把 `uv.x ∈ [0,1]` 的 counter 横向坐标映射到 atlas 中
// archetype 索引 `idx` 对应的 cell 范围（`u0..u1`）。
fn atlas_u_for(idx: u32, local_x: f32) -> f32 {
    let cell_w = 1.0 / 16.0;
    let u0 = f32(idx) * cell_w;
    return u0 + local_x * cell_w;
}

fn digit_segments(d: u32) -> u32 {
    // bits: top, upper-right, lower-right, bottom, lower-left, upper-left, middle.
    if (d == 0u) { return 0x3fu; }
    if (d == 1u) { return 0x06u; }
    if (d == 2u) { return 0x5bu; }
    if (d == 3u) { return 0x4fu; }
    if (d == 4u) { return 0x66u; }
    if (d == 5u) { return 0x6du; }
    if (d == 6u) { return 0x7du; }
    if (d == 7u) { return 0x07u; }
    if (d == 8u) { return 0x7fu; }
    if (d == 9u) { return 0x6fu; }
    return 0u;
}

fn in_rect(p: vec2<f32>, x0: f32, y0: f32, x1: f32, y1: f32) -> bool {
    return p.x >= x0 && p.x <= x1 && p.y >= y0 && p.y <= y1;
}

fn seven_seg_digit_mask(d: u32, p: vec2<f32>, scale: f32) -> f32 {
    let bits = digit_segments(d);
    let w = 5.0 * scale;
    let h = 7.0 * scale;
    let t = max(0.9 * scale, 0.75);
    var on = false;
    if ((bits & 0x01u) != 0u) { on = on || in_rect(p, t, 0.0, w - t, t); }
    if ((bits & 0x02u) != 0u) { on = on || in_rect(p, w - t, t, w, h * 0.5 - t * 0.35); }
    if ((bits & 0x04u) != 0u) { on = on || in_rect(p, w - t, h * 0.5 + t * 0.35, w, h - t); }
    if ((bits & 0x08u) != 0u) { on = on || in_rect(p, t, h - t, w - t, h); }
    if ((bits & 0x10u) != 0u) { on = on || in_rect(p, 0.0, h * 0.5 + t * 0.35, t, h - t); }
    if ((bits & 0x20u) != 0u) { on = on || in_rect(p, 0.0, t, t, h * 0.5 - t * 0.35); }
    if ((bits & 0x40u) != 0u) { on = on || in_rect(p, t, h * 0.5 - t * 0.5, w - t, h * 0.5 + t * 0.5); }
    return select(0.0, 1.0, on);
}

fn stack_number_mask(value: u32, local_px: vec2<f32>, size_px: vec2<f32>) -> f32 {
    let display = min(value, 99u);
    let scale = clamp(size_px.x / 58.0, 0.60, 1.20);
    let digit_w = 5.0 * scale;
    let digit_h = 7.0 * scale;
    let gap = 1.3 * scale;
    var total_w = digit_w;
    if (display >= 10u) {
        total_w = digit_w * 2.0 + gap;
    }
    let origin = vec2<f32>(size_px.x - total_w - 4.0, 4.0);
    var mask = 0.0;
    if (display >= 10u) {
        mask = max(mask, seven_seg_digit_mask(display / 10u, local_px - origin, scale));
        mask = max(mask, seven_seg_digit_mask(display % 10u, local_px - origin - vec2<f32>(digit_w + gap, 0.0), scale));
    } else {
        mask = seven_seg_digit_mask(display, local_px - origin, scale);
    }
    return mask;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    // counter 局部坐标，原点在中心。
    let half = in.size_px * 0.5;
    let local = (in.uv - vec2<f32>(0.5, 0.5)) * in.size_px;

    let radius = 6.0;
    let dist = sdf_rounded_rect(local, half, radius);

    // 1 像素 anti-alias 软边。
    let aa = 1.0;
    let alpha_rect = clamp(0.5 - dist / aa, 0.0, 1.0);
    if (alpha_rect <= 0.001) {
        discard;
    }

    let flags = in.state_pack.y;
    let is_underlay = (flags & 16u) != 0u; // bit4 = IS_UNDERLAY

    // V7 视觉优化：顶部高光 0.35→0.20（更柔和）
    let top_t = 1.0 - clamp(in.uv.y, 0.0, 1.0);
    let top_highlight = top_t * 0.20;
    var rgb = mix(in.country_color.rgb, vec3<f32>(1.0, 1.0, 1.0), top_highlight);

    if (is_underlay) {
        // 叠层底牌：layer 0..3，alpha 0.85 / 0.65 / 0.45 / 0.30（保留兜底）。
        let layer = (flags >> 5u) & 3u; // bits 5..6
        var underlay_alpha: f32 = 0.85;
        if (layer == 1u) {
            underlay_alpha = 0.65;
        } else if (layer == 2u) {
            underlay_alpha = 0.45;
        } else if (layer == 3u) {
            underlay_alpha = 0.30;
        }
        // 1px 暗内描边让底牌之间有可见的卡牌缝。
        let edge_u = clamp((dist + 1.0) * (-1.0), 0.0, 1.0);
        rgb = mix(rgb, vec3<f32>(0.05, 0.05, 0.05), edge_u * 0.6);
        // 底牌不画 atlas、bar、数字 —— 仅纯色卡片 + 高光 + 暗边。
        return vec4<f32>(rgb, alpha_rect * underlay_alpha * u.opacity);
    }

    // 顶牌：1px 黑色内描边，V7 强度 0.85→0.55（更轻盈）。
    let edge = clamp((dist + 1.0) * (-1.0), 0.0, 1.0);
    rgb = mix(rgb, vec3<f32>(0.05, 0.05, 0.05), edge * 0.55);

    // ── CR-2.3：战斗红边 / 选中金框 ────────────────────────────────────
    let is_combat  = false;
    let is_selected = (flags & 1u) != 0u; // bit0

    // 选中金框：2px 金色描边 + 1.5px 外发光（SDF smoothstep）。
    if (is_selected) {
        let gold = vec3<f32>(1.0, 0.847, 0.376); // #ffd860
        // 内描边 2px 带（dist ∈ [-3, -1]）
        let sel_band = smoothstep(-3.5, -3.0, dist) * (1.0 - smoothstep(-1.0, -0.5, dist));
        rgb = mix(rgb, gold, sel_band * 0.95);
        // 外发光 1.5px（dist ∈ [0, 1.5]）
        let sel_glow = 1.0 - smoothstep(0.0, 1.5, dist);
        rgb = mix(rgb, gold, sel_glow * 0.4);
    }

    // 战斗红边：2px 红色外框发光（dist ∈ [-1, 3]）。
    if (is_combat) {
        let combat_red = vec3<f32>(0.91, 0.15, 0.15); // #e82626
        let combat_glow = 1.0 - smoothstep(-1.0, 3.0, dist);
        rgb = mix(rgb, combat_red, combat_glow * 0.55);

        // CR-2.3.5：战斗攻击指示 — 左侧红色三角箭头（"前倾"视觉暗示）。
        // 完整 from→to 虚线连线推迟到 CR-4（需独立 line-list pipeline）。
        let combat_local = in.uv * in.size_px;
        let bx = 10.0 - combat_local.x; // 从左边缘向内
        let by = combat_local.y - in.size_px.y * 0.5;
        let in_combat_arrow = bx >= 0.0 && bx <= 8.0 && abs(by) <= (8.0 - bx);
        if (in_combat_arrow) {
            rgb = combat_red;
        }
    }

    // ── CR-2.1.3：兵种 atlas 采样 ──────────────────────────────────────
    //
    // 把 counter 的中央 60% × 60% 矩形分配给 icon（剩余空间留给 CR-2.2 的
    // bar / CR-2.4 经验星标 / CR-2.6 堆叠数徽章）。
    let icon_uv0 = vec2<f32>(0.20, 0.08);
    let icon_uv1 = vec2<f32>(0.80, 0.68);
    let inside_icon = all(in.uv >= icon_uv0) && all(in.uv <= icon_uv1);

    if (inside_icon) {
        let local_icon = (in.uv - icon_uv0) / (icon_uv1 - icon_uv0); // 0..1 在 icon 区域
        let arch = in.state_pack.x;
        let icon_u = atlas_u_for(arch, local_icon.x);
        let icon_v = local_icon.y;
        let icon_sample = textureSample(
            counter_atlas,
            counter_atlas_sampler,
            vec2<f32>(icon_u, icon_v)
        );
        // 对比色：根据底色亮度选黑或白。
        let lum = dot(in.country_color.rgb, vec3<f32>(0.299, 0.587, 0.114));
        var tint = vec3<f32>(0.95, 0.95, 0.95); // 暗底 → 亮色 icon
        if (lum > 0.5) {
            tint = vec3<f32>(0.06, 0.06, 0.06); // 亮底 → 暗色 icon
        }
        // 用 atlas 的 alpha 通道作为 mask（SVG → premultiplied PNG，alpha 即遮罩）。
        rgb = mix(rgb, tint, icon_sample.a);
    }

    // ── CR-2.2：组织度 / 战力条 ────────────────────────────────────────
    //
    // 设计稿规格：50×3 px 水平 bar，底色 `#1a1a1a`，居中横排。
    //   * 上层 = org（绿 #5cb85c，< 30% 变红 #e84040）
    //   * 下层 = str（金 #d9b84a，< 50% 变橙 #e89040）
    // 在 CR-1.5 IS_UNDERLAY 分支已 return — 此处只画顶牌。
    let local_px = in.uv * in.size_px;
    let bar_w_px = 50.0;
    let bar_h_px = 3.0;
    let bar_x0 = (in.size_px.x - bar_w_px) * 0.5;
    let bar_x1 = bar_x0 + bar_w_px;
    // 距底部 13px / 7px（org 在上、str 在下；间距 3 px 避免视觉黏连）
    let org_y0 = in.size_px.y - 13.0;
    let org_y1 = org_y0 + bar_h_px;
    let str_y0 = in.size_px.y - 7.0;
    let str_y1 = str_y0 + bar_h_px;

    let org_norm = f32(in.state_pack.w) / 255.0;
    let str_norm = f32(in.hierarchy_pack.x) / 255.0;

    let bar_bg = vec3<f32>(0.102, 0.102, 0.102);          // #1a1a1a
    let org_normal_fg = vec3<f32>(0.361, 0.722, 0.361);   // #5cb85c
    let org_low_fg    = vec3<f32>(0.910, 0.251, 0.251);   // #e84040
    let str_normal_fg = vec3<f32>(0.851, 0.722, 0.290);   // #d9b84a
    let str_low_fg    = vec3<f32>(0.910, 0.565, 0.251);   // #e89040

    let org_fg = select(org_normal_fg, org_low_fg, org_norm < 0.30);
    let str_fg = select(str_normal_fg, str_low_fg, str_norm < 0.50);

    // Org bar
    let in_org = local_px.x >= bar_x0 && local_px.x < bar_x1
              && local_px.y >= org_y0 && local_px.y < org_y1;
    if (in_org) {
        let progress = (local_px.x - bar_x0) / bar_w_px;
        let filled = progress <= org_norm;
        rgb = select(bar_bg, org_fg, filled);
    }

    // Str bar
    let in_str = local_px.x >= bar_x0 && local_px.x < bar_x1
              && local_px.y >= str_y0 && local_px.y < str_y1;
    if (in_str) {
        let progress = (local_px.x - bar_x0) / bar_w_px;
        let filled = progress <= str_norm;
        rgb = select(bar_bg, str_fg, filled);
    }

    // ── CR-2.4：经验星标 ──────────────────────────────────────────────
    // experience_level: 0=无 / 1=老练(1星) / 2=精锐(2星) / 3=王牌(3星)
    // 右上角，每颗星 6×6 px 横排，从 (size.x - 8 - n*7, 6) 开始
    let exp_level = in.hierarchy_pack.y; // experience_level
    if (exp_level > 0u) {
        let star_gold = vec3<f32>(1.0, 0.847, 0.376); // #ffd860
        let star_count = min(exp_level, 3u);
        let star_local = in.uv * in.size_px;
        for (var si = 0u; si < star_count; si = si + 1u) {
            let cx = in.size_px.x - 10.0 - f32(si) * 7.0;
            let cy = 8.0;
            let dx = star_local.x - cx;
            let dy = star_local.y - cy;
            let r = sqrt(dx * dx + dy * dy);
            // 简化星形：用圆形近似（半径 3px）
            if (r <= 3.0) {
                rgb = mix(rgb, star_gold, 0.9);
            }
        }
    }

    // Stack count is drawn in-shader so high-speed camera movement does not
    // require hundreds of CPU text draws every frame.
    let stack = in.state_pack.z; // stack_count
    if (stack > 0u) {
        let badge_local = in.uv * in.size_px;
        let display = min(stack, 99u);
        let digit_scale = clamp(in.size_px.x / 58.0, 0.60, 1.20);
        let digit_w = 5.0 * digit_scale;
        let digit_h = 7.0 * digit_scale;
        let digit_gap = 1.3 * digit_scale;
        var digits_w = digit_w;
        if (display >= 10u) {
            digits_w = digit_w * 2.0 + digit_gap;
        }
        let bg0 = vec2<f32>(in.size_px.x - digits_w - 6.0, 2.0);
        let bg1 = vec2<f32>(in.size_px.x - 2.0, 4.0 + digit_h + 2.0);
        if (badge_local.x >= bg0.x && badge_local.x <= bg1.x && badge_local.y >= bg0.y && badge_local.y <= bg1.y) {
            rgb = mix(rgb, vec3<f32>(0.05, 0.05, 0.05), 0.62);
        }
        let digit_mask = stack_number_mask(stack, badge_local, in.size_px);
        if (digit_mask > 0.0) {
            let lum = dot(in.country_color.rgb, vec3<f32>(0.299, 0.587, 0.114));
            var digit_color = vec3<f32>(0.98, 0.94, 0.78);
            if (lum > 0.58) {
                digit_color = vec3<f32>(0.06, 0.05, 0.04);
            }
            rgb = mix(rgb, digit_color, 0.96);
        }
    }

    // 移动指示：右侧小三角箭头（金色 #ffd860）。
    let is_moving = (flags & 4u) != 0u; // bit2
    if (is_moving) {
        let arrow_gold = vec3<f32>(1.0, 0.847, 0.376);
        let arrow_local = in.uv * in.size_px;
        // 箭头区域：右侧 10px 内 × 中央 16px 高
        let ax = arrow_local.x - (in.size_px.x - 10.0);
        let ay = arrow_local.y - in.size_px.y * 0.5;
        // 三角形：ax ∈ [0, 8], |ay| ≤ (8 - ax)
        let in_arrow = ax >= 0.0 && ax <= 8.0 && abs(ay) <= (8.0 - ax);
        if (in_arrow) {
            rgb = arrow_gold;
        }
    }

    // ── CR-3: NATO size markers (horizontal lines at top) ────────────────
    let hierarchy_level = in.hierarchy_pack.z;
    if (hierarchy_level > 0u) {
        let marker_local = in.uv * in.size_px;
        let marker_cx = in.size_px.x * 0.5;
        let marker_y_base = 4.0;
        let line_w = 12.0;
        let line_h = 1.5;
        let line_gap = 3.0;
        let line_count = hierarchy_level + 1u; // level 1=2 lines, 2=3, 3=4
        var marker_color = vec3<f32>(0.95, 0.95, 0.95);
        if (hierarchy_level == 3u) {
            marker_color = vec3<f32>(1.0, 0.847, 0.376); // gold for army group
        }
        for (var li = 0u; li < line_count; li = li + 1u) {
            let ly = marker_y_base + f32(li) * line_gap;
            let in_line = abs(marker_local.x - marker_cx) <= line_w * 0.5
                       && marker_local.y >= ly && marker_local.y < ly + line_h;
            if (in_line) {
                rgb = marker_color;
            }
        }
    }

    // ── CR-3: hierarchy-dependent border color ────────────────────────────
    if (hierarchy_level >= 2u) {
        // Army+ gets a thicker gold/white inner border
        let hier_border_color = select(
            vec3<f32>(0.9, 0.9, 0.9),
            vec3<f32>(1.0, 0.847, 0.376),
            hierarchy_level == 3u
        );
        let hier_band = smoothstep(-2.5, -2.0, dist) * (1.0 - smoothstep(-0.5, 0.0, dist));
        rgb = mix(rgb, hier_border_color, hier_band * 0.7);
    }

    // CR-2.3+: 数字 / 经验星 / 战斗箭头见后续阶段。
    return vec4<f32>(rgb, alpha_rect * u.opacity);
}
