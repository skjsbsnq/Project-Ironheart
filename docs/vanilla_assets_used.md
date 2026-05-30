# Vanilla 资产白名单（V5）

> ROADMAP_V5.md §0.1 配套：本文件列出 Project Ironheart V5 允许从用户合法持有的
> HOI4 安装目录读取的所有 vanilla 资产路径。所有 `db.open(...)` /
> `path_cfg.find(...)` / `game_path.join(...)` / `from_system_font(...)` 引用
> 都必须命中本白名单。


## 1. 地图（`map/`）

`hoi4-map` + `hoi4-render` 渲染地形 / 水 / 河 / 树 / 边界等 25 个 pass 必需。

- `map/provinces.bmp` — 省份索引图（13382 省）
- `map/heightmap.bmp` — 高度图
- `map/terrain.bmp` — 地形类型索引
- `map/rivers.bmp` — 河流图
- `map/trees.bmp` — 树木分布
- `map/cities.bmp` — 城市分布
- `map/world_normal.bmp` — 全球法线
- `map/seasons.txt` — 季节配置
- `map/railways.txt` — 铁路网
- `map/default.map` — 地图主配置


### 1.1 `map/terrain/` 渲染贴图

vanilla pdxmap / pdxwater / 边界 / 河流 / 雾 / 大气 等 shader 需要的纹理：

- `map/terrain/atlas{0,1,2}.dds`
- `map/terrain/atlas_normal{0,1,2}.dds`
- `map/terrain/colormap.dds`
- `map/terrain/colormap_rgb.dds`
- `map/terrain/colormap_rgb_cityemissivemask_a.dds`
- `map/terrain/colormap_water_{0,1}.dds`
- `map/terrain/citylights_rgb_snowmask_a_{0,1}.dds`
- `map/terrain/lean1.dds` / `map/terrain/lean2.dds`
- `map/terrain/mud_diffuse_rgb_gloss_a_{0,1}.dds`
- `map/terrain/mud_normal_rgb_spec_a_{0,1}.dds`
- `map/terrain/snow_normal_rgb_diffuse_a.dds`
- `map/terrain/ice_diffuse.dds`
- `map/terrain/ice_noise_{0,1}.dds`
- `map/terrain/RiverSurface_diffuse_{0,1}.dds`
- `map/terrain/RiverSurface_normal_{0,1}.dds`
- `map/terrain/RiverSurface_masks.dds`
- `map/terrain/fow_rgb_waterspec_a.dds`
- `map/terrain/fow_noise_{0,1}.dds`
- `map/terrain/reflection.dds`
- `map/terrain/reflection_land_unit.dds`
- `map/terrain/border_{country,province,state,sea,sea_region,impassable}_{0,1,2}.dds`
- `map/terrain/strait.dds`
- `map/terrain/naval_dominance_fx.dds`
- `map/terrain/underwater_terrain_{0,1}.dds`
- `map/terrain/Tree_season.bmp`
- `map/terrain/Tree_tint.bmp`


## 2. 国家初始化（`history/` + `common/`）

`hoi4-data::loader` 加载世界初始状态需要的脚本文件（V5 §3.4 已停用 focus /
decision / event / scripted_* 加载，下表只剩"国家定义 / 装备 / 科技 / 状态"
四类核心数据）。

- `history/countries/*.txt` — 各国 1936 起手 OOB / ideas / vars / flags
- `history/states/*.txt` — 状态（省份归属 / 工厂 / 资源 / 建筑）
- `history/units/*.txt` — 陆 / 海 / 空 OOB
- `common/country_tags/*.txt` — 国家 tag 列表
- `common/countries/*.txt` — 国家定义（包含 `Germany.txt` 等）
- `common/countries/colors.txt` — 国旗 / 地图色
- `common/ideologies/00_ideologies.txt` — 意识形态表
- `common/ideas/*.txt` — 国家精神 / 顾问 / 法律 / 战略物资政策
- `common/buildings/*.txt` — 建筑类型表
- `common/resources/00_resources.txt` — 资源类型表
- `common/units/*.txt` — 子单位（兵种）定义
- `common/units/equipment/*.txt` — 装备类型表
- `common/technologies/*.txt` — 科技树
- `common/combat_tactics.txt` — 战斗战术
- `common/defines.lua` / `common/defines/00_defines.lua` — 全局常数
- `common/division_templates/*.txt` —（如已在 history/ 下存在 OOB 模板，
  通过 OOB 文件间接读取）


## 3. 视觉资产

### 3.1 国旗 `gfx/flags/`

