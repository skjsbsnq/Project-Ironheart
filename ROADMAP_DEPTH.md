# Project Ironheart — 世界深度与系统联动 改进路线图（DEPTH）

> **Historical note (Phase 11, 2026-05-30):** This document is archived for research context. Current map renderer pass order, ownership, debug, baseline, and cleanup policy live in [`MAP_RENDERER_V2_REFACTOR_ROADMAP.md`](./MAP_RENDERER_V2_REFACTOR_ROADMAP.md), [`docs/map_renderer_v2.md`](./docs/map_renderer_v2.md), and [`docs/map_renderer_v2_contributing.md`](./docs/map_renderer_v2_contributing.md).

> **这份文档是研究输出，不是 V5 之外的第二条主路线。**
>
> 起草背景：用户 2026-05-20 反馈"系统割裂、生产白板、省份点击没反馈、政治面板缺执政党与元首肖像、每天自动建工厂太离谱"。
>
> 项目治理规则（`ROADMAP_V5.md` §1 / §8.3）禁止新增长期共存的子路线图——V3 时代被 4 份累计 370 KB 的补丁路线图反复折腾过。
> 因此本文件只作为 **V5 阶段 J（Depth & Coherence）候选清单** 提交：所有条目最终需 merge 回 `ROADMAP_V5.md` 对应阶段（多数挂在新增的"阶段 J"，少量回填阶段 C/F），不在主路线之外长期保留本文件。
>
> 与已有 `.kiro/specs/cross-system-feedback-loops/requirements.md`（feedback bus 设计）的关系：本路线图是更广的"内容深度 + 玩家反馈表层"补丁，**包含且超出**那份 spec；feedback bus 落到本文 J.7。
>
> **状态**：草案 v0.5（2026-05-20 J.10 完整实装：AI师移动执行层 + News事件不暂停）；~~待用户确认后开始 J.1~~ **J.1 已完成（2026-05-20）**。

---

## 0. 诊断：当前世界为什么"割裂"

V5 阶段 A→G 已交付：5 大面板 / 焦点树 / 决议 / 事件 / 7 国 AI / 平衡曲线 / 存档 / 设置 / 结束界面。
但这些子系统在玩家眼里仍是**互相独立的进度条**。下面是按"玩家点击 → 期望反馈"列出的 8 处可见空白：

| # | 玩家行为 | 期望反馈（vanilla 同款） | 当前实际 |
|---|---|---|---|
| 0.1 | 看政治面板 | 国旗 + 执政党全称（"Nationalsozialistische Deutsche Arbeiterpartei"）+ 国家元首肖像 + 元首名字 | 仅 ideology 色块 + "fascism" key；无名字、无肖像 |
| 0.2 | 左键点省份 | 省份 InfoCard：owner / controller / state_category / 建筑槽位 / 工厂数 / 战略资源 / 驻军 / VP | 仅高亮 + 红色脉动；无任何文字反馈 |
| 0.3 | 完成国策 / 决议 / 事件选项 | 立刻看到工厂 +1、稳定度跳动、idea 添加并出现在面板 | effect 大多写 `GlobalFlag`，对 `World.countries.*` 无可见副作用；只有 PP/Stab/WS 三件直接生效 |
| 0.4 | 在某州建造工厂 | 看到该州"建筑槽位 4/6 → 5/6"，槽位满后无法再建 | `state_category` 已读但**槽位 cap 没生效**；可以无限建到 u8 上限 |
| 0.5 | 拿下敌方核心州 | 立刻见到敌方民工掉、自己拿到 20% 占领产能、敌国稳定度跌 | 仅 `controllers[]` 改字段；其它系统下次 tick 才"碰巧"读到 |
| 0.6 | 生产 50 辆 PzKpfw III | 库存 +50 → 师装备需求被满足 → 师 strength 涨 → 战斗中装备减少 → 库存被吃光后 strength 回不去 | 生产线 → stockpile 闭环只完成一半；师 strength 不读 stockpile，战斗损耗也不写库存 |
| 0.7 | 研究"工业 III" | 工厂日产能立刻 +X% | 科技 modifier 表已有，但 `EconomyState.factory_output_factor` **未在 research tick 完成时重算** |
| 0.8 | 静观一段时间 | AI 国家以可信节奏发育 | 70+ 国 AI 在月初集中触发 production eval，外观像"每天都在建"；玩家自己 GER 已 skip，所以 0.8 主要是别国节奏失真 |
| 0.9 | 和平时期移动军队到外国 | 被阻止——除非对方是同阵营成员或已授予军事通行权 | `movement.rs` **无任何通行权检查**；师可以自由穿越任何外国领土，和平时期也能走到巴黎/莫斯科 |

**根因结论**：项目缺一条**显式的"状态变化 → 派生系统重算 → 玩家可见反馈"管线**。子系统都在但电线没接。此外，军事移动缺乏基本的通行权约束，导致和平时期师可以自由穿越外国领土——这在 HOI4 原版中是绝对不允许的。

---

## 1. 设计原则

1. **能玩 > 数值平衡**。本路线图以"玩家在 5 分钟内能看到反馈"为验收口径，不追求 vanilla 数值精确还原。
2. **借用 vanilla 美术，不解析 vanilla 玩法资产**——继续遵守 V5 §0.2。
3. **每个子阶段必须有玩家可观测的副作用**。"加了一个 trait" 不算交付，"点这个按钮 → 那个数字改变 → tooltip 解释为什么改" 才算。
4. **不破坏 V5 §1 治理**：本路线图条目最终 merge 回 V5；不引入第二份长期路线。
5. **stockpile 与 ProvinceInfoCard 是杠杆点**：玩家最常看的两个表层；优先实装。

---

## 2. 阶段 J 总览（候选挂入 ROADMAP_V5.md）

> v0.2 调整：用户 2026-05-20 要求"执政党名字 + 元首肖像"放到最前。原 J.5 → 新 J.1，其余顺延。

| 子阶段 | 目标 | 估时 | merge 目标 |
|---|---|---|---|
| **J.1** Country Leader & Portrait | 解决 0.1 | 1 周 | V5 阶段 C.2 增量 |
| **J.2** 省份 InfoCard + 左键交互 | 解决 0.2 | 1 周 | V5 阶段 C（5 大面板）扩 C.8 |
| **J.3** State Category & 建筑槽位 | 解决 0.4 | 1 周 | V5 阶段 J（新建） |
| **J.4** Stockpile 闭环（生产→库存→师→战斗→库存） | 解决 0.6 | 2-3 周 | V5 阶段 J | ✅ 已完成 |
| **J.4b** 库存面板 + 资源仓储面板 | 玩家可见装备库存与资源收支 | 1-1.5 周 | V5 阶段 C 扩 C.9 | ✅ 已完成 |
| **J.5** Effect Registry 杠杆扩展 | 解决 0.3 | 1.5 周 | V5 阶段 D.2 / F.1 / F.2 增量 | ✅ 已完成 |
| **J.5b** 事件系统重构 + 局势系统 + 西班牙内战完整实装 | 事件分类 + 局势面板 + 多国介入 | 5-6.5 周 | V5 阶段 F.1 增量 | ✅ 已完成 |
| **J.5c** 生产面板补全：新增生产线 + 建造面板 | 玩家可新增/管理生产线和建造队列 | 1.5 周 | V5 阶段 C.3 增量 | ✅ 已完成 |
| **J.6** AI 建造节奏与约束 + 民工建造机制修复 | 解决 0.8 + 建造规则 | 1.5 周 | V5 阶段 F.3 增量 + 经济 bug fix | ✅ 已完成 |
| **J.7** Feedback Bus（领域事件总线） | 解决 0.5 / 0.7 | 2-3 周 | V5 阶段 J（吸收 cross-system-feedback-loops spec） | ✅ 已完成 |
| **J.8** 通知 Feed + 关键事件自动暂停 | feedback bus 表层 | 0.5-1 周 | V5 阶段 J |
| **J.9** 军事通行权（Military Access） | 解决 0.9 | 1 周 | V5 阶段 J |
| **J.10** AI 师移动执行层（Plan → Movement） | AI 评估完"进攻/防御"后真正下达移动命令 | 2 周 | V5 阶段 J | ✅ 已完成 |
| **J.11** 国策面板改版（HOI4 原版风格） | 近全屏面板 + 方形卡片 + 拖拽平移 + 底部详情栏 | 1-2 周 | V5 阶段 D.4 增量 |

**累计 19.5-24.5 周**（≈ 5-6 月单人全职）。可与 V5 阶段 I（Counter Reboot）按顺序串接，也可在 MVP2 之前插入。

强烈建议的执行顺序：**~~J.5c~~(已完成) → J.1 → J.9 → J.10 → J.2 → J.5 → J.5b → J.3 → J.4 → J.4b → J.7 → J.8 → J.6**
理由：
- J.1 用户明确指定最高优先（元首肖像 + 党全称）
- J.9 紧随其后——军事通行权是基本游戏规则，不修复则后续所有军事测试都不可信
- **J.10 紧跟 J.9**——没有 AI 师移动执行层，战争就是双方站桩，整个军事系统形同虚设；J.9 的通行权检查也需要 J.10 来验证
- J.2 省份 InfoCard 视觉收益高、风险低
- J.5 需先于 J.7（feedback bus 需要 effect 已经能写得动 World）
- J.5b 紧跟 J.5——事件系统重构依赖 effect 扩展，且西班牙内战是 1936 最重要的世界事件
- J.3 / J.4 是经济链核心
- J.4b 紧跟 J.4——stockpile 闭环打通后立刻给玩家可见的库存 + 资源面板
- J.7 是粘合剂
- J.6 在 AI 行为之上微调 + 修复消费品工厂 bug，留到最后（但如果用户急于修复"建造太快"，可以把 J.6.1-J.6.4 提前到 J.5c 之后单独做）

