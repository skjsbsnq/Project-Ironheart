# 全面中文汉化路线图

## 现状分析

### 已有基础设施

| 组件 | 状态 | 说明 |
|------|------|------|
| `hoi4-ui/src/i18n.rs` | ✅ 已实装 | 静态双语翻译表（~200+ key），`tr("key")` 查找，`Language::Chinese` 枚举 |
| `hoi4-ui/src/theme.rs` | ✅ 已实装 | egui 字体加载：Latin 衬线 + CJK fallback（msyh.ttc），中文可正常渲染 |
| `hoi4-app/src/glyphon_text.rs` | ⚠️ 部分 | fontdue atlas 用于地图文字，候选列表含 msyh.ttc 但优先加载 segoeui.ttf |
| `hoi4-ui/src/loc.rs` | ✅ 已实装 | `LocCatalog` 加载 vanilla `localisation/english/*.yml`，但**仅加载英文** |
| `hoi4-data/src/loader.rs` | ⚠️ 仅英文 | 解析 `parties_l_english.yml`、`*characters*l_english*.yml`，硬编码英文路径 |

### 未汉化的内容分类

| 类别 | 来源 | 数量估计 | 难度 |
|------|------|----------|------|
| **UI 硬编码字符串** | 各 panel `.rs` 文件中残留的英文 | ~20-30 处 | 低 |
| **游戏数据本地化**（国名/州名/科技名/装备名等） | vanilla `localisation/english/*.yml` | ~15,000+ key | 中 |
| **国策树名称** | `i18n.rs` 仅有 40 条德国国策 | 各国数百条 | 中 |
| **事件标题/描述** | `i18n.rs` 仅有 50 条德国事件 | 全部事件数千条 | 高 |
| **地图标签**（国名/省名） | `mapname_atlas.rs` + `province_name_atlas.rs` | ~800 州 + ~200 国 | 中 |
| **师名/舰名/联队名** | `hoi4-state` 中的 OOB 数据 | 动态生成 | 低 |
| **Situation 面板** | `situation_panel.rs` 中 "Supporters"/"Winner" 等 | ~5 处 | 低 |

---

## 路线图（分 5 个阶段）

### 阶段 1：消灭 UI 层残留硬编码（1-2 天）

**目标**：所有 UI 面板的固定文本 100% 走 `tr()` 通道。

**具体任务**：

1. `province_info.rs` — `"Slots: {}/{}"` → `tr("slots")`
2. `situation_panel.rs` — `"Supporters: "` → `tr("supporters")`，`"Winner: "` → `tr("winner")`
3. `military.rs` — `"eq "` / `"str "` 前缀 → `tr("equipment_short")` / `tr("strength_short")`
4. 全局搜索所有 `ui.label(format!(...` 和 `RichText::new(format!(...` 中的英文字面量
5. 在 `i18n.rs` 的 `TRANSLATIONS` 表中补充对应中文翻译

**验收**：切换到中文后，所有 egui 面板无英文残留。

---

### 阶段 2：游戏数据本地化管线改造（3-5 天）

**目标**：让 `LocCatalog` 支持按当前语言加载对应的 `.yml` 文件，实现国名/州名/科技名等的中文显示。

**具体任务**：

1. **改造 `LocCatalog::load_from_dir`**：
   - 接受 `Language` 参数
   - 中文时优先加载 `localisation/chinese/` 目录（如果存在）
   - 不存在时 fallback 到 `localisation/english/`
   - HOI4 原版没有中文 loc 文件，需要从社区汉化 mod 加载或自建

2. **改造 `hoi4-data/src/loader.rs`**：
   - `load_party_names()` 和 `load_character_names()` 支持语言参数
   - 中文时查找 `parties_l_chinese.yml` / `*l_chinese*.yml`

3. **State 名称本地化**：
   - `loader.rs` 中 state 的 `name` 字段是 loc key（如 `STATE_1`）
   - 需要在 `World` 初始化时用 `LocCatalog` 解析为实际显示名
   - 中文 loc 文件中 `STATE_1:0 "伊斯坦布尔"` 等

4. **创建中文 loc 数据源**：
   - 方案 A：支持加载社区汉化 mod（`mod/` 目录下的 `localisation/chinese/`）
   - 方案 B：内置一份精简中文 loc 文件（覆盖国名/州名/科技名）
   - 推荐方案 A + B 结合：内置核心翻译，支持 mod 覆盖

**验收**：选择中文后，国家选择界面的国名、省份信息卡的州名显示中文。

---

### 阶段 3：地图文字中文渲染（3-5 天）