`hoi4-app::flag_bank` 按 fallback 链尝试：

- `gfx/flags/{TAG}_{ideology}.tga`
- `gfx/flags/{TAG}.tga`
- `gfx/flags/medium/{TAG}_{ideology}.tga`
- `gfx/flags/medium/{TAG}.tga`

### 3.2 NATO 兵牌 `gfx/interface/` + `gfx/interface/counters/divisions_small/`

~~CR-5 已删除：旧 `MapSymbolPass` 及其 vanilla 兵牌纹理依赖已移除。~~
~~以下 18 项资源不再使用（HOI3 风格 procedural counter 零 vanilla 纹理依赖）：~~

~~- `gfx/interface/onmap_unit_counter.dds`（3 帧）~~
~~- `gfx/interface/onmap_unit_counter_ideology.dds`（1 帧）~~
~~- `gfx/interface/onmap_unit_counter_overlay.dds`（4 帧）~~
~~- `gfx/interface/counters/divisions_small/onmap_no_intel_icon.dds`~~
~~- `gfx/interface/counters/divisions_small/onmap_unit_{infantry,cavalry,motorized,mechanized}_icon.dds`~~
~~- `gfx/interface/counters/divisions_small/onmap_unit_{mountain,marine,paratroop,militia}_icon.dds`~~
~~- `gfx/interface/counters/divisions_small/onmap_unit_{light_tank,medium_tank,heavy_armor,modern_armor}_icon.dds`~~
~~- `gfx/interface/counters/divisions_small/onmap_unit_{art,at,anti_air}_icon.dds`~~

### 3.2.bis UI 9-slice 木纹窗口 `gfx/interface/tiles/`

V5 阶段 B.4（2026-05-19）：自研 UI（hoi4-ui）走 vanilla 调性，需要 9-slice
木纹窗口背景纹理。`hoi4_ui::nine_slice::NineSlice::load_vanilla` 通过
`PathConfig::find` 显式相对路径加载，BGRA8 通道交换后注册到
`egui::Context::load_texture`：

- `gfx/interface/tiles/tiled_window.dds`（192×192 BGRA8，stretchy_size=32 px，
  vanilla 主窗口背景；本阶段唯一启用条目）

后续候选（B.5+ 按需启用）：

- `gfx/interface/tiles/tiled_window_thin_border.dds`（190×190 BGRA8，细边框窗口）
- `gfx/interface/tiles/tiled_window_transparent.dds`（95×95 BGRA8，半透明窗口）
- `gfx/interface/tiles/tiled_window_small.dds`（95×95 BGRA8，小窗口背景）
- `gfx/interface/tiles/tiled_window_small_small.dds`（48×48 BC1，最小窗口；
  需要 BC1 解码，B.5 接入时一并加 BC1 → RGBA8 解码）

边宽（`stretchy_size`）原本由 vanilla `frontend.gfx` 定义；V5 §0.2 不解析
`.gfx`，所以由 caller 显式传入 `NineSliceEdges`。

### 3.2.ter UI sprite icon `gfx/interface/goals/`

V5 阶段 B.5（2026-05-19）：自研 UI 用 `hoi4_ui::icons::IconBank` lazy 加载
`GFX_xxx` 命名的 sprite。bank 默认搜索 `gfx/interface/goals/`；命名约定：

- `GFX_focus_GER_anschluss` → strip `GFX_` → 各 search dir 拼接 →
  `gfx/interface/goals/focus_GER_anschluss.dds`

启动时 `hoi4_app::main` 预热 2 个 GER focus 图标（`GFX_focus_GER_anschluss`
+ `GFX_focus_GER_afrikakorps`）作为加载链路烟雾测试，命中 / 失败仅打 banner，
不阻塞启动。

后续阶段（C.2 政治面板 ideas / C.4 决议）会通过
`IconBank::add_search_dir(...)` 增加：

- `gfx/interface/ideas/`（idea 系列图标）
- `gfx/interface/decisions/`（决议图标）

vanilla 真正的 sprite 索引（`.gfx` 中的 `SpriteType` 自定义 frame / 路径）
在 V5 §0.2 禁解析；本 bank 走「strip GFX_ + 同名 DDS」硬编码约定，假设
vanilla 绝大多数 sprite 名都对应同名 DDS（实测 vanilla `goals/focus_GER_*`
全部满足）。

DDS 解码：BGRA8（vanilla 主流）+ BC1（DXT1） + BC3（DXT5）三种格式由
`hoi4_ui::dds_decode::decode_mip0_to_rgba` 统一处理；不支持 BC5 / BC4 / BC7。