---

## 3. 子阶段详细规格

### J.1 — Country Leader & Portrait （**优先级最高**）

**问题**：政治面板缺执政党全称 + 国家元首名字 + 元首肖像；外交面板悬停一个国家也只看到 tag。**用户 2026-05-20 明确要求此项最先完成。**

**vanilla 路径**：
- `common/characters/GER.txt` 等：人物定义（含 portrait 路径、name、character_role: country_leader / political_advisor / theorist）
- `gfx/leaders/GER/Portrait_Germany_Adolf_Hitler.dds`：肖像 (~256×256 BC1)
- `localisation/english/parties_l_english.yml`：党派全称（key 形如 `GER_fascism_party`）

**当前代码现状**：
- `World.countries.ruling_party: Vec<String>` 已存在，仅持有 ideology key（"fascism" 等）
- 党派全称从未读过 `parties_l_english.yml`
- `common/characters/*.txt` 完全未解析；`gfx/leaders/**` 完全未加载
- `hoi4-ui::politics::PoliticsPanel::show` 顶部只有 ideology 色块 + `ideology_label(key)` 拿到的小写英文名

**交付**：
| # | 任务 | 估时 |
|---|---|---|
| ✅ J.1.1 | `hoi4-data::characters` 模块新建：解析 `common/characters/*.txt` 子集——只取 `country_leader` 角色 + `name` + `portraits.civilian.large` + `ideology`；其它角色（顾问 / 战地 / 元帅）骨架占位返回 `Vec<CharacterDef>` | 1.5 天 |
| ✅ J.1.2 | `World.countries` 加 `country_leader_id: Vec<Option<CharacterId>>` 字段；`World::new` 初始化时按 `ruling_party` 字符串选对应 ideology 的 country_leader（vanilla 同 TAG 同 ideology 一般只有 1 人，多人时取第一个） | 0.5 天 |
| ✅ J.1.3 | `loader.rs` 解析 `localisation/english/parties_l_english.yml` 进 `GameData.party_names: HashMap<(CountryTag, IdeologyKey), String>`；缺失则 fallback 到 ideology_label 旧路径 | 1 天 |
| ✅ J.1.4 | `hoi4-ui::icons::IconBank` 加 `gfx/leaders/<TAG>/` 作为 search dir；DDS BC1 / BC3 解码已在 V5 阶段 B.5 实现，直接复用 | 0.5 天 |
| ✅ J.1.5 | `politics::PoliticsData` 加 4 字段：`leader_name: String` / `leader_portrait_key: Option<String>` / `party_full_name: String` / `country_tag: String`；`hoi4-app::main` 在 `politics_data` 快照构造点填充 | 0.5 天 |
| ✅ J.1.6 | `politics::PoliticsPanel::show` 顶部布局重构：左 128×128 元首肖像（IconBank 渲染，缺失时用国旗大图占位）+ 右纵列 [元首名 / 党全称 / 现 ideology 色块]；原 ruling_party 行删除 | 1 天 |
| ✅ J.1.7 | 外交面板（`diplomacy.rs`）每国行：tag → 国旗 + 元首小头像（48×48）+ 元首名；`DiplomacyData::CountryEntry` 加 `leader_name` / `leader_portrait_key` 字段 | 1 天 |
| ✅ J.1.8 | `swap_ruling_party` 类 effect 触发时：在 effect 执行末尾调用 `World::refresh_country_leader(country)` 重选 leader；测试：完成 GER `Oppose Hitler` 民主支线后顶部肖像切换 | 0.5 天 |
| ✅ J.1.9 | `docs/vanilla_assets_used.md` §3.7 加 `gfx/leaders/**` + `common/characters/**` + `localisation/**/parties_l_*.yml`；启动 banner 扫描脚本验证 70+ 国都至少有一个 country_leader（缺失则降级到"国旗大图 + ideology 名"，warning 不致命） | 0.5 天 |
| ✅ J.1.10 | 集成测试 `tests/leader_portrait.rs`：①世界初始化后 GER 的 leader 名 = "Adolf Hitler" ②SOV.leader = "Joseph Stalin" ③GBR.leader = "Stanley Baldwin"（1936 起手）④fallback 路径在缺失肖像时 leader_portrait_key = None 不 panic | 1 天 |

**验收**：
- 启动游戏选 GER → 政治面板顶部出现希特勒肖像 + "Adolf Hitler" + "Nationalsozialistische Deutsche Arbeiterpartei"
- 切到外交面板看苏联行：显示斯大林头像 + "Joseph Stalin"
- 完成"复辟霍亨索伦"focus → 政治面板肖像变 Wilhelm II（前提：J.5 把 swap_ruling_party effect 接通；本阶段先确保字段链路工作，effect 联动可放 J.5）
- `cargo test --workspace` 全绿

**merge 回 V5**：补丁挂入 V5 阶段 C.2 的"已完成"行——原 C.2 描述里"5 顾问槽位（空）" 后面追加"+ 元首肖像 + 党全称（J.1 补丁）"。

---

### J.2 — 省份 InfoCard 左键弹窗

**问题**：左键省份只产生高亮，玩家不知道点了什么。

**交付**：
| # | 任务 | 估时 |
|---|---|---|
| ✅ J.2.1 | `crates/hoi4-ui/src/province_info.rs` 新增 `ProvinceInfoCard`，固定贴在屏幕左下角，可拖动可关闭 | 0.5 天 |
| ✅ J.2.2 | 数据快照 `ProvinceInfoData` 由 `hoi4-app::main` 在 `try_pick_province()` 命中后填充：province_id / state_id / state_name / owner_tag + flag / controller_tag + flag / state_category 名 / 当前建筑槽位 used 与 max（J.3 提供，本阶段先显示 used 不显示 max）/ civ-mil-dock 数 / state.resources 列表 / supply 数值 / VP / 驻该省份的 division 列表（玩家国家可见） | 1.5 天 |
| ✅ J.2.3 | 9-slice 木纹背景 + 暖金分隔条 + 资源图标走 `gfx/interface/resources/*.dds`（白名单 §3.6 加） | 1 天 |
| ✅ J.2.4 | 右键省份保留现有 `ProvinceMenu`（justify wargoal / declare war / move），不冲突；左键 = InfoCard，右键 = 菜单 | 0.5 天 |
| J.2.5 | 卡片可滚动；点击 owner/controller flag 跳外交面板对应国家（接 J.1 已交付的 leader 头像）；点击 division 跳军队面板高亮该师 | 1 天 |

**验收**：在跑动游戏中左键巴黎（玩家=GER, 1936-01-01）→ 卡片显示 "Île-de-France / 大都会 / civ 8 mil 0 dock 0 / steel 8 / VP 50 / 法国 1. Division Motorisée"，关闭按 ESC，再左键莫斯科→显示苏联数据 + 斯大林头像（点 owner flag 跳过去看到）。

**merge 回 V5**：作为 V5 阶段 C 的 C.8 节，追加到 ROADMAP_V5.md §5.阶段 C 的表中。

---

### J.3 — State Category & 建筑槽位 cap

**问题**：vanilla `common/state_category/00_state_category.txt` 定义 12 个等级（wasteland 1 → megalopolis 12 槽位），当前 loader 只读字符串名没解析槽位；建筑可以无限建。

**交付**：
| # | 任务 | 估时 |
|---|---|---|
| ✅ J.3.1 | `hoi4-data` 新增 `StateCategoryDef { name, local_building_slots: u8 }`；loader 解析 `common/state_category/*.txt` | 1 天 |
| ✅ J.3.2 | `GameData.state_categories: HashMap<String, StateCategoryDef>` + `StateData` 加 `category_slots: u8`（运行时缓存解析结果）；vanilla 缺类则 fallback `local_building_slots = 4` | 0.5 天 |
| ✅ J.3.3 | `economy::construction::tick` 入队前校验：`current_shared_buildings + queue_for_state < category_slots`，超出即 reject 并触发 tracing warn | 1 天 |
| ✅ J.3.4 | UI 端：生产/建造面板的 BuildOrder 弹窗按 state 当前 used/max 灰显；`ProvinceInfoCard`（J.2）显示 "槽位 5/6" | 1 天 |
| J.3.5 | 单元测试：metropolis 12 槽位、wasteland 1 槽位、共享建筑 ≠ 独立建筑分别校验；集成测试：1936 GER 各 state 槽位 cap 加和 ≈ vanilla 数据 | 1 天 |
| J.3.6 | `naval_base / air_base / anti_air / radar` 完工分支：`construction::tick` 完成时写入 `world.states.naval_bases[si] += 1` 等（要先扩 StateStore 字段） | 2 天 |
| J.3.7 | 阶段 I（HOI3 兵牌）依赖：建筑槽位 cap 满了之后玩家会去新州建造，那些州可能在波兰/法国，需要在 J.7 占领事件里把"占领州的槽位继承给新 controller"做对——本子任务只埋接口，行为留 J.7 | 0.5 天 |

**验收**：玩家在柏林（metropolis, 12 槽位）连建工厂到第 13 个时面板灰显并提示"槽位已满"；在小工业农村州建到第 4 个工厂时同样灰显。1936-01-01 GER 总民工 + 总军工 + 总海工 ≈ vanilla 历史值（±5%）。

**merge 回 V5**：作为新阶段 J 的子节。

