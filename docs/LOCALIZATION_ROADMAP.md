# 游戏内容汉化路线图

## 现状分析

| 内容类型 | 来源 | 条目数 | 当前语言 |
|---|---|---|---|
| 国策树（GER 40 节点） | `content/GER_focus_tree.ron` 的 `name` 字段 | 40 | 英文 |
| 事件（GER 43 个） | `content/GER_events.ron` 的 `name` / option `name` | ~130 | 英文 |
| 决议（GER 14 个） | `content/GER_decisions.ron` 的 `name` / `description` | 28 | 英文 |
| 科技名称 | vanilla `common/technologies/*.txt` → `Technology.key` | ~200 | 英文 key |
| 装备名称 | vanilla `common/units/equipment/*.txt` → `Equipment.key` | ~100 | 英文 key |
| 国家精神（ideas） | vanilla `common/ideas/*.txt` → `IdeaDef.key` | ~50 活跃 | 英文 key |
| 国家名 / 标签 | `CountryStore.tags` | 70+ | 3 字母 tag |
| 省份 / 地区名 | vanilla `localisation/english/*.yml` → `LocCatalog` | ~13000 | 英文 |
| 师模板名 | vanilla `history/units/*.txt` | ~20 GER | 英文 |

## 汉化方案

不改 RON/数据文件（保持 key 为英文 ID），在显示层通过 `i18n::tr()` 翻译表查找中文。

---

## 阶段 L.1 — 自研内容汉化（国策 / 事件 / 决议）

**工作量**：~200 条翻译，1-2 天

| 步骤 | 内容 |
|---|---|
| L.1.1 | `i18n.rs` 新增 `focus_*` 系列 key（40 条国策名） |
| L.1.2 | 新增 `event_*` 系列 key（43 事件标题 + ~90 选项名） |
| L.1.3 | 新增 `decision_*` 系列 key（14 决议名 + 14 决议描述） |
| L.1.4 | `focus_tree_panel.rs` 节点名走 `tr(focus.id)` 回退 `focus.name` |
| L.1.5 | `event_panel.rs` 事件标题/选项走 `tr(event.id)` |
| L.1.6 | `politics.rs` 决议行走 `tr(decision.id)` |

**翻译示例**：
```
("GER_rhineland", ["Remilitarize the Rhineland", "莱茵兰再军事化"]),
("GER_four_year_plan", ["Four Year Plan", "四年计划"]),
("GER_anschluss", ["Anschluss", "德奥合并"]),
("GER_demand_sudetenland", ["Demand Sudetenland", "索取苏台德"]),
("GER_danzig_or_war", ["Danzig or War", "但泽或战争"]),
("germany.rhineland", ["Rhineland Remilitarized!", "莱茵兰再军事化！"]),
("ger.volkswagen_werk", ["Build the Volkswagen Werk", "建造大众汽车工厂"]),
```

---

## 阶段 L.2 — 科技树汉化

**工作量**：~200 条翻译，1 天

| 步骤 | 内容 |
|---|---|
| L.2.1 | `TRANSLATIONS` 表新增科技 key → 中文名 |
| L.2.2 | `research.rs` 显示科技名走 `tr(tech.key)` 回退 key |
| L.2.3 | 科技类别名（infantry/armor/air/naval/industry/electronics） |

**翻译示例**：
```
("infantry_weapons1", ["Infantry Weapons I", "步兵武器 I"]),
("support_weapons", ["Support Weapons", "支援武器"]),
("motorized_infantry", ["Motorized Infantry", "摩托化步兵"]),
("medium_tank1", ["Medium Tank I (Pz.III)", "中型坦克 I（三号坦克）"]),
("fighter1", ["Fighter I", "战斗机 I"]),
("strategic_bomber1", ["Strategic Bomber I", "战略轰炸机 I"]),
("basic_machine_tools", ["Basic Machine Tools", "基础机床"]),
("electronic_mechanical_engineering", ["Electronic Mechanical Engineering", "电子机械工程"]),
```