### 3.3 天空 cubemap `gfx/loadingscreens/`

`hoi4-app::passes::sky` 6 面体：

- `gfx/loadingscreens/sky_{pos,neg}_{x,y,z}.dds`


### 3.4 3D mesh `gfx/models/`

`hoi4-app::passes::pdxmesh` 工业建筑 + `passes::trees_full` 树木：

- `gfx/models/buildings/civ_factory.mesh`
- `gfx/models/buildings/factory.mesh`
- `gfx/models/buildings/dock_01.mesh`
- `gfx/models/buildings/factory_d.dds`
- `gfx/models/buildings/factory_n.dds`
- `gfx/models/buildings/factory_s.dds`
- `gfx/models/buildings/dock_diffuse.dds`
- `gfx/models/buildings/dock_normal.dds`
- `gfx/models/buildings/dock_specular.dds`
- `gfx/models/mapitems/trees/beech.mesh`
- `gfx/models/mapitems/trees/Pine_01.mesh`
- `gfx/models/mapitems/trees/palmer.mesh`
- `gfx/models/mapitems/trees/beech_diffuse.dds`
- `gfx/models/mapitems/trees/beech_normal.dds`
- `gfx/models/mapitems/trees/pinetree_diffuse.dds`
- `gfx/models/mapitems/trees/pinetree_normal.dds`
- `gfx/models/mapitems/trees/palm_lod_diffuse.dds`
- `gfx/models/mapitems/trees/palmblad_normal.dds`

`pdxmesh` 解析过程中 mesh 内嵌纹理路径会指向 `gfx/models/units/...` 等子目录；
当前实现使用 `normalize_mesh_tex_path` 将相对路径转为绝对，以 mesh 所在目录
为根。所有最终落到磁盘的 DDS 文件都视为命中本节白名单。

### 3.5 Shader 翻译参考 `gfx/FX/`

`hoi4-render::shader_rt` 仅在调试 / 注册表查找时引用 vanilla shader 名（不读字节）：

- `gfx/FX/pdxmap.shader` —— V3 时代 24 个翻译 shader 的命名参考；
  V5 不再追加新翻译，运行时不读这些文件。

### 3.6 资源图标 `gfx/interface/resources/`

（预留 J.2 ProvinceInfoCard 使用）

### 3.7 国家元首肖像 `gfx/leaders/<TAG>/` + `common/characters/*.txt` + `localisation/**/parties_l_*.yml`

V5 阶段 J.1（Country Leader & Portrait）新增：

- `common/characters/*.txt` — 解析 `country_leader` 角色的 `name` + `portraits.civilian.large` + `ideology`。
  仅取 country_leader 子集；advisor / field_marshal / corps_commander 等角色骨架占位不展开。
- `gfx/leaders/<TAG>/*.dds` — 元首肖像 DDS（BC1/BC3，~256×256）。
  `IconBank::add_leader_dirs` 注册每国目录；`try_load` 先精确匹配 `portrait_<TAG>_<name>.dds`，
  失败后 fuzzy 扫描目录（处理 vanilla 老规约 `Portrait_<English>_<Name>.dds`）。
- `localisation/english/parties_l_english.yml` — 党派全称（`<TAG>_<ideology>_party_long`）。
- `localisation/english/*characters*l_english*.yml` — 角色显示名（`GER_adolf_hitler` → "Adolf Hitler"）。

启动时 banner 扫描：对 7 大国（GER/SOV/ENG/FRA/ITA/JAP/USA）验证 country_leader 命中；
缺失则 warning 不致命，降级为"国旗大图 + ideology 名"。


## 4. 音乐 `music/`

`hoi4-render::audio::MusicPlayer` 扫描 `music/` 目录加载 `.ogg`：

- `music/*.ogg` — vanilla 主菜单 / 战时背景音乐
- `music/main_menu.ogg` —（asset_meta 测试中作为示例引用）

V5 阶段 G.2 接入 `*.asset` playlist 后，本节会扩展。

## 5. 字体

V5 收口（§4.4 / §3.5）删除了 vanilla `gfx/fonts/hoi_*.fnt` BmFont 路径。
**事实更正（2026-05-19，B.3 阶段勘察）**：vanilla HOI4 在 `gfx/fonts/`
**不发布任何 `.ttf`**，全部是 BmFont（`.fnt` + `.dds`/`.tga`）。早先 ROADMAP_V5
B.3 行写「vanilla `gfx/fonts/hoi_*.ttf`」属事实错误，已在该行就地更正。