---

### J.4 — Stockpile 闭环

**问题**：装备生产→库存→师装备→战斗损耗→补员的链条缺多个环节。

**当前现状审计**：
- ✅ `EconomyState.stockpile: Vec<HashMap<String, f32>>` 字段存在
- ✅ `economy::production::tick` 把日产能写入 stockpile（已复核 + 加资源系数）
- ✅ 师 `strength` 计算读 stockpile；战斗损耗扣装备
- ✅ 师 `template` 的 equipment_need（每师需要 N 单位 X 装备）已实现
- ✅ "训练新师消耗 stockpile"的扣减已实现

**交付**：
| # | 任务 | 估时 |
|---|---|---|
| ✅ J.4.1 | 审计 `production::tick` 完整路径：日产能 = `factories × output_per_factory × efficiency × 资源系数 × 科技 modifier`；补齐写 stockpile；写测试 | 2 天 |
| ✅ J.4.2 | `DivisionTemplate` 加 `equipment_needs: HashMap<String, u32>`（每个 battalion 需要 100 单位 infantry_equipment 等，按 vanilla `units/*.txt` 简化） | 1 天 |
| ✅ J.4.3 | 师 `strength_target` = `min(equipment_satisfied_ratio, manpower_satisfied_ratio)`；缺装备时 strength_target 下降；每日 strength 朝 strength_target 收敛（恢复速率 = 1%/天 + 训练时间） | 2 天 |
| ✅ J.4.4 | 战斗 tick：每场战斗按 attacker/defender 装备数量比例从 stockpile 扣 X% 损耗；strength 同步下降 | 2 天 |
| ✅ J.4.5 | "训练新师"动作（C.6 已有 Train 按钮）必须扣 stockpile + manpower；不足则按 ratio 训不满 | 1 天 |
| ✅ J.4.6 | 资源短缺 → 生产线 efficiency 降级：`steel/rubber/oil` 缺口 N% → output × (1 - 0.5×N%)，写入 `econ.resources[ci].consumed` | 2 天 |
| ✅ J.4.7 | UI 端：生产面板每生产线显示"日产 X / 资源缺口 Y / 实际产出 Z"；军队面板每师显示"装备 87% / 缺 13 PzKpfw III" | 1.5 天 |
| ✅ J.4.8 | 集成测试：1936-01-01 GER 跑 365 天，断言 ①infantry_equipment stockpile 单调上升 ②任意一场战斗后双方 stockpile 下降 ③波兰战役后 GER 损耗符合"几万件装备"量级 | 1 天 |

**验收**：跑通"GER 1939-09-01 宣战 POL → 9-10 月战役 → infantry_equipment 库存先涨后跌 → 师装备率从 100% → 87% → 战役结束后训练时间内回到 100%"。生产面板能看到资源短缺时 efficiency 自动降级。

**merge 回 V5**：作为新阶段 J 的子节；可向后影响 V5 阶段 C.3 / C.6 已交付内容（产生增量）。

---

### J.4b — 库存面板 + 资源仓储面板

**问题**：J.4 把 stockpile 闭环打通了，但玩家**看不到**自己有多少装备、每天产多少、消耗多少。同样，资源系统虽然在后台运行（`resources::tick` 每日计算），但玩家没有任何面板能看到"我有多少钢 / 石油 / 铝 / 橡胶 / 钨 / 铬，每天产多少、消耗多少、缺口多少"。

**vanilla 对标**：
- **装备库存**（Logistics 面板）：按装备类型列表，每行显示 [图标 / 名称 / 库存数 / 日产 / 日耗 / 净变化 / 缺口]
- **资源面板**（TopBar 资源图标点击展开）：6 种战略资源各一行 [图标 / 名称 / 产出 / 进口 / 出口 / 可用 / 缺口]，缺口用红色标注

**交付**：
| # | 任务 | 估时 |
|---|---|---|
| ✅ J.4b.1 | `crates/hoi4-ui/src/logistics_panel.rs` 新增 `LogisticsPanel`（装备库存面板）：egui `SidePanel`，快捷键 L 开关；按装备类别分组（Infantry / Armor / Artillery / Motorized / Air / Naval） | 1 天 |
| ✅ J.4b.2 | `LogisticsData` 快照：每种装备 → `{ name, icon_key, stockpile, daily_production, daily_consumption, net_change, deficit }`；`daily_consumption` = 师装备需求 - 当前持有（J.4.3 已计算）；`deficit` = 需求总量 - 库存 - 日产×30（30 天预估缺口） | 1 天 |
| ✅ J.4b.3 | 面板渲染：每行 [装备图标 48×48 / 名称 / 库存数（绿色）/ 日产（+N 蓝色）/ 日耗（-N 橙色）/ 净变化（绿/红）/ 缺口（红色粗体，0 时不显示）]；底部汇总"总装备种类 / 总缺口种类" | 1 天 |
| ✅ J.4b.4 | `crates/hoi4-ui/src/resource_panel.rs` 新增 `ResourcePanel`（资源仓储面板）：egui `Window`，从 TopBar 资源图标点击展开（或快捷键 R） | 0.5 天 |
| ✅ J.4b.5 | `ResourcePanelData` 快照：6 种战略资源各一行 → `{ kind, icon_key, produced, imported, exported, consumed, available, deficit }`；数据源 = `econ.resources[player]`（J.4 已有 `ResourceBalance`） | 0.5 天 |
| ✅ J.4b.6 | 面板渲染：每行 [资源图标 24×24 / 名称 / 产出（绿）/ 进口（蓝）/ 出口（黄）/ 消耗（橙）/ 可用（白/红）]；缺口 > 0 时整行背景变暗红 + tooltip 解释"缺 N 单位 → 生产效率 -X%" | 1 天 |
| ✅ J.4b.7 | TopBar 资源区域改造：现有 TopBar 只显示 fuel；扩展为 6 个资源小图标 + 数字（产出/可用），点击展开 ResourcePanel | 1 天 |
| J.4b.8 | 资源图标白名单：`gfx/interface/resources/resource_steel.dds` 等 6 个 DDS 加入 `docs/vanilla_assets_used.md` §3.8 | 0.5 天 |
| J.4b.9 | 集成测试：①GER 1936 LogisticsPanel 显示 infantry_equipment 库存 > 0 + 日产 > 0 ②ResourcePanel 显示 steel 产出 > 0 ③宣战 POL 后 daily_consumption 上升 + 库存下降可见 | 1 天 |

**验收**：
- 按 L 打开库存面板 → 看到 infantry_equipment 库存 12000 / 日产 +45 / 日耗 -12 / 净 +33 / 缺口 0
- 宣战 POL 后再看 → 日耗跳到 -80 / 净变负 / 缺口出现红色数字
- TopBar 显示 steel 24 / oil 3 / aluminum 8 等；点击展开 ResourcePanel → 看到完整收支明细
- 资源缺口时（如橡胶 = 0）整行暗红 + tooltip "Rubber deficit: -3 → production efficiency -15%"

**merge 回 V5**：作为 V5 阶段 C 的 C.9 节（继 C.8 ProvinceInfoCard 之后）。

---

### J.5 — Effect Registry 杠杆扩展

**问题**：D.2 已交付 51 个 effect，但许多写 `GlobalFlag` 后再无下文。玩家完成国策 / 触发决议 / 事件选项后看不到 World 变化。

**交付**：
| # | 任务 | 估时 |
|---|---|---|
| ✅ J.5.1 | 审计现有 51 effect：每条标注 ①直接写 World 字段 ②走 modifier 表 ③仅写 GlobalFlag。产出表格 `docs/effect_audit.md` | 1 天 |
| ✅ J.5.2 | 补 12 条经济杠杆 effect：`AddCivilianFactories`/`AddMilitaryFactories`/`AddDockyards`（state-scoped）/`AddResourcesToState`/`AddBuildingSlot`/`AddManpowerToState`/`AddEquipmentStockpile`/`AddDivisionsFromTemplate`/`UnlockBuildingType`/`SetStateCategory`/`BoostFactoryOutput`/`AddIdea` 真正写入 `World.countries.ideas` | 2 天 |
| ✅ J.5.3 | 补 6 条政治杠杆：`PromoteAdvisor`（顾问槽真插一个）/`SwapRulingParty`（联动 J.1 的 `refresh_country_leader`）/`AddPartyPopularity`/`SetCountryFlagWithDays`/`UnlockFocusBranch`/`AddPoliticalAdvisorPool` | 1.5 天 |
| J.5.4 | 把 `GER_focuses.ron` / `GER_events.ron` / `GER_decisions.ron` 中所有 `SetCountryFlag` 副作用扫一遍：能改成具体杠杆 effect 的全部改掉；剩下确实只是 flag 的保留 | 2 天 |
| ✅ J.5.5 | tooltip：决议 / 事件选项的 cost 行下显示 effect summary（"+5 民工 / +1 idea: war_economy / 稳定 -3%"），由 effect 自带 `summary_for_tooltip()` 生成 | 1 天 |
| J.5.6 | 集成测试：完成 GER `Four Year Plan` focus → 1 个核心 state 民工 +2 + 资源 +X + idea：`war_economy` 出现在 ideas 列表 + 政治面板可见；完成 `Oppose Hitler` → 政治面板顶部肖像切换（验 J.1 联动） | 1 天 |
| J.5.7 | **修复 Anschluss 等国策的吞并逻辑**：当前 `GER_anschluss` 用 `TransferState(4)` 只转了一个 state——如果 AUT 有多个 state 则不完整。改为 `AnnexCountry("AUS")`（已实现的 effect，会转移所有 state + 标记 annexed）；同理检查 `GER_end_of_czechoslovakia`（当前用 `TransferState(70), TransferState(71)` 手动列举）是否遗漏 CZE 的 state，如有则改为 `AnnexCountry("CZE")` 或补全所有 state id | 1 天 |

