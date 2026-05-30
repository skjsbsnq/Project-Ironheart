# Phase 3.1 — 原版 Shader 清单调研

> **Historical note (Phase 11, 2026-05-30):** This audit is archived. Current map renderer pass order, shader ownership, fallback policy, and contribution workflow live in [`../MAP_RENDERER_V2_REFACTOR_ROADMAP.md`](../MAP_RENDERER_V2_REFACTOR_ROADMAP.md), [`map_renderer_v2.md`](./map_renderer_v2.md), and [`map_renderer_v2_contributing.md`](./map_renderer_v2_contributing.md).

## vanilla `gfx/FX/` 文件清单（60 个）

### 按用途归类

#### 地形 / 地图（核心视觉）
| 原版文件 | 大小 | 用途 |
|---|---|---|
| `pdxmap.shader` | 16 KB | 主地形渲染（terrain atlas 多层混合 + hillshade + fog） |
| `pdxmap.fxh` | 174 B | 地图公共 include |
| `pdxwater.shader` | 12 KB | 水面（深度渐变 + 反射 + 波浪 + 海岸泡沫） |
| `river.shader` | 12 KB | 河流（流动动画 + 深度） |
| `border.shader` | 4 KB | 国/省/海岸边界 SDF |
| `tree.shader` | 13 KB | 树木（顶点风摆 + billboard LOD + 季节） |
| `sky.shader` | 1.3 KB | 天空盒 |
| `strait.shader` | 2 KB | 海峡线 |

#### 地图标注 / 箭头
| 原版文件 | 大小 | 用途 |
|---|---|---|
| `mapname.shader` | 2.3 KB | 国家名 3D 文字（沿地形曲面） |
| `maparrow.shader` | 21 KB | 进攻箭头 / 移动箭头（路径动画） |
| `arrow.shader` | 5.3 KB | 简单箭头 |
| `traderoute.shader` | 4 KB | 贸易路线 |

#### 单位 / 模型
| 原版文件 | 大小 | 用途 |
|---|---|---|
| `pdxmesh.shader` | 27 KB | 通用 3D 模型（建筑/舰船/飞机/坦克） |
| `particle.shader` | 6.8 KB | 粒子特效（爆炸/烟雾） |
| `shadow.fxh` | 1.7 KB | 阴影贴图 |

#### GUI / UI
| 原版文件 | 大小 | 用途 |
|---|---|---|
| `buttonstate.shader` | 4.9 KB | 按钮状态（hover/press/disable） |
| `buttonstate.fxh` | 243 B | 按钮公共 include |
| `buttonstate_blendframes.shader` | 2.5 KB | 帧混合按钮 |
| `buttonstate_fade_frames_to_black.shader` | 2.6 KB | 渐黑按钮 |
| `buttonstate_linear.shader` | 2.4 KB | 线性按钮 |
| `buttonstate_nodowneffect.shader` | 2.3 KB | 无按下效果 |
| `buttonstate_nodownordisableeffect.shader` | 1.7 KB | 无按下/禁用效果 |
| `buttonstate_onlydisable.shader` | 2.5 KB | 仅禁用效果 |
| `buttonstate_rendertarget.shader` | 1.3 KB | 渲染目标按钮 |
| `static_button.shader` | 1.5 KB | 静态按钮 |
| `text.shader` | 1.7 KB | 文字渲染 |
| `circularprogressbar.shader` | 1.6 KB | 圆形进度条 |
| `progress.shader` | 1.7 KB | 进度条 |
| `progress_minmax.shader` | 1.9 KB | 最小最大进度条 |
| `progress_radial.shader` | 1.8 KB | 径向进度条 |
| `progress_reverse.shader` | 2.3 KB | 反向进度条 |
| `progress_startend.shader` | 2.2 KB | 起止进度条 |
| `linechart.shader` | 1.6 KB | 折线图 |
| `maskedflag.shader` | 2.8 KB | 国旗遮罩 |
| `coa_shield.shader` | 5.4 KB | 国徽盾形 |
| `portrait.shader` | 2.5 KB | 人物肖像 |

#### 后处理
| 原版文件 | 大小 | 用途 |
|---|---|---|
| `bloom.shader` | 3.5 KB | 泛光 |
| `downsample.shader` | 1.6 KB | 降采样 |
| `downsample_luminance.shader` | 3 KB | 亮度降采样 |
| `lut_blender.shader` | 2.4 KB | 色彩 LUT 混合 |
| `restorescene.shader` | 5.6 KB | 场景还原（tonemap + vignette） |
| `saturation_slider.shader` | 4.7 KB | 饱和度调节 |
| `shadowblur.shader` | 2.7 KB | 阴影模糊 |