所有文字渲染走 `fontdue` / `egui` + 系统 TTF，**不读 vanilla 字体文件**：

- `glyphon_text::FontdueAtlas::from_system_font` 通过显式系统字体路径加载
  （Windows: `segoeui.ttf` / `msyh.ttc` / `arial.ttf`；Linux: `DejaVuSans` 等）
- `hoi4_ui::theme::apply_vanilla_theme` 加载 Latin 衬线 + CJK fallback：
  - **Latin 主字体**（vanilla Garamond 视觉近似）：
    `C:\Windows\Fonts\georgia.ttf` →
    `C:\Windows\Fonts\palab.ttf` →
    `C:\Windows\Fonts\times.ttf`
    依次回退；macOS `/Library/Fonts/Georgia.ttf`；Linux Liberation Serif。
  - **CJK fallback**（中文 / 日文 / 韩文）：
    `C:\Windows\Fonts\msyh.ttc` (Microsoft YaHei) →
    `C:\Windows\Fonts\simsun.ttc` →
    `C:\Windows\Fonts\simhei.ttf`
    依次回退；macOS `PingFang.ttc`；Linux WenQuanYi Micro Hei。
- 用户系统字体（Windows 上典型为 Microsoft YaHei / Segoe UI / Georgia；其他
  平台对应回退）—— **不属于 vanilla 白名单，由系统自带**。本文件列出仅为
  追溯依赖来源；它们不在 V5 §0.1「借用 vanilla 美术资源」范围内，因此 CI
  grep 不约束。

## 6. 不读取的 vanilla 路径（V5 §0.2 + §3 明确排除）

以下 vanilla 路径在 V5 路线图中**禁止读取**，CI grep 校验需保证零引用：

- `interface/*.gui` / `interface/*.gfx` / `gfx/fonts/*.fnt`
- `common/national_focus/`
- `common/decisions/` 与 `common/decisions/categories/`
- `events/*.txt`
- `common/scripted_triggers/` / `scripted_effects/` / `scripted_localisation/`
  / `scripted_guis/`
- `common/ai_strategy/` / `ai_strategy_plans/` / `ai_templates/`
- `dlc/` 任意路径
- `tutorial/` / `tweakergui/` / `portraits/`
- `localisation/` 加载尚未启用（阶段 C.7 才接入；届时再扩本节）


## 7. grep 校验脚本

每次 PR 走 CI 时，以下 grep 必须返回**空**：

```bash
# A. 已删除的 vanilla GUI 解析器层
grep -r 'GuiRuntime\|UiCommand\|gui_runtime\|gfx_index\|set_visible_windows\|GuiIndex\|GfxIndex\|BmFont\|GuiCommand\|BMFont' crates/

# B. 已删除的 V3 4.3 政治面板
grep -r 'UiPass\|PoliticsPass' crates/

# C. 已停用的 vanilla 玩法脚本目录引用
grep -rE 'common/(national_focus|decisions|scripted_triggers|scripted_effects|ai_strategy|ai_templates|scripted_localisation|scripted_guis)' crates/

# D. DLC 显式名引用（仅完整长字符串名；纯 2-3 字母缩写如 NSB/MtG/LaR
#    极易撞上局部变量 `rp` / `nb` 等，不在 grep 列表内 — 由 code review 兜底）
grep -rE '\b(TfV_DLC|BftB_DLC|NSB_DLC|MtG_DLC|LaR_DLC|WtT_DLC|DoD_DLC|BBA_DLC|AAT_DLC|ToA_DLC|TogetherForVictory|DeathOrDishonor|WakingTheTiger|ManTheGuns|LaResistance|BattleForTheBosporus|NoStepBack|ByBloodAlone|ArmsAgainstTyranny|TrialOfAllegiance|GotterdammerungDLC|GraveyardOfEmpires)\b' crates/
```

## 8. 维护规则

1. **新增 vanilla 引用前必须先更新本文件**。任何 PR 引入新路径但未列入
   白名单的，CI grep 会失败。
2. **白名单不收 mod 路径**。V5 §0.2 明确不做 mod / Steam Workshop /
   `replace_path`。
3. **路径变体（DLC 不同版本）走 `hoi4-paths::find` 的 fallback 链**，
   关键文件缺失时启动 banner 警告，地图 / 国旗 / 字体类关键缺失退出，
   装饰类（兵牌 sprite / 树木贴图）缺失仅警告并降级渲染。
4. **跨平台路径分隔符**：源码统一用 forward slash；`FsAssetDb` 内部规范化。