**验收**：玩家点完 4 年计划 → UI 弹出 "+2 民工(柏林) / +1 民工(鲁尔) / 添加意识：战争经济" → 立刻在生产面板看到工厂数 +4 / 政治面板看到新 idea。完成民主支线 → 政治面板顶部元首换人。

**merge 回 V5**：作为 V5 §阶段 D.2 的增量补丁记录；新增 18 个 effect 写入 ROADMAP_V5.md 阶段 D.2 完成日期下方。

---

### J.5b — 事件系统重构：国家事件 vs 世界事件 + 西班牙内战完整实装 + 局势面板 ✅ 已完成（2026-05-20）

**问题**：当前事件系统只有一种类型——所有事件都是"国家专属事件"（只对 `world.player` 即 GER 触发弹窗）。没有"世界事件 / 新闻事件"的概念。

**vanilla 对标**：
- **country_event**（国家事件）：只对特定国家弹窗。如"德奥合并"只有 GER 收到弹窗并做选择
- **news_event**（新闻事件）：全球所有国家都收到弹窗通知。如"西班牙内战爆发"所有国家都看到这条新闻（但只有部分国家有实际选项，其它国家只有"我知道了"）
- **局势系统（Situation）**：vanilla 没有这个面板，但 V5 作为独立游戏可以自研。设计参考：EU4 的"大事件进度条" + HOI4 的"内战支持度"机制 + Stellaris 的"局势日志"

**当前代码诊断**：
- `Event` struct 没有 `scope` / `event_type` 字段——所有事件隐含为 country_event
- `EventScheduler::daily_tick` 只接受一个 `country: CountryId` 参数，只对该国评估 trigger
- `hoi4-app::main` 中只对 `player` 调用 `daily_event_tick`——AI 国家完全不触发事件
- 西班牙内战（`germany.spanish_civil_war`）只是 GER 视角的"要不要派秃鹰军团"，西班牙本身没有内战机制（没有 SPR/SPA 分裂、没有战争、没有其它国家收到通知）

---

#### J.5b Part 1 — 事件 scope 重构（1.5 周）

| # | 任务 | 估时 |
|---|---|---|
| ✅ J.5b.1 | `Event` struct 加 `scope: EventScope` 字段（`enum EventScope { Country, News }`，默认 `Country`）；RON schema 加 `scope: News` 可选标注 | 0.5 天 |
| ✅ J.5b.2 | `EventScheduler` 重构：`daily_tick` 改为接受 `&[CountryId]`（所有活跃国家）而非单个 country；对 `scope=Country` 事件仍只对 trigger 中指定的国家评估；对 `scope=News` 事件对所有国家评估（trigger 通过 = 全球触发） | 1 天 |
| ✅ J.5b.3 | `PendingEvent` 加 `target_countries: Vec<CountryId>` 字段：`Country` 事件 target = 触发国；`News` 事件 target = 所有国家（或 trigger 中 `TargetCountries([...])` 指定的子集） | 0.5 天 |
| ✅ J.5b.4 | UI 弹窗区分：`Country` 事件用现有 modal（居中、有选项按钮）；`News` 事件用"报纸风格"弹窗（标题大字 + 描述 + 只有"我知道了"按钮，除非该国有特殊选项） | 1 天 |
| ✅ J.5b.5 | 对 AI 国家的事件处理：AI 收到 `News` 事件时自动选 `ai_chance` 最高的选项（已有逻辑，只需扩展到非 player 国家）；AI 收到 `Country` 事件同理 | 0.5 天 |
| J.5b.6 | 现有 43 个 GER 事件审计：标注哪些应该是 `News`（如"英法对波兰保证"、"莫洛托夫-里宾特洛甫条约"、"9.3 英法宣战"）、哪些保持 `Country`（如"Hossbach 会议"、"水晶之夜"、"Blomberg-Fritsch 危机"）；改标 scope | 1 天 |
| J.5b.7 | 集成测试：①News 事件全国弹窗 ②Country 事件只对目标国弹窗 ③AI 自动选项 ④fire_only_once 对 News 也生效 | 1 天 |

---

#### J.5b Part 2 — 局势系统（Situation System）设计与实装（2-3 周）

**设计理念**：局势（Situation）是一种**持续性的世界级事件**，有进度条、多方参与者、阶段性触发。与普通事件的区别：
- 普通事件 = 一次性弹窗 + 选项 → 结束
- 局势 = 持续数月/数年的进程，多国可以持续介入，有进度条推进，阶段性触发子事件

**局势 RON Schema 设计**：

```ron
Situation(
    id: "spanish_civil_war",
    title: "The Spanish Civil War",
    description: "Spain is torn apart...",
    icon: "GFX_situation_spain",

    // 触发条件 + 时间窗口
    start_trigger: Date(year: 1936, month: 7, day: 17),
    end_trigger: Or([Date(year: 1939, month: 4, day: 1), SituationProgress("nationalist", 100)]),

    // 参与方（faction 不是阵营，是局势内的"阵营"）
    sides: [
        SituationSide(
            id: "nationalist",
            name: "Nationalists (Franco)",
            color: [0x8B, 0x45, 0x13],
            initial_progress: 40,
            // 哪些国家默认支持这一方
            default_supporters: ["ITA", "GER"],
        ),
        SituationSide(
            id: "republican",
            name: "Republicans",
            color: [0xCC, 0x33, 0x33],
            initial_progress: 60,
            default_supporters: ["SOV", "MEX"],
        ),
    ],

    // 玩家/AI 可选的介入方式
    interventions: [
        Intervention(
            id: "send_volunteers",
            name: "Send Volunteer Division",
            cost: InterventionCost(manpower: 5000, equipment: [("infantry_equipment", 500)]),
            cooldown_days: 90,
            effect_on_side: ProgressBoost(amount: 5),
            effect_on_self: [ArmyExperience(10.0), AddModifier(name: "volunteers_abroad", duration_days: 90)],
            available: And([NotAtWar, HasManpower(5000)]),
        ),
        Intervention(
            id: "send_advisors",
            name: "Send Military Advisors",
            cost: InterventionCost(political_power: 50.0),
            cooldown_days: 60,
            effect_on_side: ProgressBoost(amount: 3),
            effect_on_self: [ArmyExperience(5.0), AirExperience(5.0)],
            available: AlwaysTrue,
        ),
        Intervention(
            id: "send_weapons",
            name: "Send Weapons & Equipment",
            cost: InterventionCost(equipment: [("infantry_equipment", 2000), ("artillery_equipment", 200)]),
            cooldown_days: 30,
            effect_on_side: ProgressBoost(amount: 4),
            effect_on_self: [],
            available: HasEquipment("infantry_equipment", 2000),
        ),
        Intervention(
            id: "send_aircraft",
            name: "Send Aircraft",
            cost: InterventionCost(equipment: [("fighter", 50)]),
            cooldown_days: 60,
            effect_on_side: ProgressBoost(amount: 6),
            effect_on_self: [AirExperience(15.0)],
            available: HasEquipment("fighter", 50),
        ),
    ],

    // 进度 tick：每周按双方 progress 差值 + 介入加成推进
    weekly_tick: SituationWeeklyTick(
        base_nationalist_gain: 1,
        base_republican_gain: 1,
        // 每个 supporter 国家的 intervention 累计 boost 加到对应 side
    ),

    // 阶段性子事件（进度到达某值时触发）
    milestones: [
        SituationMilestone(
            trigger: SituationProgress("nationalist", 60),
            event: "news.madrid_falls",  // News 事件
        ),
        SituationMilestone(
            trigger: SituationProgress("nationalist", 80),
            event: "news.barcelona_falls",
        ),
        SituationMilestone(
            trigger: SituationProgress("nationalist", 100),
            event: "news.spanish_civil_war_ends_nationalist",
        ),
        SituationMilestone(
            trigger: SituationProgress("republican", 100),
            event: "news.spanish_civil_war_ends_republican",
        ),
    ],

    // 结束效果
    on_end: [
        // nationalist 胜：SPA 吞并 SPR，Franco 执政
        // republican 胜：SPR 保留，共和政府存续
    ],
)
```

**交付**：
| # | 任务 | 估时 |
|---|---|---|
| ✅ J.5b.8 | `hoi4-content` 新增 `situation.rs`：`Situation` / `SituationSide` / `Intervention` / `InterventionCost` / `SituationMilestone` / `SituationWeeklyTick` RON schema + serde 解析 + `validate_situation()` 校验 | 2 天 |
| ✅ J.5b.9 | `SituationState` 运行时状态：`active_situations: Vec<ActiveSituation>`，每个含 `side_progress: HashMap<String, f32>` / `interventions_used: HashMap<(CountryId, String), GameDate>` / `supporters: HashMap<String, Vec<CountryId>>` | 1 天 |
| ✅ J.5b.10 | `situation_tick::weekly_situation_tick`：每周推进所有活跃局势的 progress（base_gain + 各 supporter 的 intervention boost 累计）；检查 milestones 触发子事件；检查 end_trigger 结束局势 | 1.5 天 |
| ✅ J.5b.11 | `hoi4-ui/src/situation_panel.rs` 新增 **局势面板**（快捷键 J 或从通知 feed 点击进入）：左侧局势列表（当前活跃 + 已结束历史）；右侧详情：双方进度条（nationalist vs republican）+ 支持国列表 + 介入选项按钮 | 2 天 |
| ✅ J.5b.12 | 介入交互：玩家点击"Send Volunteers" → 校验 `available` trigger + 扣 `cost`（manpower / equipment / PP）→ 写入 `interventions_used` + 给对应 side 加 `ProgressBoost` + 对自己执行 `effect_on_self`；冷却期内按钮灰显 + 倒计时 | 1.5 天 |
| ✅ J.5b.13 | AI 介入逻辑：AI 国家按 `default_supporters` 自动选边；每次 weekly tick 时，如果 AI 有足够资源且不在冷却，按 `ai_priority`（GER/ITA 优先 send_volunteers > send_weapons；SOV 优先 send_advisors > send_weapons）自动执行介入 | 1.5 天 |