#### 公共库
| 原版文件 | 大小 | 用途 |
|---|---|---|
| `standardfuncsgfx.fxh` | 45 KB | 公共函数库（光照/雾/采样/噪声） |
| `constants.fxh` | 12 KB | 全局常量 |
| `fow.fxh` | 3.7 KB | 战争迷雾 |
| `tiled_pointlights.fxh` | 1.7 KB | 点光源 |
| `posteffect_base.fxh` | 232 B | 后处理基础 |
| `sprite_animation.fxh` | 4.2 KB | sprite 动画 |
| `fade_frames_to_black_constants.fxh` | 189 B | 渐黑常量 |
| `defines_glsl.fxh` | 2.5 KB | GLSL 定义 |
| `defines_hlsl.fxh` | 718 B | HLSL 定义 |
| `defines_hlsl_dx11.fxh` | 2.1 KB | DX11 HLSL 定义 |

#### 其他
| 原版文件 | 大小 | 用途 |
|---|---|---|
| `color.shader` | 733 B | 纯色 |
| `simple.shader` | 1.5 KB | 简单纹理 |
| `DebugLines.shader` | 788 B | 调试线 |
| `DebugTexture.shader` | 1.1 KB | 调试纹理 |

---

## 本机已有 wgsl 实现（5 个）

| 文件 | 覆盖范围 |
|---|---|
| `shader.wgsl` | 地形 mesh + hillshade + 三平面噪声 + 雪线 + 海岸沙带 + SDF 边界 + 水面 fbm + 海岸泡沫 + Phong + 程序云层 + 昼夜循环 + 季节 + 政治色 LUT + 占领色 |
| `trees.wgsl` | 树木 billboard（无风摆） |
| `railways.wgsl` | 铁路 LineList |
| `units.wgsl` | 🔄 替换为 procedural counter_v3.wgsl（HOI3 风格） |
| `frontlines.wgsl` | 前线 LineList |

另有 `ui_pass.rs` 内嵌 wgsl（2D sprite quad + alpha blend）。

---

## 差距分析

### P0 — 视觉影响最大，必须做

| 原版 | 当前状态 | 差距 |
|---|---|---|
| `pdxmap.shader` (terrain atlas) | 程序化噪声 + hillshade | **无真实 terrain atlas 采样**；颜色全靠程序生成，不像原版 |
| `pdxwater.shader` | fbm 波浪 + 泡沫 | 缺深度渐变 LUT + 反射 cubemap + 海岸线渐变 |
| `mapname.shader` | 无 | **完全没有**；原版地图上的大字国名是核心视觉 |
| `text.shader` | `ui_pass.rs` 内嵌 | 仅 UI sprite，无 glyph atlas 文字渲染 |
| `restorescene.shader` (tonemap+vignette) | 无 | 缺 post-processing pass |

### P1 — 明显差异但不阻塞可玩

| 原版 | 当前状态 | 差距 |
|---|---|---|
| `border.shader` | SDF 边界已有 | 参数需调优（线宽/颜色/LOD） |
| `buttonstate*.shader` (10 个) | `ui_pass.rs` 无状态 | 按钮无 hover/press/disable 视觉反馈 |
| `pdxmesh.shader` | 无 | 3D 模型无法渲染（Phase 7） |
| `maparrow.shader` | 前线 LineList | 缺贝塞尔箭头 + 动画 |
| `coa_shield.shader` / `maskedflag.shader` | 无 | 国旗/国徽无遮罩染色 |
| `river.shader` | 无 | 河流不可见 |

### P2 — 锦上添花

| 原版 | 当前状态 | 差距 |
|---|---|---|
| `tree.shader` (风摆) | billboard 无动画 | 缺顶点风摆 |
| `bloom.shader` + `downsample*.shader` | 无 | 缺泛光后处理 |
| `particle.shader` | 无 | 缺粒子特效 |
| `sky.shader` | 程序云层 | 缺天空盒纹理 |
| `shadow.fxh` + `shadowblur.shader` | 无 | 缺阴影 |
| `traderoute.shader` | 无 | 贸易路线不可见 |

---

## 结论

60 个原版 shader 文件中：
- **5 个已有等价 wgsl**（terrain/trees/railways/units/frontlines）
- **5 个 P0 必做**（terrain atlas / water 升级 / mapname / text / post-processing）
- **~15 个 P1**（borders 调参 / buttons / mesh / arrows / flags / rivers）
- **~15 个 P2**（bloom / particles / shadows / sky / trade）
- **~20 个 GUI 变体**（buttonstate 系列 + progress 系列 — 功能相似，可合并为 2-3 个 wgsl）

Phase 3.3 应优先实现 P0 的 5 个 shader 等价物。