**目标**：地图上的国名标签和省名标签正确显示中文。

**具体任务**：

1. **修复 `glyphon_text.rs` 字体优先级**：
   - 当语言为中文时，优先加载 `msyh.ttc` 而非 `segoeui.ttf`
   - 或实现字体 fallback 链：先用 segoeui 渲染 Latin，缺字时回退 msyh

2. **扩大 fontdue atlas 容量**：
   - 当前 1024×1024 对 CJK 不够用
   - 中文时扩大到 2048×2048 或实现多页 atlas
   - 或改为按需分配 + LRU 淘汰策略

3. **`mapname_atlas.rs` 中文支持**：
   - `bake_country_name_atlas()` 使用 loc 后的中文国名
   - 确保 fontdue 能正确光栅化中文字符
   - 调整字号/间距适配中文排版（中文字符等宽，不需要 kerning）

4. **`province_name_atlas.rs` 中文支持**：
   - 省名/州名标签同理

**验收**：地图缩放到各级别时，国名和省名标签显示中文。

---

### 阶段 4：国策/事件/科技全量翻译（1-2 周）

**目标**：游戏内所有动态内容（国策树、事件、科技）显示中文。

**具体任务**：

1. **国策树翻译扩展**：
   - 当前 `i18n.rs` 仅有 40 条德国国策
   - 需要覆盖所有主要国家（苏联/英国/美国/日本/意大利/法国/中国等）
   - 每国约 50-80 条，总计 ~500 条
   - 数据来源：HOI4 中文 Wiki / 社区汉化包

2. **事件翻译扩展**：
   - 当前仅 50 条德国事件
   - 全部事件约 2000-3000 条
   - 优先翻译主要国家的历史事件线

3. **科技名称翻译**：
   - 步兵/装甲/空军/海军/工业/电子各分支
   - 约 200-300 条科技名 + 描述

4. **翻译数据外置**：
   - 将翻译从 `i18n.rs` 静态数组迁移到外部 `.json` 或 `.yml` 文件
   - 运行时加载，方便社区贡献翻译
   - 路径：`assets/localization/zh/*.yml`

**验收**：国策树面板、事件弹窗、科技面板全部显示中文。

---

### 阶段 5：完善与社区支持（1 周）

**目标**：建立可持续的翻译维护机制。

**具体任务**：

1. **翻译覆盖率检测工具**：
   - 编写脚本扫描所有 `tr()` 调用的 key
   - 对比翻译表，输出未翻译 key 列表
   - CI 中集成覆盖率报告

2. **支持社区汉化 mod 热加载**：
   - 设置面板增加"汉化 mod 路径"选项
   - 支持从 Steam Workshop 的汉化 mod 目录加载 `l_chinese` 文件
   - 优先级：用户 mod > 内置翻译 > 英文 fallback

3. **翻译贡献指南**：
   - 编写 `CONTRIBUTING_TRANSLATION.md`
   - 定义翻译文件格式和提交流程
   - 提供翻译模板文件

4. **字体配置化**：
   - 设置面板增加"自定义字体路径"
   - 支持用户指定 CJK 字体（适配非 Windows 平台）

5. **文本排版优化**：
   - 中文不需要词间空格，调整 UI 布局间距
   - 长中文文本自动换行（按字符而非按词）
   - tooltip 宽度适配中文

---

## 优先级排序

```
阶段 1（UI 硬编码）  ← 最快见效，1-2 天
    ↓
阶段 2（数据管线）   ← 核心架构改造，解锁后续所有内容
    ↓
阶段 3（地图文字）   ← 视觉冲击最大
    ↓
阶段 4（全量翻译）   ← 工作量最大，可渐进完成
    ↓
阶段 5（社区支持）   ← 长期维护
```

## 技术风险

| 风险 | 影响 | 缓解措施 |
|------|------|----------|
| fontdue atlas 空间不足 | 中文字符显示为空白 | 多页 atlas + LRU 淘汰 |
| 社区汉化 mod 格式不兼容 | 加载失败 | 兼容 Paradox 标准 yml 格式 |
| 中文文本过长溢出 UI | 布局错乱 | 自动缩放 + 省略号截断 |
| 性能：大量 CJK 字形光栅化 | 启动变慢 | 预热常用字 + 异步光栅化 |

## 快速开始建议

如果想立刻看到效果，从**阶段 1** 开始：全局搜索 `format!("Slots`、`"Supporters`、`"Winner` 等硬编码字符串，替换为 `tr()` 调用并在 `i18n.rs` 中补充中文翻译。这是最小改动、最快见效的路径。