---

#### J.5b Part 3 — 西班牙内战 + 意大利-埃塞俄比亚战争完整实装 ✅ 已完成（2026-05-20）

| # | 任务 | 估时 |
|---|---|---|
| ✅ J.5b.14 | 西班牙内战局势数据：`main.rs` 中 `SituationDef` 定义完整 SCW 局势（nationalist 40% / republican 60%、7 个介入选项含双方、milestone 60/80/100、on_start SplitCountry + TriggerEvent + 爆发新闻、on_end 双方吞并） | 1 天 |
| ✅ J.5b.15 | 西班牙分裂机制：`SituationEffect::SplitCountry` 已实装——on_start 时 ①spawn SPR (Nationalist Spain) ②~50% SPA 省份转给 SPR ③SPR vs SPA 战争创建 ④双方 at_war 标记；SPR 已加入 i18n | 1.5 天 |
| ✅ J.5b.16 | 局势进度 → 地图联动：milestone_effects 已配置——60% Madrid falls → TransferStates(dynamic ~1/3) + news.madrid_falls；80% Barcelona falls → TransferStates + news.barcelona_falls；ETH 战争 75%/85% 亦有 TransferStates + 新闻 | 1 天 |
| ✅ J.5b.17 | 局势结束处理：on_end[0] = SPR annexes SPA + TriggerEvent(news.scw_nationalist_victory)；on_end[1] = SPA annexes SPR + TriggerEvent(news.scw_republican_victory)；ETH: on_end[0] = ITA annexes ETH + AddIdea(east_african_empire) + news; on_end[1] = ITA stability -20% / war_support -30% + news | 1 天 |
| ✅ J.5b.18 | 介入效果可见性：`ActiveSituation.intervention_log: Vec<InterventionLog>` 记录每条介入（country_tag / intervention_name / side_name / progress_boost / army_xp / air_xp）；局势面板显示完整 Intervention Log | 0.5 天 |
| ✅ J.5b.19 | 与现有 GER 事件联动：`germany.spanish_civil_war` 已是 `is_triggered_only: true`，由局势 on_start 的 `TriggerEvent("germany.spanish_civil_war")` 触发；on_start 同时触发 `TriggerEvent("news.scw_outbreak")` 全局新闻 | 1 天 |
| ✅ J.5b.20 | **意大利-埃塞俄比亚战争局势**：`main.rs` 中 `SituationDef` 定义——开局即激活(start_date=(0,0,0))、italy 65% / ethiopia 35%、base_progress 3:1、ITA vs ETH CreateWar、3 个第三国介入选项 | 1 天 |
| ✅ J.5b.21 | 埃塞战争地图联动：milestone 75% → TransferStates + news.eth_mustard_gas；85% → TransferStates + news.eth_addis_ababa_threatened；95% → news.eth_selassie_exile；100% → on_end; ETH side color 修复为 [0x8B,0x00,0x00] | 1.5 天 |
| ✅ J.5b.22 | 埃塞战争介入选项：①"对意大利实施石油禁运"(75PP, 一次性, ethiopia +8) ②"向埃塞走私武器"(1000 步兵装备, 60d 冷却, ethiopia +3) ③"外交支持意大利殖民主张"(25PP, 一次性, italy +2) | 1 天 |
| ✅ J.5b.23 | 埃塞战争 AI 行为：`ai_auto_intervene()` 每周 tick 自动执行——AI default_supporters 按顺序尝试首个可负担介入；ETH 无 default_supporters 故 AI 几乎不介入；如果 ethiopia 到 100% → on_end 已包含 ITA 稳定度 -20% / 战争支持 -30% | 0.5 天 |
| ✅ J.5b.24 | 新增 `SituationEffect::AddOpinion` + `SituationEffect::SetCountryFlag` 变体及 main.rs 处理；`news_events.ron` 10 个 News 事件（SCW 爆发/Madrid/Barcelona/双方胜利 + ETH 芥子气/亚的斯亚贝巴/塞拉西流亡/意大利胜/埃塞胜）已创建并合并到 EventDb；`cargo build --workspace` 全绿 | 1 天 |

---

#### J.5b 验收汇总

**Part 1 验收**：
- 1939-09-03 "Britain and France declare war" 是报纸新闻，所有国家都看到
- 1938 德奥合并只有 GER 弹窗（国家事件），其它国家不弹

**Part 2 + 3 验收**：
- 1936-07-17 全球弹出报纸新闻 "Spanish Civil War Erupts" → 地图上西班牙分裂为两色
- 按 S 打开局势面板 → 看到"The Spanish Civil War"：nationalist 40% / republican 60% 进度条
- 点击"Send Volunteers" → 扣 5000 人力 + 500 步兵装备 → nationalist progress +5 → 自己 army_xp +10
- 90 天冷却后可再次派遣
- AI 国家（ITA/SOV）自动介入，面板上可见"Italy: Sent Weapons — +4 nationalist"
- 1938 年左右 nationalist progress 到 60 → 马德里陷落新闻 → 地图翻转
- 1939-04 nationalist 到 100 → 全球新闻 "Nationalist Victory" → SPA 吞并全西班牙

**merge 回 V5**：补丁挂入 V5 阶段 F.1 的"已完成"行——原 F.1 描述追加"+ 事件 scope 区分 + 局势系统 + 西班牙内战完整实装（J.5b 补丁）"。

---

### J.5c — 生产面板补全：新增生产线 + 建造面板 ✅ 已完成（2026-05-20）

> 已实装：建造队列右侧有一排建筑图标（军工/民工等 SVG），点击后所有可建造省份高亮，
> 再点击省份即自动在建筑队列新增对应建造项目。`cargo build --workspace` 全绿。

**问题**：当前生产面板（C.3 已交付）只能操作**已有的**生产线和建造队列（调整工厂数 / 删除 / 重排），但**没有任何入口让玩家新增生产线或新增建造项目**。玩家无法：
- 选择一种装备 → 开一条新生产线 → 分配军工
- 选择一个州 + 一种建筑 → 加入建造队列 → 分配民工

这意味着玩家的生产/建造完全依赖 AI 或初始数据，自己无法主动操作。

**当前代码诊断**：
- `ProductionCommand` 枚举只有 `SetFactories / RemoveLine / MoveUp / MoveDown / RemoveConstruction`——没有 `AddLine` / `AddConstruction`
- `EconomyState` 已有 `add_production_line()` 和 `enqueue_construction()` API，但 UI 没有调用入口
- 生产面板 UI 只渲染已有列表，没有"+ 新增"按钮

**当前快捷键占用**（避免冲突）：
- W/A/S/D = 地图平移
- Q = 政治 / T = 生产 / Y = 科研 / U = 外交 / I = 军队
- M = 地图模式 / F1 = debug / F2 = demo / L = 库存(J.4b) / R = 资源(J.4b) / J = 局势(J.5b)

**交付**：
| # | 任务 | 估时 |
|---|---|---|
| ✅ J.5c.1 | `ProductionCommand` 加 `AddLine { equipment_id: String, factories: u32 }` + `AddConstruction { building_key: String, target_state: StateId }` 两个变体 | 0.5 天 |
| ✅ J.5c.2 | **新增生产线 UI**：生产线列表底部加 "+ Add Production Line" 按钮 → 点击弹出 egui `Window`（装备选择器）：列出所有 `unlocked_equipments`（已解锁装备），每行 [图标 / 名称 / 类别 / 日产预估]；点击选中 → 确认 → 发出 `AddLine` 命令 + 默认分配 1 军工 | 2 天 |
| ✅ J.5c.3 | **新增建造项目 UI**：建造队列底部加 "+ Add Construction" 按钮 → 点击弹出 egui `Window`（建造选择器）：左栏选州（ScrollArea，按名称排序，显示 used/max 槽位）；右栏选建筑类型（infrastructure / industrial_complex / arms_factory / dockyard / air_base / anti_air / radar，按 `unlocked_buildings` 过滤）；槽位满的州灰显；确认 → 发出 `AddConstruction` 命令 | 2.5 天 |
| ✅ J.5c.4 | `hoi4-app::main` 处理新命令：`AddLine` → 调 `econ.add_production_line(player, equipment_id, factories)`；`AddConstruction` → 调 `econ.enqueue_construction(player, BuildOrder::new(building_key, target_state))` + 校验槽位（J.3 已交付） | 0.5 天 |
| ✅ J.5c.5 | 装备选择器数据源：从 `GameData.equipment` 中过滤 `world.countries.unlocked_equipments[player]` 已解锁的；按类别分组（Infantry / Support / Armor / Motorized / Artillery / Anti-Air / Anti-Tank / Air / Naval） | 1 天 |
| ✅ J.5c.6 | 建造选择器数据源：从 `GameData.buildings` 中过滤 `world.countries.unlocked_buildings[player]` 已解锁的 + 通用建筑（infrastructure / civ / mil / dock 始终可建）；每州显示当前 used/max 槽位 + 已有建筑数 | 0.5 天 |
| J.5c.7 | 生产线效率显示增强：新建生产线起始效率 = `BASE_FACTORY_START_EFFICIENCY`（10%），面板显示效率条 + "新线需要 ~X 天达到 50% 效率" tooltip | 0.5 天 |
| J.5c.8 | 集成测试：①玩家新增 infantry_equipment 生产线 → `econ.production[player]` 长度 +1 ②玩家新增 arms_factory 建造 → `econ.construction[player].items` 长度 +1 ③槽位满时 AddConstruction 被拒绝 ④未解锁装备不出现在选择器中 | 1 天 |