---

## 阶段 L.3 — 国家精神 / Ideas 汉化

**工作量**：~50 条翻译，0.5 天

| 步骤 | 内容 |
|---|---|
| L.3.1 | 翻译 GER 起始 + 国策产出的 idea key |
| L.3.2 | `politics.rs` 国家精神列表走 `tr(idea_key)` |

**翻译示例**：
```
("four_year_plan", ["Four Year Plan", "四年计划"]),
("autarky_idea", ["Autarky", "自给自足"]),
("war_economy", ["War Economy", "战时经济"]),
("total_mobilization", ["Total Mobilization", "全面动员"]),
("junker_officer_corps", ["Junker Officer Corps", "容克军官团"]),
```

---

## 阶段 L.4 — 国家名 / 地区名 / 装备名

**工作量**：~300 条翻译，1-2 天

| 步骤 | 内容 |
|---|---|
| L.4.1 | 国家 tag → 中文名（70 国） |
| L.4.2 | 装备 key → 中文名 |
| L.4.3 | 主要地区名（可选） |
| L.4.4 | 外交/阵营/战争列表显示走 `tr(tag)` |

**翻译示例**：
```
("GER", ["Germany", "德国"]),
("ENG", ["United Kingdom", "英国"]),
("FRA", ["France", "法国"]),
("SOV", ["Soviet Union", "苏联"]),
("POL", ["Poland", "波兰"]),
("ITA", ["Italy", "意大利"]),
("JAP", ["Japan", "日本"]),
("USA", ["United States", "美国"]),
("infantry_equipment_1", ["Infantry Equipment I", "步兵装备 I"]),
```

---

## 阶段 L.5 — 师模板 / 战斗 / 杂项

**工作量**：~50 条，0.5 天

| 步骤 | 内容 |
|---|---|
| L.5.1 | 师模板名（"Infanterie-Division" → "步兵师"） |
| L.5.2 | 战斗状态（In Combat→战斗中, Moving→移动中, Training→训练中） |
| L.5.3 | 月份名（January→一月…December→十二月） |
| L.5.4 | 建筑类型（infrastructure→基础设施, industrial_complex→民用工厂…） |

**翻译示例**：
```
("Infanterie-Division", ["Infantry Division", "步兵师"]),
("Panzer-Division", ["Panzer Division", "装甲师"]),
("infrastructure", ["Infrastructure", "基础设施"]),
("industrial_complex", ["Civilian Factory", "民用工厂"]),
("arms_factory", ["Military Factory", "军用工厂"]),
("dockyard", ["Dockyard", "船坞"]),
```

---

## 实现架构

```rust
// 面板代码统一模式：
ui.label(tr(&focus.id));  // 找到翻译用翻译，找不到回退到原始 name/key

// i18n.rs 查找逻辑：
pub fn tr(key: &str) -> &str {
    lookup(key, current_language()).unwrap_or(key)
}
```

---

## 工作量汇总

| 阶段 | 翻译条目 | 代码改动 | 估时 |
|---|---|---|---|
| L.1 国策/事件/决议 | ~200 | 3 个面板文件 | 1-2 天 |
| L.2 科技树 | ~200 | 1 个面板文件 | 1 天 |
| L.3 国家精神 | ~50 | 1 个面板文件 | 0.5 天 |
| L.4 国家/装备/地区 | ~300 | 4 个面板文件 | 1-2 天 |
| L.5 师/战斗/杂项 | ~50 | 散布 | 0.5 天 |
| **总计** | **~800 条** | | **4-6 天** |

---

## 优先级建议

1. **L.1 最优先** — 国策树是玩家每局必看的核心内容
2. **L.4.1 次优先** — 国家名在外交面板/阵营/战争中高频出现
3. **L.2 第三** — 科研面板每局必用
4. **L.3 / L.5 最后** — 国家精神和杂项影响较小

---

*文档版本*：2026-05-19
*配套代码*：`crates/hoi4-ui/src/i18n.rs`