**验收**：
- 打开生产面板（T）→ 点 "+ Add Production Line" → 弹出装备选择器 → 选 "Infantry Equipment I" → 确认 → 列表多一行，效率 10%，分配 1 军工
- 点 "+ Add Construction" → 弹出建造选择器 → 左栏选"Brandenburg"（槽位 8/12）→ 右栏选"Arms Factory" → 确认 → 建造队列多一项，进度 0%
- 尝试在槽位满的州建造 → 该州灰显不可选
- 未研究"Improved Medium Tank"时，medium_tank_equipment_2 不出现在装备选择器中

**merge 回 V5**：补丁挂入 V5 阶段 C.3 的"已完成"行——原 C.3 描述追加"+ 新增生产线/建造入口（J.5c 补丁）"。

---

### J.6 — AI 建造节奏与约束 + 民用工厂建造机制修复

**问题**：用户感受"每天自动建工厂太离谱"。诊断发现**两个根因**：

**根因 A（所有国家，含玩家）**：`consumer_goods_factories` 初始化为 0 且**从未被任何代码写入非零值**。虽然 `PoliticsCache.consumer_goods_factor` 从 idea modifier 中读取了消费品占比（HOI4 默认 35%），但这个值**从未被用来计算 `econ.consumer_goods_factories`**。结果：所有国家的全部民工都可用于建造，没有消费品占用。GER 1936 起手 ~30 民工全部投入建造 → 每天 120 IC → 一个 10000 IC 的工厂 83 天就建完 → 一年建 4-5 个工厂，远超 vanilla 节奏。

**根因 B（AI 国家）**：`PRODUCTION_EVAL_CADENCE = 30` 让 70+ 国月初集中触发，外观像"每天都在建"；AI `decide_construction` 写死"非首都州 → 第一个 → push BuildOrder"，没看槽位 / 资源 / IC 缺口 / 是否有空闲民工。

**核心规则（用户 2026-05-20 明确）**：
> 1. 建设新工厂必须用原有的民用工厂建设，不能凭空建设。玩家扮演的国家也是一样。
> 2. 玩家扮演的国家不要为 AI 自动建设工厂——建造队列完全由玩家手动操作。只有非玩家国家才由 AI 自动决定建造。

这条规则在代码中**已经实现了一半**：
- `construction::tick` 确实用 `available = total_civ - consumer - export` 来限制建造速度（骨架正确）
- `orchestrator.rs:102` 已经 skip 玩家国家的 AI 评估（`if country == world.player { continue; }`）

但因为 `consumer` 永远 = 0，所以等于没限制。且需要确认 AI 的 `production::apply_production_decision` 不会被其它路径对玩家国家调用。

**交付**：
| # | 任务 | 估时 |
|---|---|---|
| ✅ J.6.1 | **修复消费品工厂计算**：在 `economy::tick_daily` 的 `resources::tick` 之后、`construction::tick` 之前，插入 `consumer_goods::tick`：`econ.consumer_goods_factories[ci] = (total_civ as f32 × (BASE_CONSUMER_GOODS_FACTOR + politics_cache.consumer_goods_factor[ci])).ceil() as u32`。HOI4 默认 `BASE_CONSUMER_GOODS_FACTOR = 0.35`（和平时期 35% 民工用于消费品）；战争经济 idea 可降到 0.20；总动员可降到 0.10。**这一条修复后，所有国家（含玩家 GER）的可用建造民工立刻从 100% 降到 65%** | 1 天 |
| ✅ J.6.2 | **玩家国家禁止 AI 自动建造**：审计所有可能对玩家国家 push BuildOrder 的路径，确保 ①`orchestrator.rs` 已 skip 玩家（已确认）②不存在其它 "auto-build" 路径（如 focus effect / event effect 中的 `AddBuildingToQueue`）对玩家国家自动入队——如果存在则改为"弹出建议通知，玩家手动确认后才入队"。玩家国家的建造队列**只能由玩家在生产面板手动操作** | 0.5 天 |
| ✅ J.6.3 | **UI 端显示**：生产面板工厂概览行改为 "民工 30 / 消费品 11 / 可用建造 19"；TopBar 的 civ 数字旁加 tooltip 解释分配 | 0.5 天 |
| ✅ J.6.4 | **available == 0 时建造队列暂停**：如果一个国家没有空闲民工（全被消费品 + 出口占满），建造队列不推进，UI 显示 "⚠ No free civilian factories"；这对玩家和 AI 同样生效 | 0.5 天 |
| ✅ J.6.5 | 把 `PRODUCTION_EVAL_CADENCE` 改成"按 country.id 散列错开"：`day_offset[country] = country.id % 30`，每国月内不同日触发；视觉上不再月初集中 | 0.5 天 |
| ✅ J.6.6 | `decide_construction` 加 4 道 gate：①目标 state 必须有空槽（J.3 已交付） ②有空闲民工（`available_civ_factories > 0`）③不与同 building_key 队列重复 ④资源短缺时优先建 industrial_complex 而非 arms_factory | 1.5 天 |
| ✅ J.6.7 | 加 `AiPersonality.production_focus_weights`：industry / military / dockyard / infrastructure / civilian 5 权重；按国家档案微调（GER → military 偏高，GBR → dockyard 偏高，POL → infrastructure 偏高） | 1 天 |
| ✅ J.6.8 | AI 生产线选择：当前可能频繁 reshuffle；加"不变更阈值 = 0.7"——只有候选生产线评分超过当前 1.4× 才切换 | 1 天 |
| J.6.9 | 通知 feed（J.8 提供入口）只展示玩家自己 + 玩家敌人 / 阵营国的建造完成；其它国静默 | 0.5 天 |
| J.6.10 | 集成测试：①GER 1936 起手 `available_civ_factories` ≈ 19（30 × 0.65 ≈ 19）②`consumer_goods_factories` ≈ 11 ③跑 365 天 GER 新建工厂数 ≈ 3-5 个（vanilla 节奏）④7 国 AI 工厂数增长曲线在历史 ±25% 内 ⑤同一日全球 BuildOrder 入队事件不超过 4 次 ⑥玩家国家建造队列在无手动操作时保持为空（AI 不自动入队） | 1 天 |

**验收**：
- 1936-01-01 GER 生产面板显示 "民工 30 / 消费品 11 / 可用建造 19"
- 跑 1 年 GER 新建工厂 3-5 个（不再是 10+ 个）
- 如果玩家把所有民工都分配给消费品（通过 idea），建造队列完全停止
- **玩家国家建造队列在无手动操作时始终为空**——AI 不会替玩家自动入队建造任务
- AI 国家发育曲线接近 vanilla（GER 109→193、SOV 130→260、GBR 45→90 类比）
- 通知 feed 不再被 AI 建造刷屏

**merge 回 V5**：补丁挂入 V5 阶段 F.3 的"已完成"行 + 经济系统 bug fix。

---

### J.7 — Feedback Bus（领域事件总线）

**关系**：`.kiro/specs/cross-system-feedback-loops/requirements.md` 已起草 9 条 requirements（Bus 基础设施 + 6 条 loop + 通知层 + 性能约束）。本阶段就是把那份 spec 推进到 design + tasks + 实现。

**交付**（直接套那份 spec 的 9 条 requirements）：
| # | 任务 | 估时 |
|---|---|---|
| ✅ J.7.1 | 设计文档 `design.md`（基于现有 requirements）：API 形态、生命周期、级联深度、错误处理、与 SystemSchedule 集成点 | 1 天 |
| ✅ J.7.2 | `FeedbackBus` 基础设施：单线程 / 非 Send / publish / subscribe / `MAX_CASCADE_DEPTH = 8` / 4096 环形日志 | 1.5 天 |
| ✅ J.7.3 | `Loop_Battle_To_Stability`：胜方 stability + war_support↑、败方 ↓ | 1 天 |
| ✅ J.7.4 | `Loop_Diplomacy_To_AI_Mood`：宣战 → 第三国 threat_perception；阵营 → opinion +25 | 1.5 天 |
| ✅ J.7.5 | `Loop_Occupation_To_Economy`：核心州被占 → owner 民工 - / new_controller +20% / 资源转移；解放回滚 | 2 天 |
| ✅ J.7.6 | `Loop_Focus_To_Diplomacy`：focus 标签 `aggressive` / `peaceful` 影响 tension + neighbour opinion | 1 天 |
| ✅ J.7.7 | `Loop_Research_To_Production`：Industry 类完成 → 立刻重算 `factory_output_factor`；Equipment 类 → AI 候选池更新 | 1 天 |
| J.7.8 | F3 调试面板：4096 事件滚动 + 类型过滤 + assert helper | 1 天 |
| J.7.9 | 性能验收：30d smoke 测试 ms/day 在引入 bus 后 < +10% | 0.5 天 |

**验收**：1937 GER 完成"莱茵兰" focus → tension +0.02 / FRA opinion -15 → AI 立刻在下一次 diplomacy eval 中把 GER threat 调高 → 通知 feed 弹出"Rhineland: neighbours react"。

**merge 回 V5**：作为新阶段 J 的核心，吸收 cross-system-feedback-loops spec（吸收后该 spec 状态改为 "absorbed into V5 §J.7"）。

---

### J.8 — Notification Feed + 关键事件自动暂停

**交付**：
| # | 任务 | 估时 |
|---|---|---|
| J.8.1 | `crates/hoi4-ui/src/notification_feed.rs`：右下角竖向滚动 list，最近 200 条，自动消失 30s 不显示，全部条目存档可在 F4 面板查看历史 | 1.5 天 |
| J.8.2 | 数据源 = FeedbackBus（J.7）+ 现有 EventScheduler；条目 = (date, kind, title, delta_summary, target_id) | 0.5 天 |
| J.8.3 | 点击条目 → 主面板路由（state→定位地图、country→外交面板、focus→focus 面板）| 1 天 |
| J.8.4 | 自动暂停：`Settings.auto_pause` 已实现的列表扩到包含 `WarDeclared` / `FocusCompleted` / `StateOccupied`（仅 player 相关）/ `ImportantEventFired` | 0.5 天 |
| J.8.5 | 设置面板加每类事件的"自动暂停 / 仅通知 / 静默"3 档 toggle | 1 天 |

**验收**：跑游戏 1939-09-01 宣战 POL → 自动暂停 + 屏幕右下"Germany declared war on Poland" → 点击条目跳外交面板高亮 POL → 1939-09-15 华沙占领 → 再次自动暂停 + "Warsaw fell to Germany"。

**merge 回 V5**：作为新阶段 J 的最终用户层。

---

### J.9 — 军事通行权（Military Access）

**问题**：当前 `hoi4-logic::military::movement::daily_movement_tick` 完全没有通行权检查。师可以在和平时期自由穿越任何外国领土——这在 HOI4 原版中是绝对不允许的。原版规则：
- 师只能进入 ①自己国家控制的省份 ②同阵营成员控制的省份 ③已授予军事通行权的国家控制的省份 ④正在交战的敌国控制的省份
- 和平时期进入外国领土 = 不可能（除非有通行权或同阵营）

**当前代码诊断**（`crates/hoi4-logic/src/military/movement.rs`）：
- `daily_movement_tick` 遍历所有师，只要有 `destination` 就 BFS 走一步
- 唯一的"外交检查"是**到达后**：如果目标省份 controller 与师 owner 处于战争状态，则翻转 controller
- **没有任何"能不能进入"的前置检查**——和平时期师可以走到巴黎、莫斯科、伦敦

**外交系统现状**：
- `DiplomacyState.factions: Vec<Faction>` 已存在，`faction_of(country)` 可查
- `DiplomacyState` 已有 `NavalAccess` 条约类型（`WarGoalType::NavalAccess`），但没有 `MilitaryAccess` 条约
- 没有 `has_military_access(from, to)` 查询函数

**交付**：
| # | 任务 | 估时 |
|---|---|---|
| ✅ J.9.1 | `hoi4-state::diplomacy` 加 `MilitaryAccess` 条约类型 + `DiplomacyState.military_access: HashSet<(CountryId, CountryId)>`（(grantor, grantee) 对）+ `grant_military_access` / `revoke_military_access` / `has_military_access(from, to)` API | 0.5 天 |
| ✅ J.9.2 | `hoi4-logic::military::movement` 加 `can_enter_province(world, division_owner, target_province) -> bool` 函数：返回 true 当且仅当 ①target controller == owner ②target controller 与 owner 同阵营 ③owner 对 target controller 有 military_access ④owner 与 target controller 处于战争状态 ⑤target controller == NONE（无主地） | 1 天 |
| ✅ J.9.3 | `daily_movement_tick` 中 BFS 选下一步时调用 `can_enter_province`：如果 next_step 不可进入，则 **停止移动**（`destinations[i] = None`，师原地待命）；不 panic、不跳过、不绕路（简化版；绕路留后续） | 1 天 |
| ✅ J.9.4 | 玩家右键下达移动命令时（`hoi4-app::main` 中 `set destination`），前端预检 `can_enter_province(dest)`：如果目标省份不可进入，弹出 toast "Cannot enter — no military access"，不设 destination | 0.5 天 |
| ✅ J.9.5 | AI 移动逻辑（`hoi4-ai::ground` / `hoi4-ai::frontline`）在选择目标省份时也走 `can_enter_province` 过滤；AI 不会尝试穿越无通行权的外国 | 1 天 |
| ✅ J.9.6 | 同阵营自动通行：`faction::join_faction` / `create_faction` 时不需要显式 grant——`can_enter_province` 直接查 `same_faction`；离开阵营时自动失去通行权 | 0.5 天 |
| ✅ J.9.7 | 宣战时自动获得对敌国的"进入权"（已隐含在 ④ 条件中）；和平条约签订后自动撤销（师如果还在敌国领土 → 强制传送回首都省份，简化处理） | 0.5 天 |
| ✅ J.9.8 | 外交面板加"请求军事通行权"按钮（AI 对手按 opinion > 50 + 同 ideology 自动同意；否则拒绝）；决议 / effect 也可 `GrantMilitaryAccess(target)` | 1 天 |
| J.9.9 | 集成测试：①1936 GER 师设 destination = 巴黎 → 师不动（FRA 非同阵营、无通行权、未宣战）②GER 宣战 POL 后师可进入 POL 省份 ③GER 加入 Axis + ITA 加入 Axis → GER 师可进入 ITA 省份 ④GER 请求 HUN 通行权（opinion > 50）→ 获批 → 师可进入 HUN | 1 天 |

**验收**：
- 1936-01-01 GER 师右键点巴黎 → toast "Cannot enter — no military access"，师不动
- 1939-09-01 宣战 POL → 师可以正常推进到华沙
- GER + ITA 同阵营 → GER 师可以穿越意大利领土
- 外交面板对匈牙利点"Request Military Access" → 获批 → 师可以穿越匈牙利
- AI 师在和平时期不会出现在外国领土上（除非同阵营）

**merge 回 V5**：作为新阶段 J 的子节。这是基本游戏规则修复，优先级仅次于 J.1。

---

### J.10 — AI 师移动执行层（Plan → Movement）

**问题**：当前 AI 的陆军战术模块（`hoi4-ai::ground`）只做**评估**——输出"这段前线应该进攻/防御/撤退"的姿态决策，但**没有任何代码把决策转化为实际的师移动命令**。`orchestrator.rs` 收集到 `GroundPosture::Attack` 后直接丢弃，从不设置 `world.divisions.destinations[i]`。

结果：AI 国家的师在整场战争中**原地不动**。宣战后敌方师不会推进、不会防御、不会撤退。战斗只在初始接触线上发生（因为 `arbiter.rs` 检测相邻省份的对立 controller），但没有任何推进。

**代码诊断**：
- `ground::evaluate_ground()` → 输出 `GroundEvaluation { decisions: [FrontDecision { posture: Attack, ... }] }`
- `orchestrator.rs` 第 175-180 行：调用 `ground::evaluate_ground` 后只写日志，不执行任何移动
- `movement::daily_movement_tick()` 只处理已有 `destinations` 的师——但 AI 从不设置 destination
- 注释明确写着："调度具体师从 A → B：需要 movement / supply / order 系统"——这部分从未实现

**交付**：
| # | 任务 | 估时 |
|---|---|---|
| ✅ J.10.1 | `hoi4-ai/src/ground_orders.rs` 新增模块：把 `GroundEvaluation` 翻译成具体的师移动命令。核心逻辑：①`Attack` 姿态 → 找前线上我方师 → 设 destination 为最近的敌方控制省份 ②`Defend` → 师不动（保持当前位置）③`Retreat` → 设 destination 为后方省份 | 3 天 |
| ✅ J.10.2 | 师分配算法（简化版）：按前线段分配师——每段前线按"敌方师数 × 1.2"分配我方师；多余师集中到 `Attack` 段作为突击力量；不足时从 `Retreat` 段抽调 | 2 天 |
| ✅ J.10.3 | 进攻目标选择：`Attack` 姿态时，选择目标省份的优先级 = ①VP 省份 ②敌方工厂州 ③包围目标（`encirclement_targets`）④最近的敌方省份（fallback） | 1.5 天 |
| ✅ J.10.4 | `orchestrator.rs` 接入：在 tactical eval 之后调用 `ground_orders::execute_ground_orders(world, country, &eval)`，真正设置 `divisions.destinations` | 0.5 天 |
| ✅ J.10.5 | 防止师"抖动"：如果师已经在移动中（destination != None），不要每次 eval 都重设目标；只有 ①到达目标 ②姿态从 Attack 变 Retreat ③目标省份已被占领 时才重新分配 | 1 天 |
| ✅ J.10.6 | 与 J.9（通行权）集成：`execute_ground_orders` 设置 destination 前调用 `can_enter_province` 校验；不可进入的目标跳过 | 0.5 天 |
| J.10.7 | 集成测试：①GER 宣战 POL → POL 的师在 7 天内开始向前线移动（destination != None）②GER AI 师在 Attack 姿态时向华沙方向推进 ③30 天后至少有 1 个 POL 省份被 GER 占领 ④Defend 姿态的师不乱跑 | 1.5 天 |

**验收**：
- 1939-09-01 GER 宣战 POL → POL 师立刻开始向边境移动防御 → GER 师向波兰推进
- 30 天内华沙被占领（力量比 GER >> POL 时）
- AI 不会把师送到无法进入的省份（通行权检查）
- 防御姿态的师不会乱跑到前线后方

**merge 回 V5**：作为新阶段 J 的子节。**这是让战争"能打起来"的关键缺失**——没有这个，宣战后双方师原地站桩。

---

### J.11 — 国策面板改版（HOI4 原版风格）

**问题**：当前国策面板是一个小窗口（`egui::Window`），节点是圆形，没有拖拽平移，没有国策图标，"开始研究"按钮在顶部工具栏而不是节点旁边，连线是直线穿过其它节点，整体不像 HOI4。

**改版目标**：让国策面板的操作体验接近 HOI4 原版。

**交付**：
| # | 任务 | 估时 |
|---|---|---|
| J.11.1 | 面板改为近全屏覆盖（80%×85% 屏幕），深色木纹背景（#1a1510），不再是可拖动小窗口 | 1 天 |
| J.11.2 | 打开面板时自动暂停游戏，关闭时恢复之前的速度 | 0.5 天 |
| J.11.3 | 左键拖拽画布平移（替代滚动条），滚轮缩放（0.5×–2.0×） | 1 天 |
| J.11.4 | 节点从圆形改为方形卡片（80×80），中心纯色占位图标（52×52），底部 12px 名字标签 | 1 天 |
| J.11.5 | 节点间距从 110px 加大到 150px，给节点呼吸感 | 0.5 天 |
| J.11.6 | 节点四态视觉：完成=绿边+勾、进行中=黄边+进度条、可选=亮色、锁定=暗灰 | 1 天 |
| J.11.7 | 连线从直线改为 L 形折线（垂直→水平→垂直），不穿过其它节点 | 1 天 |
| J.11.8 | 顶部当前研究进度条："正在研究: 莱茵兰 ████████░░ 45/70 天 [取消]" | 0.5 天 |
| J.11.9 | 底部详情栏：选中节点后显示 [名字 / 天数 / 前置 / 效果摘要 / 开始按钮]；效果摘要显示具体数值（"+2 民工(柏林)"） | 1.5 天 |
| J.11.10 | `Effect` 枚举加 `effect_summary()` 方法：每个 effect 变体返回中文可读字符串 | 1 天 |
| J.11.11 | 更新 `main.rs` 调用点：传入当前游戏速度，处理暂停/恢复命令 | 0.5 天 |

**验收**：
- 按快捷键打开国策面板 → 游戏暂停 → 看到近全屏深色面板
- 左键拖拽画布自由平移，滚轮缩放
- 节点是方形卡片，已完成的有绿色勾，正在研究的有黄色进度条
- 连线是 L 形折线，不穿过其它节点
- 点击一个节点 → 底部详情栏显示"四年计划 / 70天 / 需要: 莱茵兰 / 效果: +2 民工(柏林) +2 军工(鲁尔) / [▶ 开始研究]"
- 顶部进度条显示当前研究进度 + 取消按钮
- 关闭面板 → 游戏恢复之前的速度

**merge 回 V5**：补丁挂入 V5 阶段 D.4 的"已完成"行。

---

## 4. 不在本路线图（明确）

避免范围蔓延，下列条目不放进阶段 J；如要做需另写 spec：
- 重做战斗判定算法
- 海军 / 空军任务系统升级
- 战争经济（民转军 / 配给 / 出口许可）—— 与 J.5 部分重叠但要更深时另立
- 谍报 / MIO / 合法性 / 和平会议（DLC 等价）
- mod / Steam Workshop
- 多国 focus 树（V5 §0.1 明确仅 GER）

---

## 5. 风险与缓解

### 5.1 J.1（Leader & Portrait）资产路径漂移
不同 HOI4 版本 / 语言版的肖像 DDS 文件名规则可能不一致（`Portrait_Germany_Adolf_Hitler.dds` vs `GER_Adolf_Hitler.dds`）；早期 vanilla 把 leader 数据放 `history/countries/*.txt` 而非 `common/characters/`，后期 patch 拆分了。

**缓解**：
- `common/characters/*.txt` 解析失败时回退读 `history/countries/<TAG>.txt` 中的 `create_country_leader = { ... }` 块（vanilla 老格式）
- 肖像查找走 `hoi4-paths::find` 已有 fallback 链：`gfx/leaders/<TAG>/Portrait_<Country>_<Name>.dds` → `gfx/leaders/<TAG>/<TAG>_<Name>.dds` → `gfx/leaders/<TAG>/<Name>.dds`
- 缺失肖像降级为"国旗大图 + 文本名"，启动 banner 警告但不致命；测试断言 GER/SOV/GBR/FRA/ITA/JAP/USA 7 大国必须命中肖像（命中率 < 100% 时启动 banner 列名单）

### 5.2 J.4（Stockpile 闭环）改动 ripple 太广
连接 production / template / division / combat / training 五处。

**缓解**：每条子任务独立测试 + integration test 必须先红再绿；J.4.1 完整审计文档先于代码（半天投入换两天稳定）。

### 5.3 J.7（Feedback Bus）级联爆炸
6 条 loop 互相 publish 容易死循环（占领触发经济重算 → 经济重算又触发研究重算 → ...）。

**缓解**：`MAX_CASCADE_DEPTH = 8` 硬上限 + 每条 loop 写明"我可能 publish 谁"的依赖图（见 J.7.1 design.md）。

### 5.4 玩家"自动建工厂还是离谱"的真正根因已确认
经代码审计确认：`consumer_goods_factories` 初始化为 0 且从未被写入非零值。`PoliticsCache.consumer_goods_factor` 虽然从 idea modifier 中读取了消费品占比，但**从未被用来计算 `econ.consumer_goods_factories`**。这意味着所有国家 100% 民工可用于建造。

**修复方案**（J.6.1）：在 `tick_daily` 中插入 `consumer_goods::tick`，按 `BASE_CONSUMER_GOODS_FACTOR = 0.35` + idea modifier 计算消费品占用。修复后 GER 1936 可用建造民工从 30 降到 ~19，建造速度立刻降 35%。这对玩家和 AI 同样生效——用户明确要求"玩家扮演的国家也是一样，必须用民用工厂建设新工厂"。

### 5.5 治理风险：本路线图不并入 V5 长期共存
V5 §1 / §8.3 已明确教训。

**缓解**：J.1 起手时必须同步在 ROADMAP_V5.md 加阶段 J 表头；J.8 完成时本文件 → `docs/legacy/ROADMAP_DEPTH.md`，主路线只剩 V5。

---

## 6. 与现有 spec 的关系

| 已有 spec | 处理 |
|---|---|
| `.kiro/specs/cross-system-feedback-loops/requirements.md` | J.7 吸收；spec 完成时把状态改为 "absorbed into V5 §J.7" 不再独立推进 |
| `.kiro/specs/playable-core-reboot/` | 历史 spec，已基本被 V5 阶段 A-G 覆盖；不再激活 |
| `.kiro/specs/assets-skeleton/` | 历史 spec（Phase 2.1 V3 时代），已废止 |

---

## 7. 验收里程碑

| 里程碑 | 满足条件 |
|---|---|
| **M-J1**（J.1 完成） | ✅ 玩家说"政治面板像个游戏了"——元首肖像 + 党全称 + 外交面板每国带肖像；用户 2026-05-20 指定的最高优先项交付（2026-05-20 完成） |
| **M-J1.5**（J.9 + J.10 完成） | ✅ 玩家说"军队不再乱跑了 + 战争能打起来了"——和平时期师不能进入外国领土，同阵营可以自由通行，宣战后AI师真正向敌方推进（2026-05-20 完成） |
| **M-J2**（J.2 完成） | ✅ 玩家说"地图能玩了"——左键省份有完整 InfoCard，能看 owner / 资源 / 驻军（2026-05-20 完成，J.2.5 面板跳转留后续） |
| **M-J3**（J.3 + J.5 + J.5b 完成） | ✅ 玩家说"国策决议真的在改世界了 + 事件有区分了"——焦点完成会看到 +N 工厂、idea 上线，建工厂槽位会满；西班牙内战是全球新闻，德奥合并是德国专属（2026-05-20 完成） |
| **M-J4**（J.4 + J.4b 完成） | ✅ 玩家说"打仗有意义了 + 经济看得见了"——师 strength 与 stockpile 有耦合，资源缺口能感知；库存面板显示装备收支，资源面板显示 6 种战略资源明细（2026-05-20 完成） |
| **M-J5**（J.7 + J.8 完成） | ✅ 玩家说"世界活了"——focus / 战斗 / 占领 / 研究 / 外交 5 类事件互相触发，FeedbackBus 6 条 loop 实装（2026-05-20 J.7 完成，J.8 通知 feed 留后续） |
| **M-J6**（J.6 完成） | ✅ 玩家说"建造节奏正常了"——消费品占用 35% 民工，GER 一年建 3-5 个工厂（不再 10+），AI 发育曲线接近历史（2026-05-20 完成） |

完成全部 6 个里程碑后，本文件归档到 `docs/legacy/ROADMAP_DEPTH.md`，所有条目 merge 回 ROADMAP_V5.md 阶段 J。

---

**版本**：DEPTH 0.4（J.5b Part 3 完整实装 — 西班牙内战+意大利-埃塞俄比亚战争+10个News事件+AI自动介入+介入日志+SituationEffect扩展）
**起草**：2026-05-20
**作者**：Ironheart 项目维护者 + Kiro CLI agent

