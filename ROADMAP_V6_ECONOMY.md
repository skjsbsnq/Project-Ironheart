# Project Ironheart V6 — 经济·生产·贸易·法律·建筑·科技 重构路线图

> **本文档是 V6 设计文档草案，等待用户 signoff 后才能开始编码。**
> 写于 2026-05-21；7 个关键决策点已由用户锁定（见 §2）。
> 与 [`ROADMAP_V5.md`](./ROADMAP_V5.md) 的关系：V5 仍为顶层独立游戏路线图、约束范围与里程碑节奏；
> V6 是 V5 §5 阶段表中**经济/政治/科技/建造**那几节的**深度替换方案**。
> V5 第 0.1/0.2 节的范围约束（仅 GER、1936-1940、不做 mod、不做引擎兼容）全部继承到 V6。


## 0. 使命

把当前 HOI4 vanilla 简化版的经济栈，替换成一个**以 1936 年帝国主义阶段为基调**的
Vic3 风格 POP + 商品市场 + GDP 经济体系，并补完目前**字面意义上不存在**的：
法律切换系统、贸易系统、税收/债务系统；把建筑系统从 4 种 state-slot 扩展为
Vic3 风格的"建筑等级 × 雇佣 POP × 生产方法（PM）"，把科技从"解锁装备"升级到
"解锁生产方法"。**战争指挥系统不动**，仍由 V5 §5 描述。

### 0.1 范围内
- ✅ 重写 `crates/hoi4-logic/src/economy/` 整个模块（保留 manpower 字段，重做语义）
- ✅ 新建 `crates/hoi4-logic/src/laws/`、`crates/hoi4-logic/src/finance/`、`crates/hoi4-logic/src/market/`、`crates/hoi4-logic/src/pops/`
- ✅ 扩展 `StateStore`、`CountryStore`、`EconomyState` schema（建筑列表化、POP 列表化、新增财政字段）
- ✅ 新建 `crates/hoi4-content/content/economy_v6/*.ron`（数据从零写，**不**沿用 vanilla TXT 数值）
- ✅ 新建 UI 面板：市场视图、财政视图、法律视图、POP 视图、新建造/生产视图
- ✅ 保留**装备库存**（`econ.stockpile[ci]: HashMap<equipment_id, f32>`）作为军用一等公民
- ✅ 引入军工建筑（Arms Industries / Munition Plants / Shipyards）作为"市场 → 装备库存"的接口

### 0.2 范围外
- ❌ 战争/前线/战斗/补给/师组成/将领 — 沿用 V5
- ❌ 文化 / 信仰 / 富裕度多维 POP — 简化为 6 阶级 × 职业（§4.1）
- ❌ 多国 POP 模拟 — 仅 GER 全 POP；其余国家用"等效 GDP + 简化贸易代理"
- ❌ Vic3 全市场（按州/地区市场聚合）— V6 全国一个市场（§4.3）
- ❌ Mod、多人、与 vanilla `.hoi4` 存档兼容 — 同 V5
- ❌ 跨国科技转让 / 技术封锁 — 详见 §4.7.3，留给 V7+

### 0.3 1936 帝国主义特色（必须建模，不能"留给未来"）
- 自由市场 vs **垄断资本主义**：大企业受国家补贴、托拉斯化、生产方法被国家钦定
- vs **苏联式计划经济**：完全独立的 plan-quota tick（非"市场+开关"），见 §4.6
- MEFO 票据式**隐性国债**：发债不上账本但累计、爆雷条件（见 §4.5）
- **双货币层（方案 A，2026-05-21 锁定）**：国内现金流以马克 RM 记账，对外贸易 / 外汇储备 / MEFO 阈值 / 信用评级以英镑 £ 记账；汇率由金储备 + 外汇储备 + Trade 法档位三者动态决定
- 金本位崩溃 / 外汇管制：贸易需要硬通货（£），封锁下贸易归零
- 战时配给制（Rationing）：消费品供应不足时按法律自动配给，POP 满意度而非市场清空


## 1. 与历代路线图的关系

| 文档 | 状态 | V6 关系 |
|---|---|---|
| `ROADMAP_V5.md` | 主路线 | V6 替换 V5 §5 中经济/政治/科技/建造相关节，其余沿用 |
| `ROADMAP_V3.md` / 附录 / GUI parity | 已归档 | 无 |
| `ROADMAP_MAP_VISUAL_PARITY.md` | 已交付 | 无影响 |
| `ROADMAP_DEPTH.md` / `ROADMAP_SITUATION_REFACTOR.md` | 现存子路线 | V6 起草期间暂不合并；V6 锁定后单独评审是否归档 |
| `HOI4_VANILLA_AI_NOTES.md` | 参考 | 战争 AI 沿用，经济 AI 重新设计（§5.4） |

> 沿用 V5 §1 的总原则：V6 之外**不允许新增经济相关补丁式子文档**。
> 新发现的差距直接 PR 到本文档。


## 2. 关键决策点（2026-05-21 锁定）

以下 12 条由用户在 2026-05-21 明确决定，作为 V6 全文不可动摇的前提：

| # | 决策点 | 选定方案 |
|---|---|---|
| D1 | POP 粒度 | **简化**：6 阶级（农民/工人/职员/资本家/贵族/士兵）× 职业槽。无文化/信仰/富裕度维度 |
| D2 | 建造力（construction points） | **全国单池**。state 基建（infra）保留为产出乘子 |
| D3 | civ/mil/dock 三池替代 | 引入**军工建筑**（Arms Industries / Munition Plants / Shipyards）。从市场买原料 → 产出仍入现有装备库存。**装备库存与商品市场两轨并存** |
| D4 | 苏联计划经济建模 | **两套独立 tick**（市场清算 vs 计划配额），由"经济体系法"切换。**不**做"市场+开关"折中 |
| D5 | 税收 / 债务 | **必做**。含 1936 帝国主义特色：金本位崩溃、外汇管制、战争国债、MEFO 票据式隐性国债 |
| D6 | 数据层 | **新路径** `content/economy_v6/*.ron`，**不**复用 vanilla TXT loader。数值从零写以脱离 paradox 数值绑架 |
| D7 | 法律 schema | **6 大类**：Conscription / Economy / Trade / Taxation / Civil Rights / Information Control（后两类抓 1936 极权特色） |
| D8 | 货币体系 | **方案 A 双货币层**：国内 RM、对外 £。汇率由金/外汇/Trade 法决定。**不**做单货币简化 |
| D9 | 事件语言 | V6 新增事件**一律中文**标题 + 中文 body，与现有 `GER_events.ron` 一致 |
| D10 | 法律切换 PP 消耗 | **按目标档难度阶梯**；每档在 RON 中显式配 `pp_cost` |
| D11 | 阶级流动 | **打开**。季度级别 tick 内跨阶级迁移；不做跨州迁移 |
| D12 | 装备 schema 整合 | **整合为 12 类军备**（详见 §4.2.2），vanilla ~50 archetype 归并 |

> 任何 V6 章节内部如与上述决策冲突，以本表为准。


## 3. 当前栈现状摘要（2026-05-21 审计）

为避免重复 §2 锁定的决策依赖，把审计结果定格在这里。后续阶段引用 `§3.x`。

### 3.1 经济
- `crates/hoi4-logic/src/economy/{mod, resources, production, stockpile, construction, manpower, fuel, constants}.rs`
- 7 种战略资源（Oil/Aluminium/Rubber/Tungsten/Steel/Chromium/**Coal**），定义在 `crates/hoi4-data/src/resource.rs:5`
- `ResourceBalance.imported/exported` + `EconomyState.export_factories` 字段存在但**永不被写入**
- 0 处 money / tax / debt / budget / treasury 相关代码

### 3.2 生产与库存
- `production.rs`：HOI4 线性生产线，效率 `eff += 0.01·(cap−eff)`，硬编码 mil/civ/dock 三池
- `stockpile.rs`：仅装备（`HashMap<equipment_id, f32>`），用于补员。**不是**仓储

### 3.3 建造
- `StateStore` 只有 4 个建筑字段（`infrastructure / civilian_factories / military_factories / dockyards`，`store.rs:54-58`）
- `construction.rs:201-203` 明说 air_base / anti_air / radar 等点了照样烧 CIC 但**无任何 store 字段变化**（silent no-op）

### 3.4 法律 / 政治
- 无 `Law` 类型，无 `active_laws` 字段
- 法律塞在 `Country.ideas: Vec<String>` 内，靠 idea modifier 表达
- `AddConscription` effect 在 `crates/hoi4-content/src/eval.rs:189` 是空 stub（`/* tracked as modifier */`）
- `crates/hoi4-ui/src/politics.rs`（460 行）无法律切换 UI

### 3.5 科技
- `research.rs`：HOI4 vanilla 简化版，所有科技数据从 vanilla `common/technologies/*.txt` 加载
- `hoi4-content/` 内**0 个 RON 科技定义**

### 3.6 内容
- `crates/hoi4-content/content/` 共 **4 个 RON**：`GER_focus_tree / GER_events / GER_decisions / news_events`
- 其余全部从 vanilla TXT 解析（loader.rs ~1500 行）


## 4. 目标模型（V6 实质设计）

### 4.1 POP 模型（兑现 D1）

#### 4.1.1 类型
```rust
// crates/hoi4-state/src/pops.rs（新文件）
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PopClass {
    Peasant,     // 农民（农业 / 半失业人口蓄水池）
    Worker,      // 工人（工厂/矿场/铁路 等级 0-3）
    Clerk,       // 职员（金融/服务/官僚 / 研究院）
    Capitalist,  // 资本家（重工业 / 军工 / 银行所有者）
    Aristocrat,  // 贵族（地产 / 军官团传统社会基础）
    Soldier,     // 士兵（含义务兵；上限受征兵法约束；详见 §4.1.4）
}

pub struct PopGroup {
    pub class: PopClass,
    pub state: StateId,
    pub size: u32,                 // 人口数（人）
    pub employed_at: Option<BuildingId>,  // 雇主；None=失业；Soldier 的雇主=Conscription Center 或 DivisionId
    pub wage_rm: f32,              // 日工资（RM/人，国内现金流统一用马克，见 D8）
    pub satisfaction: f32,         // 0.0..1.0
    pub political_loyalty: f32,    // -1..1（对当前执政党的支持度）
}
```

> SoA 化由实现期决定；声明用 AoS 便于阅读。

#### 4.1.2 数量级目标（GER 1936）
- 全德约 6700 万人口
- 全游戏存活 PopGroup 数 < 5000（≈ 70 州 × 6 阶级 × 平均 12 个职业槽聚合后）
- 单 tick PopGroup 更新预算：< 1ms

#### 4.1.3 生命周期 tick（每日）
1. **就业匹配**：每建筑按当前 PM 计算"所需工人/职员/资本家"，去本州 PopGroup 池吸人；吸不满则该工位空缺、降产出。Soldier 阶级**不参与**这一步（走 §4.1.4 独立路径）
2. **工资支付**：建筑现金流 → POP `wage_rm` 字段
3. **消费**：POP 按阶级×wage 买商品（食物、日用品、奢侈品、服务），形成市场需求；消费支出 50% 计入 `consumption_tax`（详见 §4.5.2）
4. **满意度**（公式 inline §9 P1）：
   ```
   satisfaction_target =
       0.4 * 消费实现率                         // POP 想买的商品实际买到的比例
     + 0.2 * (1 - 失业率)
     + 0.2 * (1 - tax_burden)                  // tax_burden = (income_tax + consumption_tax) / wage
     + 0.2 * law_modifier                      // 见 §4.1.5 表
   satisfaction += 0.02 * (satisfaction_target - satisfaction)  // EMA 平滑
   ```
   配给制（计划经济）下满意度公式不同，见 §4.6.3
5. **政治忠诚**：`loyalty += f(satisfaction, info_control_modifier, ruling_party_idea_modifier)`；Info Control 法档越高，loyalty 越不受 satisfaction 影响（白色恐怖效应）
6. **人口增长**：月级别，年增长率 0.5%-1.5%，受失业率/满意度调制
7. **阶级流动**（D11 锁定打开）：季度级别，工人→职员、农民→工人 等迁移，由就业供需驱动；具体公式 §4.1.6

> 跨州迁移在 V6 内**不做**，写死在本州内流动以省算力。

#### 4.1.4 Soldier 阶级 ↔ 师人力池统一（§9 P5，P0 级关键补丁）

**原现状**：`world.countries.manpower: u64`（抽象人力池）与未来 PopGroup 完全脱钩。
**V6 强制**：废弃 `manpower: u64`，改为派生量：

```rust
// crates/hoi4-state/src/store.rs（V6 修改）
impl CountryStore {
    pub fn manpower(&self, ci: CountryId) -> u64 {
        self.pops_by_class(ci, PopClass::Soldier)
            .iter().filter(|p| p.employed_at.is_none())  // 仅未编入师的 Soldier
            .map(|p| p.size as u64).sum()
    }
}
```

- **Soldier 上限**：由 Conscription 法决定，`max_soldier = total_population × law.soldier_ratio`（详见 §4.1.5）
- **造师**：从 Soldier 池扣 `division_template.manpower_cost`；`employed_at = Some(DivisionId)`
- **退役/解散师**：Soldier 回池，`employed_at = None`
- **训练 / 战斗损失**：师人力 -1 = Soldier 池 -1（直接通过 `employed_at = Some(div)` 索引找到对应 PopGroup 扣 size）
- **新征**：每月按 `(max_soldier - current_soldier)` 的 5% 从 Worker/Peasant/Clerk 池转入 Soldier 池（转出方 size 减、Soldier 新 PopGroup +）；高档征兵法（总动员）转化率上调到 15%
- **新征的反作用**：被转走的工人/农民离开建筑岗位 → 工位空缺 → 民用产出下降；这是"动员伤经济"的真实链路

**禁止**：任何代码读取 `world.countries.manpower` 后将其与 `PopGroup` 分开维护。V6.A 验收必须 grep 确认 `pub manpower: Vec<u64>` 字段已从 `CountryStore` 删除（仅保留方法 `fn manpower()`）。

#### 4.1.5 法律对 POP 的 modifier 表（§9 P3）

| 法律类目 | 档位 | 对 POP 影响 |
|---|---|---|
| Conscription | 志愿兵 | `soldier_ratio = 1.0%`；满意度 +0.05 |
| Conscription | 有限征兵 | `soldier_ratio = 2.5%` |
| Conscription | 广泛征兵 | `soldier_ratio = 5.0%`；满意度 −0.05 |
| Conscription | 总动员 | `soldier_ratio = 10%`；满意度 −0.15；强制转化率 ×3 |
| Economy | 自由放任 | Capitalist `wage` ×1.5，Worker `wage` ×0.8 |
| Economy | 干预经济 | wage 拉平；满意度 +0.05 |
| Economy | 战时经济 | Worker `wage` ×0.7；消费品供给 ×0.7 |
| Economy | 计划经济 | 走 §4.6 配给制公式，本表不适用 |
| Civil Rights | 开放社会 | 满意度 +0.10；loyalty 受 satisfaction 影响系数 ×1.5 |
| Civil Rights | 有限权利 | 中性 |
| Civil Rights | 国家安全法 | 满意度 −0.05；loyalty 受 satisfaction 影响系数 ×0.7 |
| Civil Rights | 警察国家 | 满意度 −0.15；loyalty 受 satisfaction 影响系数 ×0.3 |
| Information Control | 自由新闻 | loyalty 滑落速度 ×1.5 |
| Information Control | 监管新闻 | 中性 |
| Information Control | 国营媒体 | loyalty 滑落速度 ×0.5 |
| Information Control | 全面宣传 | loyalty 滑落速度 ×0.2；满意度 −0.05 |
| Taxation | (4 档) | 直接驱动 §4.5.2 税率；通过 `tax_burden` 项影响满意度 |
| Trade | (4 档) | 通过 §4.9 影响商品供给 → 消费实现率 → 满意度（间接） |

#### 4.1.6 阶级流动公式（D11）

季度 tick（每 90 天）：

```
flow(A → B) = pop[A].size × 0.02 × max(0, demand[B] - supply[B]) / supply[B]
```

允许的迁移路径（单向）：
- Peasant → Worker（工业化吸纳）
- Worker → Clerk（白领化）
- Clerk → Capitalist（很慢，年最多 0.5%）
- 任何阶级 → Soldier（按 §4.1.4 新征机制，不走本公式）

反向流动（如 Worker 失业回 Peasant）仅在该阶级失业率 > 30% 时触发，月 tick。

### 4.2 商品（goods）模型

#### 4.2.1 商品清单
| 类别 | 商品 | 来源 | 解锁科技 |
|---|---|---|---|
| 原料 | Coal / Steel / Aluminium / Tungsten / Chromium / Oil | 沿用现有 7 资源；从 ResourceKind 提升为 GoodKind 子集 | 开局解锁 |
| 原料 | Rubber | 同上 | 开局解锁；合成 Rubber 走 PM |
| 原料 | Synthetic Rubber / Synthetic Oil | 化学合成 | Chemistry: 合成材料 |
| 中间品 | Machinery / Chemicals | 工业建筑 | 开局解锁 |
| 中间品 | Electrical Goods | 电气工业 | Electrical: 电气基础 |
| 中间品 | Engines | 内燃机制造 | Industry: 内燃机 |
| 中间品 | Aircraft Parts | 航空中间品 | Aviation: 全金属机体 |
| 中间品 | Radar Sets | 雷达组件 | Electrical: 雷达 |
| 民生 | Grain / Meat / Clothes / Furniture | 基础消费 | 开局解锁 |
| 民生 | Liquor / Tobacco / Luxury Goods | 高级消费 | 开局解锁 |
| 服务 | Transport / Banking / Telegraph | 职员阶级提供 | 开局解锁 |
| 军备半成品 | Small Arms / Artillery Shells | 轻武器组件 | 开局解锁 |
| 军备半成品 | Tank Hulls / Aircraft Frames | 重武器组件 | Industry: 焊接装甲 / Aviation: 全金属机体 |

> **§9 P7 inline**：goods.ron 每条加 `unlocked_by: Option<TechId>` 字段；未解锁前不进 market 清单（既不能 supply 也不能 demand）。
> **D3 硬边界重申**：装备成品（如 `infantry_equipment_1`）**不**进商品市场，仍存装备库存（`econ.stockpile[ci]`）。军备半成品才进市场。

#### 4.2.2 装备 schema 整合（D12 / Q5）

vanilla ~50 装备 archetype 归并为 **12 类军备**，与上表"军备半成品"商品对齐：

| V6 整合类 | 涵盖 vanilla archetype（举例） | 由哪种军工建筑产出 | 输入半成品 |
|---|---|---|---|
| Infantry Equipment | infantry_equipment_0/1/2/3 | Arms Industry | Small Arms |
| Support Equipment | support_equipment | Arms Industry | Small Arms + Machinery |
| Artillery | artillery_equipment_0/1/2 | Arms Industry | Artillery Shells + Steel |
| Anti-Tank | anti_tank_equipment | Arms Industry | Artillery Shells + Steel |
| Anti-Air (towed) | anti_air_equipment | Arms Industry | Artillery Shells + Steel |
| Light Armor | light_tank_chassis 全代 | Tank Factory | Tank Hulls + Engines |
| Medium Armor | medium_tank_chassis 全代 | Tank Factory | Tank Hulls + Engines |
| Heavy Armor | heavy_tank_chassis + super_heavy | Tank Factory | Tank Hulls + Engines + Steel |
| Motorized/Mechanized | motorized / mechanized_equipment | Tank Factory | Engines + Steel |
| Fighter Aircraft | fighter_equipment + heavy_fighter | Aircraft Factory | Aircraft Frames + Engines |
| Bomber Aircraft | tac/cma/strat/naval_bomber | Aircraft Factory | Aircraft Frames + Engines × N |
| Ships | ship_hull_* 全部 | Shipyard | Steel + Engines + Machinery |

- World 内仍保留 vanilla 字符串 ID（避免存档大改），但所有"采购 / 生产 / 库存查询"API 统一走 12 类维度
- **装备升级（mk1→mk2）作为 PM 切换体现**，不再各占一个生产线槽
- Munition Plant（合成航弹/炮弹/普通弹药）单独产出 `Artillery Shells` 进市场，供 Arms Industry 与 Tank/Aircraft Factory 拉取

#### 4.2.3 价格形成
- 基础价（base price，单位 RM）在 RON 中定义
- 实时价 = `base × clamp(demand / supply, 0.25, 4.0)`（每周更新，不是每日，避免抖动）
- 计划经济模式下（D4）价格由法律 + RON 固定，市场不影响

### 4.3 全国市场（兑现 D2 的延伸：单池）

```rust
// crates/hoi4-logic/src/market/mod.rs（新文件）
pub struct NationalMarket {
    pub supply:  HashMap<GoodKind, f32>,      // 当日总供
    pub demand:  HashMap<GoodKind, f32>,      // 当日总需
    pub price:   HashMap<GoodKind, f32>,      // 当周实时价
    pub stockpile: HashMap<GoodKind, f32>,    // 全国仓储（≠装备库存）
    pub imports: HashMap<GoodKind, f32>,
    pub exports: HashMap<GoodKind, f32>,
}
```

> 单池：不做 Vic3 的"州/地区市场"。州 infra 体现在"该州建筑供给/需求乘以 (1+0.2·infra)"。
> `stockpile` 是真·商品仓储；和现有装备 `stockpile.rs` 完全解耦。

### 4.4 建筑模型（替换 §3.3）

#### 4.4.1 schema
```rust
// crates/hoi4-state/src/buildings_v6.rs（新文件）
pub struct Building {
    pub kind: BuildingKind,           // 见下表
    pub state: StateId,
    pub level: u8,                    // 0..max_level
    pub active_pm: ProductionMethodId,  // 当前生产方法
    pub employment: [u32; 6],         // 按 PopClass 索引的实际雇佣数
    pub owner: BuildingOwner,         // Private / State / Cartel
    pub requires_law: Option<(LawCategory, LawId)>,  // §9 P9: 当前法律不满足则建筑禁用且不产出
}

pub enum BuildingOwner {
    Private,   // 资本家所有；利润进资本家 POP wage
    State,     // 国营；利润直归 Treasury（计划经济默认）
    Cartel,    // 1936 帝国主义特色：托拉斯（IG Farben / Krupp）；利润 70% 入资本家、30% 入 Treasury 作"自愿贡献"
}
```

#### 4.4.2 建筑分类（最小可行集）
| 大类 | 建筑 | 备注 | 法律门槛（§9 P9） |
|---|---|---|---|
| 资源 | Iron Mine / Coal Mine / Oil Rig / Rubber Plantation / Bauxite Mine / Chromium Mine / Tungsten Mine | PM 切换决定"露天/矿井/合成" | 无 |
| 工业 | Steel Mill / Aluminium Plant / Machinery Workshop / Chemical Plant / Electrical Works | 中间品来源 | 无 |
| 农业 | Grain Farm / Livestock Ranch | 农民阶级吸纳器 | 无 |
| 民生 | Textile Mill / Furniture Factory / Distillery / Tobacco Plantation | 消费品供给 | Economy ≠ 计划经济（计划下转为 State 所有制） |
| 服务 | Bank | 雇佣资本家 / 提供 Banking 商品 | Economy ≠ 计划经济（计划下 Bank 被关闭，金融归国家） |
| 服务 | University | 雇佣职员 / 提供研究效率 | 无 |
| 服务 | Telegraph Office | 雇佣职员 / 提供 Telegraph 商品 | 无 |
| **军工**（D3 核心） | **Arms Industry / Munition Plant / Tank Factory / Aircraft Factory / Shipyard / Synthetic Refinery** | 输入端走市场，输出端直入装备库存 | 无（任何 Economy 法都可造，仅产出效率受法律 modifier 影响） |
| 基础设施 | Railway / Port / Power Plant | infra 等级的真实承载；Port 等级决定本州贸易吞吐 | 无 |
| 军事 | Air Base / Naval Base / Anti-Air / Radar / Bunker / Conscription Center | 兑现 §3.3 中 silent-no-op 的承诺，**真正落地**；Conscription Center 提供 Soldier 招募速度加成 | Radar 需 Tech: 雷达 |

#### 4.4.3 生产方法（PM）
- 每建筑有 4 个 PM 槽（生产方法 / 自动化 / 能源 / 雇佣结构），灵感来自 Vic3
- 切换 PM 立即生效，无延迟
- PM 可由科技解锁（§4.7）
- PM 决定该建筑的 `input_goods[]` / `output_goods[]` / `employment_demand[6]`

#### 4.4.4 建造队列与建造力（CP）

**CP 池**：全国单池（D2），每日 CP 总量 = `Σ (building.kind=Construction_Sector).level × pm_throughput`；Construction Sector 是新建筑类（雇 Worker + Clerk，需消耗 Machinery + Steel），不再用 vanilla 的 civilian_factories 直接转换。

**CP ↔ £ 换算（§9 P8 inline）**：CP 不是货币，但每点 CP 在每日 tick 中触发：
```
gov_buy(Machinery, cp_assigned × 0.05) RM   // 建材
pay_wage(Worker × cp_assigned × 0.10) RM    // 建造工人工资
                                            // 工人通过 Construction Sector 建筑被雇佣
```
两笔都由 Treasury.cash_rm 扣账（详见 §4.5.4 gov_buy 接口）；扣不动则该 CP 当日空转、不推进建造进度。

**建造进度**：
```
daily_progress = assigned_cp × (1 + 0.2 × state_infra) × (1 + idea_bonus) × law_modifier
```
其中 `law_modifier` 来自 Economy 法（战时经济 +30%、计划经济 +50%）。

**队列规则**：
- 单条全国建造队列；玩家可手动调序、拆分 CP
- 单项最大并行 CP = RON 中按 BuildingKind 定义（如 Steel Mill ≤ 50、Radar ≤ 10），避免无脑堆同一建筑
- 取消 V3/V5 那种"按钮 → 点州落位"的隐式 mode；改为面板内显式 enqueue
- 队列中**任意一项**的 `requires_law` 不满足时，该项跳过不消耗 CP，但不出队（玩家可见"被法律阻塞"标记）

### 4.5 财政与债务（兑现 D5 + D8 双货币）

#### 4.5.1 schema
```rust
// crates/hoi4-state/src/finance.rs（新文件）
pub struct Treasury {
    // 国内现金（马克）—— POP 工资、税收、建造、装备生产 全走这层
    pub cash_rm: f64,
    pub daily_income_rm: f64,
    pub daily_expense_rm: f64,

    // 对外储备（英镑等价）—— 贸易、外汇结算、MEFO 阈值、信用评级 全走这层
    pub reserve_gbp: f64,            // 外汇储备（£）
    pub gold_kg: f64,                // 金储备（千克）；按伦敦金价折 £
    pub daily_trade_balance_gbp: f64,  // 当日贸易顺/逆差

    // 债务
    pub public_debt_gbp: f64,        // 正规国债（以 £ 计价，国际市场发行）
    pub public_debt_rm: f64,         // 国内国债（以 RM 计价）
    pub mefo_debt_rm: f64,           // 隐性 MEFO 票据；超阈值爆雷
    pub bond_interest_rate: f32,     // 受信用评级影响
    pub credit_rating: CreditRating, // AAA..D；阈值看 (public_debt_gbp / gdp_gbp)

    // GDP（周更新；§4.10）
    pub gdp_gbp: f64,                // 用 £ 报告，便于国际比较
    pub gdp_rm: f64,                 // 国内 £→RM 换算后值
}

pub struct ExchangeRate {
    pub rm_per_gbp: f32,             // 1 £ = ? RM；动态更新
}
```

#### 4.5.2 汇率模型（§9 新增，D8 兑现）

```
rm_per_gbp_target =
    base_rate(开局=12.5 RM/£，史实)
    × (1 - 0.005 × gold_kg / 1000)      // 金储备每 1000 kg 让 RM 升值 0.5%
    × (1 + 0.01 × debt_pressure)        // public_debt_gbp / gdp_gbp > 0.5 时贬值
    × trade_law_modifier                // 自由贸易 ×1.0；闭关 ×1.3（黑市汇率高）
rm_per_gbp += 0.05 × (target - current) // 每周 EMA 平滑
```

汇率剧变（>10%/月）触发事件 "马克贬值危机"（中文，见 §4.5.5）。

#### 4.5.3 收入（按 Taxation 法档位 + Trade 法）
| 税种 | 计税基 | 进 RM 还是 £ |
|---|---|---|
| `income_tax` | POP wage_rm 总额 | RM |
| `consumption_tax` | POP 消费总额（RM） | RM |
| `corporate_tax` | Building.profit（按 owner 区分；Cartel 的 30% 已计入） | RM |
| `tariff` | imports/exports 流量（£） | £（直接入 reserve_gbp） |
| `bond_issuance_domestic` | 玩家手动发债 | RM（+public_debt_rm） |
| `bond_issuance_foreign` | 玩家手动发债，需 credit_rating ≥ BBB | £（+public_debt_gbp） |
| `mefo_print` | 玩家手动印 MEFO，需法律解锁（见 §4.5.6） | RM（+mefo_debt_rm） |
| `gold_sell` | 玩家手动卖金换 £ | £（−gold_kg, +reserve_gbp） |

#### 4.5.4 支出（gov_buy 接口，§9 P6 inline）

**统一接口**：所有政府采购走 `Treasury::gov_buy()`，禁止任何其它路径直接扣 cash_rm：

```rust
impl Treasury {
    /// 向全国市场下采购单。返回实际采购量（可能因 cash 或 supply 不足而少于请求量）。
    pub fn gov_buy(&mut self, market: &mut NationalMarket, good: GoodKind, qty: f32) -> f32 {
        let unit_price = market.price[good];     // RM/单位
        let available = market.supply[good] - market.demand[good];
        let cap_by_supply = available.max(0.0);
        let cap_by_cash = (self.cash_rm / unit_price as f64) as f32;
        let actual = qty.min(cap_by_supply).min(cap_by_cash);
        self.cash_rm -= (actual * unit_price) as f64;
        market.demand[good] += actual;            // 推高下周价格
        actual
    }
}
```

**支出项**：
| 支出 | 调用 |
|---|---|
| 建造（§4.4.4 CP→£→RM 链路） | `gov_buy(Machinery, ..)` + `pay_wage(Worker, ..)` |
| 军队维护 | 每师每日扣 `division.upkeep_rm` |
| 装备采购 | 军工建筑是 State 所有时利润已归 Treasury；Private/Cartel 时 Treasury 必须 `gov_buy_equipment(eq_id, qty)`（扣 cash + 扣 stockpile） |
| 福利 | Civil Rights 高档每日按 POP 数扣 RM |
| 国债利息 | 公开债按 `interest_rate × debt`；MEFO 不付息（这正是它"隐性"的原因） |
| 研究支出 | §4.7.2 `daily_research_cost_rm`（§9 P11 inline） |
| 外汇购汇 | 玩家手动用 RM 买 £（受 Trade 法限制；闭关时只能国家代办） |

#### 4.5.5 1936 帝国主义特色（D5 + D8）

- **金本位崩溃**：开局已脱离金本位（与史实一致）。金储备不是现金，要变现必须卖（`gold_sell`），按当日伦敦金价折 £
- **外汇管制**：Trade 法 ≥ "外汇统制" 时禁止私人结汇，所有 imports / exports 由 Treasury 代办；Treasury 抽 5%–15% 价差作税
- **战时封锁**：被海上封锁的国家 imports 归零，强制走自给 / 合成燃油 PM；exports 同步归零导致 reserve_gbp 枯竭
- **国债 / 信用评级**：`credit_rating` 按 `(public_debt_gbp + public_debt_rm/rm_per_gbp) / gdp_gbp` 阈值表（< 0.3 = AAA、0.3-0.5 = AA、0.5-0.8 = A、0.8-1.2 = BBB、> 1.2 = BB 起）；评级低于 BBB 时 `bond_issuance_foreign` 被拒绝

#### 4.5.6 MEFO 隐性国债（D5 招牌机制）

**解锁条件**：Conscription 法 ≥ 有限征兵 **且** Economy 法 ≥ 干预经济。
**机制**：玩家每日可手动选择"印 MEFO 补缺口"，最多补当日赤字的 100%；不计入 `public_debt_rm`，但累加 `mefo_debt_rm`。

**爆雷条件**：`mefo_debt_rm / gdp_rm > 0.30` 时触发事件 **「梅福债危机」**（中文）：
- 强制兑付：5 天内 `cash_rm -= mefo_debt_rm × 0.4`（如果 cash 不够则评级骤降到 D，进入主权违约事件链）
- 信用评级骤降两档
- 恶性通胀：rm_per_gbp ×= 1.5（RM 贬值 50%）
- POP 满意度 −0.20 持续 180 天
- `mefo_debt_rm` 被清零，但 `public_debt_rm` += 60% 残债

历史校准目标：纳粹德国 1936-1938 年 MEFO 累计约 120 亿 RM ≈ 当时 GDP 的 17%，1939 年到期被战争"接续"——V6 中如果玩家不发动战争（=不进入"以战养债"循环），1938 末必爆雷。这是 1936 帝国主义特色的强制游戏剧本。

### 4.6 计划经济独立 tick（兑现 D4）

#### 4.6.1 分派

```rust
// crates/hoi4-logic/src/economy/mod.rs（V6 重写）
pub fn tick_daily(world: &mut World, ...) {
    match world.countries.laws[ci].economy.current {
        EconomicSystem::Market | EconomicSystem::Regulated | EconomicSystem::WarEconomy
            => market_tick::run(world, ci, ...),     // 价格清算路径
        EconomicSystem::PlannedEconomy
            => planned_tick::run(world, ci, ...),    // 配额路径
    }
}
```

> 这是两条**完全独立的代码路径**，不是 if/else 散在一处。
> 模块边界：`crates/hoi4-logic/src/economy/{market_tick.rs, planned_tick.rs}`。
> 公共接口由 trait `EconomicSystemTick` 抽离，任何新机制必须同时实现两侧（见 §7 风险表）。

#### 4.6.2 market tick 流程
```
建筑产出 (按 PM) → market.supply
POP 消费 → market.demand
价格周更 = base × clamp(demand/supply, 0.25, 4.0)
Treasury.gov_buy() 推 demand → 价格反馈
POP wage 收税 → Treasury.cash_rm
Building.profit 收税 → Treasury.cash_rm（Cartel 的 30% 额外贡献也在此）
Trade 模块 → market.imports/exports，外汇结算 → Treasury.reserve_gbp
```

#### 4.6.3 planned tick 流程（D4 独立路径）
```
五年计划目标（pyatiletka.ron 中定义）→ 配额下发到 State 所有制建筑
建筑被强制设为指定 PM（玩家失去 PM 切换权）
建筑按配额生产；原料按"计划价"（RON 钉死，不动）从国家仓拉
POP 凭票配给：
    ration_rate[good] = min(1.0, plan_allocation[good] / total_pop_demand[good])
    POP satisfaction 配给制公式（§9 P2 inline）:
        satisfaction_target =
            0.5 × Σ(weight[good] × ration_rate[good])  // 主项：配给率
          + 0.3 × (1 - 失业率)
          + 0.2 × law_modifier (Civil Rights + Info Control)
        // tax_burden 项移除（计划经济下个人无显性税）
Treasury 收入：
    国营企业利润 100% 上缴（无 corporate_tax，但效果等价）
    income_tax 仍正常征收（§9 P10 inline）—— Stakhanov 式工资仍要交税
    consumption_tax 取消（配给制下无市场消费）
    tariff 由 §4.6.5 决定
```

#### 4.6.4 计划经济 ↔ 研究方向（§9 P12 inline）
- 五年计划 RON 中可指定"科技重点方向"（如 Stalin 1933-37 第二个五年计划重工业 + 电气）
- 玩家研究队列**不被强锁**，但：
  - 在指定方向上的研究 +30% 速度
  - 偏离方向的研究 −20% 速度
- 这避免玩家在计划经济下还能"自由选研究"的违和感，但也不剥夺玩家自主权

#### 4.6.5 计划经济外贸垄断（§9 P13 inline）
- 进入 Economy = PlannedEconomy 时，Trade 法**被强制锁定为"国家垄断外贸"档**
  - 玩家在 Trade 法面板看到该档被高亮 + 灰色锁定图标
  - 玩家无法切到"自由贸易/进口替代/闭关"等档；可见但不可选
- 离开计划经济后，Trade 法自动回到上一个状态（保存在 `LawSlot.previous_before_lock`）
- 所有贸易必须由 Treasury 代办，私商通道关闭（市场 imports/exports 字段被 0 化）

#### 4.6.6 切到计划经济的过渡事件
切到 Economy = PlannedEconomy 触发事件 **「国有化运动」**（中文）：
- 全国所有 Private 建筑批量改 owner = State
- Cartel 建筑改 owner = State，但触发"托拉斯反弹"子事件（资本家 POP loyalty −0.5）
- Bank 建筑被永久关闭（雇佣的 Capitalist POP 全部失业 → 流向 Clerk 池）
- 民生建筑（Textile / Furniture / Distillery / Tobacco）也批量国有化
- 一次性政治成本：PP −500，stability −0.20

### 4.7 科技（替换 §3.5）

#### 4.7.1 改变
- 科技数据**全部从 vanilla TXT 迁出**，新建 `content/economy_v6/technologies/*.ron`
- 单条 tech 不再"解锁 equipment archetype"，而是"解锁 PM / 解锁建筑大类 / 解锁法律档位 / 解锁商品（§4.2.1 unlocked_by）"
- 科技分类调整为：**Industry / Chemistry / Electrical / Metallurgy / Military Doctrine / Aviation / Naval / Social Science（解锁高档 Civil Rights 法）/ Information Control（解锁宣传/监控法）**
- 9 类各自独立队列，玩家可设的研究槽位 = `laws.civil_rights` 决定（2-5）

#### 4.7.2 研究速度与成本（§9 P11 inline）

```
daily_speed = BASE
    × (1 + idea_bonus)
    × ahead_of_time_penalty                    // 0.5^年
    × research_efficiency_from_clerks          // Σ(Clerk_at_University) / 10000
    × research_efficiency_from_funding         // 见下
    × planned_direction_modifier               // §4.6.4（仅计划经济）

daily_research_cost_rm = slot_base_cost × (1 + tech.difficulty)
research_efficiency_from_funding =
    1.0                          if Treasury.cash_rm >= cost × 30天
    0.5                          if cash 仅够 7-30 天
    0.0                          if cash 不够 7 天（研究停滞）

Clerk 占用：每队列需常驻 100 名 Clerk 在 University 建筑就业；不够则该队列 speed = 0
```

**资本积累带来人才储备**：Clerk POP 多 → 研究效率乘子高，反映"工业化国家 vs 农业国"的科研代差。
**研究停滞机制**：财政破产时研究自动归零，而非"无穷免费"——这是 V6 财政与科技的紧耦合点。

#### 4.7.3 V6 不做跨国科技转让（§9 P14 inline）
- 1936-1940 题材下技术封锁 / 技术援助是真实存在（美对 GER 渐进禁运、苏联从德获机床）
- V6 **明文不做** tech 转让或封锁机制；技术差距通过 `ahead_of_time_penalty` 自然形成
- 留给 V7+ 评估

### 4.8 法律（兑现 D7：6 大类）

```rust
// crates/hoi4-logic/src/laws/mod.rs（新文件）
pub enum LawCategory {
    Conscription,        // 志愿兵 → 有限征兵 → 广泛征兵 → 总动员
    Economy,             // 自由放任 → 干预经济 → 战时经济 → 计划经济（D4 切换点）
    Trade,               // 自由贸易 → 出口集中 → 进口替代 → 闭关自守
    Taxation,            // 低 → 中 → 高 → 战争税
    CivilRights,         // 开放社会 → 有限权利 → 国家安全法 → 警察国家
    InformationControl,  // 自由新闻 → 监管新闻 → 国营媒体 → 全面宣传
}

pub struct LawSlot {
    pub category: LawCategory,
    pub current: LawId,
    pub cooldown_days: u16,   // 切换后冷却，期间不可再切
    pub pending: Option<(LawId, u16)>,  // 切换中：目标 / 倒计时
}

pub struct LawSet([LawSlot; 6]);   // 6 类各一槽
```

#### 4.8.1 切换流程（D10 PP 阶梯 inline）
1. 玩家在法律面板点击"目标档"
2. 消耗 PP，按目标档难度阶梯：
   ```
   pp_cost[档位] in laws/*.ron:
     志愿兵 = 50;  有限征兵 = 100;  广泛征兵 = 200;  总动员 = 400
     自由放任 = 50;  干预经济 = 150;  战时经济 = 300;  计划经济 = 600
     ...（每法每档显式配；越激进越贵）
   ```
3. 进入 `pending`，倒计时（30-180 天，按法律配置）
4. 倒计时归零时：旧 modifier 卸下、新 modifier 装上、`current = pending.0`、`cooldown` 启动（60-120 天）

#### 4.8.2 与 `Country.ideas` 的关系
- 旧路径：法律塞 `ideas: Vec<String>` — 删除
- 新路径：`ideas` 仅保留"事件/国策授予的真 idea"；法律 modifier 由 `LawSet` 单独贡献
- `crates/hoi4-content/src/eval.rs:189` 的 `AddConscription` stub — 改成对 `LawSet[Conscription]` 的真实 setter

#### 4.8.3 法律对其它子系统的 modifier 索引
| 法 | 影响 | 详见 |
|---|---|---|
| Conscription | Soldier 上限 + 新征转化率 | §4.1.4 / §4.1.5 |
| Economy | wage 分配 + 建筑可用性 + 切到 Planned 触发独立 tick | §4.1.5 / §4.4.2 / §4.6 |
| Trade | imports/exports 开关 + 关税率 + Planned 下被强锁 | §4.9 / §4.6.5 |
| Taxation | 税率上限 + 通过 tax_burden 影响 POP 满意度 | §4.5.3 / §4.1.3 |
| Civil Rights | POP 满意度 + loyalty 公式系数 + 研究槽位数 | §4.1.5 / §4.7.1 |
| Information Control | loyalty 滑落速度 + 满意度 | §4.1.5 |

### 4.9 贸易（替换 §3.1 的字段摆设）

#### 4.9.1 撮合
- 全国市场 supply / demand 不平衡时，**贸易子系统**（V6 新建 `crates/hoi4-logic/src/trade/mod.rs`）按"价格 + 距离 + 政治关系"打分撮合贸易意图
- 玩家可手动签贸易协定（接 V5 外交模块）；AI 自动撮合用于无协定时
- 贸易路线由海上 / 陆上 / 中立国过境 三类承载，每条有吞吐上限（受 Port 等级 / Railway 等级影响）

#### 4.9.2 结算（D8 双货币锁定）
- 所有进出口**必须以 £ 结算**（1936 史实：英镑仍是国际储备主货币）
- 国内 RM ↔ £ 换汇走 §4.5.2 汇率
- Trade 法决定换汇是否需要 Treasury 代办（自由贸易：私商自换；外汇统制：必须 Treasury）
- imports 入 market.supply，对方 reserve_gbp += 货款；本方 reserve_gbp −= 货款
- exports 反之

#### 4.9.3 战时封锁 → POP 满意度（§9 P4 inline）

封锁链路：
```
本方港口被敌方海军覆盖
  → 该港口所在州 trade_throughput = 0
  → 所有走该港口的贸易路线 imports[good] -= flow
  → market.supply[good] 当周骤降
  → market.price[good] × clamp(demand/supply, .., 4.0) 触顶
  → POP 消费实现率 = supply / demand 暴跌
  → §4.1.3 第 4 步 satisfaction_target × (消费实现率 0.4) → satisfaction 跌
  → §4.1.3 第 5 步 loyalty -= ...
  → 久之触发"封锁危机"事件（中文）：stability −0.10、war_support −0.05
```

橡胶 / 石油是 GER 1936 的命门商品；被英国海上封锁后必须切到合成 PM（消耗更多 Coal + Chemicals），代价是 Steel 产出减少（Coal 被合成炼油抢走）——这是"自给自足代价"的真实展示。

#### 4.9.4 计划经济下的贸易
见 §4.6.5：Trade 法被强锁、所有贸易由 Treasury 代办、私商通道关闭。

### 4.10 GDP

- `gdp = Σ (building.output_value)` 周更新
- GDP 不是玩法资源，而是**计分 + AI 决策输入 + 信用评级输入 + MEFO 阈值依据**


## 5. 数据层（兑现 D6）

### 5.1 目录
```
crates/hoi4-content/content/economy_v6/
  goods.ron                 # 商品定义；每条含 unlocked_by: Option<TechId> (P7)
                            # 价格单位 RM
  buildings/
    arms_industry.ron       # 含 requires_law: Option<(LawCategory, LawId)> (P9)
                            # 含 owner_default: BuildingOwner
    steel_mill.ron
    ...
  production_methods/
    arms_industry_pms.ron   # 每 PM 含 input_goods[] / output_goods[] / employment_demand[6]
                            # 含 unlocked_by: Option<TechId>
    ...
  laws/
    conscription.ron        # 每档含 pp_cost (D10 阶梯) / soldier_ratio / cooldown_days
                            # 每档含 pop_modifiers: { satisfaction, loyalty_coefficient, ... }
    economy.ron             # PlannedEconomy 档含 forces_trade_law: Option<LawId> (P13)
    trade.ron
    taxation.ron            # 每档含 income_tax_rate / consumption_tax_rate / corporate_tax_rate
    civil_rights.ron        # 每档含 research_slots / pop_modifiers
    information_control.ron
  technologies/
    industry.ron            # 每 tech 含 difficulty / unlocks: [PM/Building/LawTier/Good]
    chemistry.ron
    ...
  pops/
    initial_ger.ron         # GER 1936 初始 POP 分布（按州 × 阶级）
  finance/
    tax_brackets.ron        # Taxation 法档位下各税率上限
    bond_market.ron         # 信用评级阈值、利率公式
    mefo.ron                # MEFO 票据规则：解锁条件 + 爆雷阈值 + 「梅福债危机」事件
    exchange_rate.ron       # 汇率公式参数（base_rate, gold_modifier, debt_modifier, trade_law_modifier）
  pyatiletka/
    sov_first_plan.ron      # 苏联第一个五年计划目标（D4 / §4.6.3）
    sov_second_plan.ron
  events_v6/
    mefo_crisis.ron         # 「梅福债危机」中文 (D9)
    nationalization.ron     # 「国有化运动」中文 (D9)
    mark_devaluation.ron    # 「马克贬值危机」中文 (D9)
    blockade_crisis.ron     # 「封锁危机」中文 (D9)
```

### 5.2 loader
- 新建 `crates/hoi4-content/src/v6_loader.rs`
- 与 `hoi4-data/src/loader.rs`（vanilla TXT loader）**完全隔离**
- 启动时 vanilla loader 仍跑（地图 / division template / 国策树），V6 loader 跑经济相关
- 在 `Default` 资源种类 / 建筑种类 / 法律 / 科技 上发生冲突时，**V6 数据覆盖 vanilla**
- 启动时 assert 关键 V6 ID（如 `LawCategory::Economy::PlannedEconomy`、`BuildingKind::ArmsIndustry`）不与 vanilla ID 重复，防止隐性串味

### 5.3 RON 规模预算
| 类别 | 行数 | 备注 |
|---|---:|---|
| goods | 250 | 25 商品 × 10 行（含 unlocked_by） |
| buildings | 800 | 25 建筑 × 32 行（含 requires_law / owner_default） |
| production_methods | 1500 | 25 建筑 × 平均 3 PM × 20 行 |
| laws | 800 | 6 类 × 平均 5 档 × 27 行（含 pp_cost / pop_modifiers / forces_trade_law） |
| technologies | 1000 | 100 tech × 10 行 |
| pops initial | 350 | 70 州 × 平均 5 行 |
| finance | 400 | 含 exchange_rate.ron |
| pyatiletka | 200 | SOV 五年计划 |
| events_v6 | 600 | 4 个核心事件链中文 |
| **合计** | **~5900 行 RON** | 单人手写 10-14 天 |


## 6. 阶段表

V6 严格遵循 V5 §5"每 4-6 周一个能玩里程碑"节奏。每阶段必须有"在跑动的游戏里点 5 分钟"验收 + **耦合不变式测试**（§9.6 中列出的硬约束的自动化检查）。

### 阶段 V6.A — 数据层 + Schema 骨架 + P5 manpower 重构 + 法律面板（5 周）✅ 已完成

> 2026-05-21 完成。新增法律面板 UI（`crates/hoi4-ui/src/law_panel.rs`，快捷键 P），6 大类法律切换 + 冷却显示 + PP 消耗，连接 `hoi4_content::set_law()`。验收项全部通过（I-1/I-2/I-3 invariant 测试 + 老路径并行运行 + 法律面板可操作）。

**目标**：所有新 schema 字段全部入 World，没有 tick 逻辑也能 compile + 启动 + 老路径继续工作。**P5（Soldier↔manpower 统一）必须在本阶段一次性完成，不可推迟。**

- [x] A.1 写完 `economy_v6/goods.ron` + `buildings/*.ron`（不含 PM）+ `laws/*.ron`
- [x] A.2 写完 `crates/hoi4-content/src/v6_loader.rs`，启动时加载到 World
- [x] A.3 扩展 `StateStore` / `CountryStore` / `EconomyState`：新增 `buildings_v6: Vec<Building>`、`pops: Vec<PopGroup>`、`market: NationalMarket`、`treasury: Treasury`、`law_set: LawSet`
- [x] A.4 删除 `politics.rs` 的"通过 ideas 塞法律"路径；改 `AddConscription` effect 为 `LawSet` setter
- [x] **A.5（P5 关键）**：删除 `CountryStore.manpower: Vec<u64>` 字段；改为 `fn manpower(&self) -> u64` 派生自 `PopGroup` 中 `class=Soldier && employed_at=None` 的 size之和。修改所有调用点（造师、训练、战斗补员、退役）
- [x] A.6 旧 `economy/tick_daily` 暂时**并行**跑（既动旧 `civ_factories_cached` 又动新 `buildings_v6`），保证存档不挂
- [x] A.7 新增法律面板 UI（`crates/hoi4-ui/src/law_panel.rs`），快捷键 P，6 大类法律切换 + 冷却显示 + PP 消耗，连接 `set_law()` API

**验收**：
- ✅ 游戏跑得起来；老 production / construction UI 还能用；法律面板出现，能切换冷却显示但 modifier 还没接通
- ✅ **Invariant 测试 I-1**：grep 确认 `pub manpower: Vec<u64>` 字段已从 `CountryStore` 删除
- ✅ **Invariant 测试 I-2**：启动后 `country.manpower()` 与 vanilla 1936 GER 数值（约 2300 万兵役适龄）误差 < 5%
- ✅ **Invariant 测试 I-3**：v6_loader 启动 assert 关键 V6 ID（`BuildingKind::ArmsIndustry` 等）与 vanilla ID 不冲突

### 阶段 V6.B — 市场 tick + POP 消费（5 周） ✅ 已完成

> 2026-05-21 完成。新增 market_tick + POP 生命周期 + 市场视图 UI + 建筑施工 UI + I-4/I-5/I-6 不变式测试。验收项全部通过。

**目标**：D1 + D2 + 商品市场可玩，POP 满意度真实反映消费缺口。

- [x] B.1 实现 `market_tick`：建筑产出 → market.supply → POP demand → price 公式
- [x] B.2 实现 POP 生命周期（就业、工资、消费、满意度、loyalty），含 §4.1.6 阶级流动
- [x] B.3 新建造 UI：单一全国队列、显示各建筑工人缺口、PM 切换按钮、建筑被法律阻塞时高亮（§4.4.4）
- [x] B.4 市场视图 UI：25 商品 × 价格 × supply / demand 曲线
- [x] B.5 老 `economy/production.rs` 中民用部分**完全切到** market_tick；军工部分（D3）保留装备库存输出，但原料端切到 market
- [x] B.6 实现 `EconomicSystemTick` trait + market_tick 一侧；planned_tick 占位 `unimplemented!()` 留给 V6.D

**验收**：
- ✅ 开局 30 天内能观察到"钢价 / 失业率 / 工人满意度"互动；切 Economy 法挡位能看到 modifier 真实生效
- ✅ **Invariant 测试 I-4（D3 边界关键）**：stress test 反复造装备，`market.supply[Steel]` 不可因装备产出回升；任何 `stockpile → market` 回流路径必须触发测试失败
- ✅ **Invariant 测试 I-5**：跑 60 天后 `Σ(building.employment[c])` == `Σ(PopGroup.size where class=c && employed_at != None)`，对 Worker/Clerk/Capitalist 都成立
- ✅ **Invariant 测试 I-6**：商品 `unlocked_by` 未满足时，market 中 supply/demand 始终为 0

### 阶段 V6.C — 财政 + 法律全接通 + 双货币（5 周）✅ 已完成

> 2026-05-21 完成。新增 Treasury 双货币 tick + gov_buy() 接口 + 6 类法律 modifier 全接通 + 债务/信用评级/MEFO + 汇率 + 财政 UI 面板 + 研究成本扣款 + I-7/I-8/I-9 不变式测试。验收项全部通过。

**目标**：D5 + D7 + D8 完整可玩；MEFO 票据上线；汇率运转。

- [x] C.1 实现 Treasury 双货币 schema（`cash_rm` / `reserve_gbp` / `gold_kg`），含 §4.5.2 汇率
- [x] C.2 实现 §4.5.4 `Treasury::gov_buy()` 接口，迁移所有政府采购走该接口
- [x] C.3 接通 6 类法律的 modifier 链路（每法 RON 写完即生效）；接通 §4.8.3 modifier 索引表所有项
- [x] C.4 实现国债 / 信用评级 / 利率 / `bond_issuance_domestic` / `bond_issuance_foreign`
- [x] C.5 实现 MEFO 隐性国债 + 「梅福债危机」中文事件链
- [x] C.6 实现「马克贬值危机」中文事件
- [x] C.7 财政 UI 面板：现金 RM / 储备 £ / 汇率 / 收入 / 支出 / 债务 / MEFO / 信用评级
- [x] C.8 §4.7.2 研究成本 + Clerk 占用上线

**验收**：
- ✅ 连续 6 个月不平衡预算会爆雷；切 Taxation 档能看到现金流变化；MEFO 在战争前夕能撑住扩军，但 1938 末必爆雷
- ✅ **Invariant 测试 I-7（P6 关键）**：grep 全代码库，除 `Treasury::gov_buy()` 内部和测试代码外，禁止任何代码出现 `Treasury.cash_rm -=` 直接扣账
- ✅ **Invariant 测试 I-8（P11）**：财政归零后，所有研究队列 daily_speed == 0
- ✅ **Invariant 测试 I-9**：进入"外汇统制"档后，所有 imports/exports 必须走 Treasury 代办

### 阶段 V6.D — 计划经济独立 tick（4 周） ✅ 已完成

> 2026-05-21 完成。新增 planned_tick 完整实现（配额下发 + 配给制 satisfaction 公式）+ 国有化事件执行器 + Trade 法强锁/解锁 + 五年计划 RON + SOV 计划经济参数 + 五年计划 UI 面板 + I-10/I-11/I-12 不变式测试。验收项全部通过。

**目标**：D4 兑现。

- [x] D.1 实现 `planned_tick`：五年计划目标 → 配额下发 → 配给制；含 §4.6.3 POP 配给公式
- [x] D.2 计划法切换时执行**「国有化运动」**中文事件（资本家 POP loyalty −0.5、PP −500、stability −0.20）
- [x] D.3 §4.6.4 计划经济 ↔ 研究方向加成（不强锁，仅 ±% 调速）
- [x] D.4 §4.6.5 Trade 法被强锁实现（`LawSlot.previous_before_lock` 字段 + UI 灰锁图标）
- [x] D.5 计划经济专属 UI：五年计划面板（目标 vs 实际 vs 配给率）
- [x] D.6 为非 GER 国家中的 SOV 提供完整计划经济参数（虽然 V5 仅 GER 可玩，SOV 作为 AI 需要正确建模）

**验收**：
- ✅ 用 debug 命令切 GER 到计划经济，能在面板里看到完全不同的 UI 和 tick 行为；SOV AI 在 1936-1940 工业指标符合史实数量级
- ✅ **Invariant 测试 I-10**：切到计划经济后 `Trade` 法 `is_locked == true`
- ✅ **Invariant 测试 I-11（D4 硬约束）**：grep `market_tick.rs` 与 `planned_tick.rs`，两文件不可 import 对方的内部函数；公共逻辑必须经 trait `EconomicSystemTick`
- ✅ **Invariant 测试 I-12**：切回市场经济后，Trade 法自动恢复 `previous_before_lock`

### 阶段 V6.E — 贸易 + 1936 帝国主义特色（4 周） ✅ 已完成

> 2026-05-21 完成。新增贸易撮合 + 双货币 £ 结算 + 海上封锁判定 + 封锁→POP 满意度链路 + 金本位/外汇管制/关税 + GER 历史经济事件 + 贸易面板 UI + I-13/I-14 不变式测试。验收项全部通过。

**目标**：§4.9 + §0.3 兑现。

- [x] E.1 实现贸易撮合（§4.9.1）+ 路线（陆/海/过境）
- [x] E.2 实现 §4.9.2 双货币结算：所有 imports/exports 走 £；汇率联动
- [x] E.3 实现海上封锁判定（接 V5 §5 海战节）+ 「封锁危机」中文事件
- [x] E.4 实现金本位 / 外汇管制 / 关税
- [x] E.5 实现 §4.9.3 封锁→POP 满意度链路（含合成 PM 切换副作用：Coal 被抢走 → Steel 下降）
- [x] E.6 GER 1936 历史事件：莱因兰 / 4 年计划 / MEFO / Anschluss 经济冲击 接入 `events_v6/`

**验收**：
- ✅ 和 SOV 签 Molotov-Ribbentrop 后原料 imports 真实跳升；被英国海上封锁后橡胶 supply 归零，强制合成 PM；30 天内 POP 满意度可见下跌
- ✅ **Invariant 测试 I-13**：所有贸易流量必须以 £ 结算（不可绕过汇率直接增减 cash_rm）
- ✅ **Invariant 测试 I-14**：封锁状态下被覆盖的 Port 所在州 `trade_throughput == 0`

### 阶段 V6.F — 科技重构 + 收尾（3 周） ✅ 已完成

> 2026-05-22 完成。9 类科技树 RON 写完 + 科技解锁 PM/法律/建筑/商品接通 + 旧 research.rs 切到 V6 路径 + 旧 economy 代码删除 + I-15/I-16 不变式测试通过。验收项全部通过。

- [x] F.1 9 类科技树 RON 写完
- [x] F.2 科技解锁 PM / 法律档位 / 建筑大类 / 商品（§4.2.1 unlocked_by）
- [x] F.3 旧 `research.rs` 完全切到 V6 路径
- [x] F.4 删除 §3 中所有 silent no-op 代码 + 旧 `economy/{production, stockpile, construction, resources}.rs` 中已被取代的部分；删除 V6.A 时保留的双轨 `tick_daily` 旧路径
- [x] F.5 文档同步 V5；本文档归档进 `docs/legacy/`

**验收**：
- ✅ 旧路径无残留；存档可读；V5 §5 中经济节状态从"V5 旧式"翻新为"V6 已完成"
- ✅ **Invariant 测试 I-15**：grep 全代码库无 `civ_factories_cached` / `mil_factories_cached` / `dockyards_cached` 字段引用
- ✅ **Invariant 测试 I-16**：V6.A I-1/I-2/I-7/I-8/I-11 等所有不变式测试在 F.5 仍然通过（防回归）

### 6.x 时间汇总
| 阶段 | 周 | 备注 |
|---|---:|---|
| V6.A | 5 | 原 4 周 + 1 周用于 P5 manpower 重构 |
| V6.B | 5 | |
| V6.C | 5 | 原 4 周 + 1 周用于 D8 双货币 |
| V6.D | 4 | |
| V6.E | 4 | 原 3 周 + 1 周用于双货币贸易结算 |
| V6.F | 3 | |
| **合计** | **26 周** | 原 23 + 3 周为"紧合"代价 |


## 7. 风险与缓解

| 风险 | 严重度 | 缓解 |
|---|---|---|
| POP tick 性能 < 1ms 做不到 | 中 | A.3 落地时立刻 bench；不达标改为周 tick |
| 市场清算抖动（价格震荡） | 中 | 价格周更而非日更；clamp 区间；EMA 平滑 |
| 装备库存 ↔ 市场两轨之间漏洞（玩家发现刷资源 exploit） | 高 | B.5 阶段写专项 invariant 测试，确保军工建筑的原料采购只从 market 出钱 |
| MEFO 数值平衡（早爆 / 晚爆） | 中 | C.4 留出参数化阈值；玩家 playtest 后调 RON |
| 计划经济 tick 与市场 tick 代码两套，维护成本翻倍 | 高 | D.1 写设计时强制抽离公共 trait（`EconomicSystemTick`）；任何新机制必须同时实现两侧 |
| 旧存档不兼容 | 低 | 沿用 V5 §0.2"不与 vanilla `.hoi4` 兼容"原则，V6 内部存档加版本号即可 |
| vanilla loader 的科技/建筑数据残留导致冲突 | 中 | F.4 阶段强制删除；A.2 阶段 v6_loader 启动时 assert 关键 ID 不与 vanilla 重复 |


## 8. 治理

### 8.1 决策表
- §2 的 7 条决策只能由用户改；本文档其它内容由设计阶段讨论改

### 8.2 与现有路线图同步
- 本路线锁定后：
  - 在 `ROADMAP_V5.md` §5 经济相关节加 "→ V6"
  - 更新 `MEMORY.md` / `project_v6_economy.md` 把状态从 "design doc to draft" 升级为 "design doc signed off; impl in flight"
- 本路线全部完成后：
  - 移动到 `docs/legacy/`
  - 把已交付清单合并回 `ROADMAP_V5.md`

### 8.3 失败 / 暂停条件
- 任一阶段验收"在跑动的游戏里点 5 分钟"不通过 → 阶段不算完
- 连续 2 个阶段 slip 超过 50% → 重审 V6 范围（可能砍 D4 计划经济独立 tick 改回开关式）

### 8.4 严禁
- ❌ 跳过 V6.A 直接动 tick 逻辑
- ❌ 在 V6.E 之前把 `imported / exported` 字段当作"已经实现"用
- ❌ 在 V6.D 完成前对 SOV 用市场 tick 然后"看着差不多"




## 9. 耦合不变式与中央数据流

> 本节是 V6 紧合的"硬约束"——所有原 §9 补丁（P1-P14）已 inline 进 §4 / §5；本节仅保留**断言**和**数据流图**，不再列待办。
>
> 9 个子系统：POP（§4.1）/ Goods+Market（§4.2-4.3）/ Buildings（§4.4）/ Treasury+Finance（§4.5）/ Planned-tick（§4.6）/ Tech（§4.7）/ Laws（§4.8）/ Trade（§4.9）/ Stockpile（D3 边界）。

### 9.1 已建立的耦合接口（紧合后）

| 配对 | 接口 / 写明位置 |
|---|---|
| POP × Goods | POP.consume → market.demand；价格 → POP.satisfaction (§4.1.3 步 3-4) |
| POP × Buildings | Building.employment[6] 从本州 PopGroup 池吸人；wage_rm 由 Building 付 (§4.4.1) |
| POP × Treasury | tax_burden 项进 satisfaction 公式 (§4.1.3)；income_tax/consumption_tax 流向 (§4.5.3) |
| POP × Planned | 配给制 satisfaction 公式 (§4.6.3) — 与市场公式独立 |
| POP × Tech | Clerk_at_University → research_efficiency 乘子 (§4.7.2) |
| POP × Laws | 全 6 类法律对 POP modifier 表 (§4.1.5) + §4.8.3 索引 |
| POP × Trade | 封锁→supply 短缺→消费实现率→satisfaction 链路 (§4.9.3) |
| POP × Stockpile | **Soldier ↔ manpower 统一** (§4.1.4)；造师/退役/损失全走 PopGroup |
| Goods × Buildings | Building 按 PM input_goods[] / output_goods[] 交互 (§4.4.3) |
| Goods × Treasury | gov_buy() 统一接口 (§4.5.4) — 唯一允许扣 cash_rm 的路径 |
| Goods × Planned | 价格 RON 钉死；原料按计划价从国家仓拉 (§4.6.3) |
| Goods × Tech | goods.ron `unlocked_by: Option<TechId>` (§4.2.1) — 未解锁不进 market |
| Goods × Laws | Trade 法决定关税/外汇/私商 (§4.9.2)；Economy 法决定建筑可用性 (§4.4.2) |
| Goods × Trade | imports/exports 进 market.imports/exports，£ 结算 (§4.9.2) |
| Goods × Stockpile | D3 硬边界：半成品走 market；成品入 stockpile；**禁止回流** |
| Buildings × Treasury | CP↔£↔RM 换算 + gov_buy 接口 (§4.4.4) |
| Buildings × Planned | planned_tick 强制配额下发；建筑批量 owner=State (§4.6.6) |
| Buildings × Tech | tech 解锁建筑大类 / PM (§4.7.1) |
| Buildings × Laws | `requires_law: Option<(LawCategory, LawId)>` (§4.4.1)；不满足时阻塞 (§4.4.4) |
| Buildings × Trade | Port 等级决定本州 trade_throughput (§4.9.1) |
| Buildings × Stockpile | 军工建筑产出 → stockpile（D3） |
| Treasury × Planned | 国营企业 100% 上缴；income_tax 仍正常；consumption_tax 取消 (§4.6.3 / §4.5.3) |
| Treasury × Tech | 研究每日扣 cash_rm + Clerk 占用 (§4.7.2) |
| Treasury × Laws | Taxation 法驱动税率；MEFO 解锁依赖 Conscription/Economy 法档 (§4.5.6) |
| Treasury × Trade | 关税 + 外汇结算 + 封锁损失 全经 Treasury (§4.5.5) |
| Treasury × Stockpile | 装备生产成本在 Building 端付；Stockpile 本身不涉钱 |
| Planned × Tech | 五年计划方向加成 ±%，非强锁 (§4.6.4) |
| Planned × Laws | Economy=PlannedEconomy 是 planned_tick 触发条件；切换触发「国有化运动」 (§4.6.6) |
| Planned × Trade | Trade 法被强锁；私商通道关闭 (§4.6.5) |
| Planned × Stockpile | 军工建筑在 planned 下仍产装备，原料按计划价 |
| Tech × Laws | tech 解锁高档 law（Social Science / Info Control 类） (§4.7.1) |
| Tech × Trade | V6 不做跨国 tech 转让 / 封锁 (§4.7.3) |
| Tech × Stockpile | tech 改 PM，PM 改装备产出 (§4.7.1) |
| Laws × Trade | Trade 法直接决定贸易开关 + 关税 + 结算方式 (§4.8.3) |
| Laws × Stockpile | Conscription 法决定 Soldier 上限 → 决定能拉多少现役师 (§4.1.5) |
| Trade × Stockpile | imports 可含整装备（美援等），写入 stockpile (§6 V6.E) |

**所有 36 对耦合状态：✅**（原 ⚠️ / ❌ 全部消除）。

### 9.2 中央数据流（单向，禁旁路）

```
[Laws.LawSet] ──→ [Tech 可用 PM / 可解锁 Good 集] ──→ [match Economy 法分派]
                                                            │
                            ┌───────────────────────────────┴───────────────────────────────┐
                            ↓                                                                ↓
                  market_tick.rs                                                    planned_tick.rs
                            │                                                                │
                  Buildings (按 PM) 拉原料                                  Pyatiletka.ron 配额下发到 Buildings
                       ↓ market.supply ↑                                        ↓ 国家仓 (RON 钉死价) ↑
                       ↓               ↑                                        ↓               ↑
                  POP.consume → market.demand                              POP 按 ration_rate 凭票配给
                       ↑                                                        ↑
                  POP 满意度 = f(消费实现率, 失业率, tax_burden, law)        POP 满意度 = f(ration_rate, 失业率, law)
                       ↑                                                        ↑
                  POP.wage_rm 由 Building 付                                Building.profit 100% 上缴 Treasury
                       │                                                        │
                  Treasury 收入: income/consumption/corporate tax           Treasury 收入: 利润 + income_tax
                       │                                                        │
                  Treasury 支出: gov_buy(Machinery,..) / 军维/福利/利息       Treasury 支出: 同左 + 强制建造配额
                       │                                                        │
                  Trade.撮合 → market.imports/exports                       Trade 国家垄断 → market.imports
                  £ 结算 → reserve_gbp                                       £ 结算 → reserve_gbp (国家代办)
                            ↓                                                                ↓
                            └───────────────→ [Stockpile (sink)] ←───────────────────────────┘
                                              仅军工 Building 写入 (D3)
                                              战指模块 (V5) 消耗
                                              [Soldier PopGroup] ←→ 现役师人力 (§4.1.4)
```

### 9.3 单向数据流原则
- `Laws` 是源头，**绝不**读其它子系统的状态（只写 modifier）
- `Tech` 只读 `Laws`，写 `PM 可选集 / Good 可解锁集`
- `Buildings / POP / Trade / Treasury` 是 tick 内互相读写的"主循环"
- `Stockpile` 是 sink — 主循环只**写入**（D3），战指模块只**消耗**
- **禁止 `Stockpile → POP` 或 `Stockpile → Market` 的回流**（一旦回流，D3 边界破，刷资源 exploit 上线）

### 9.4 硬约束（grep-able）—— 任何代码评审 / CI 必须检查

| ID | 约束 | 检查方式 |
|---|---|---|
| HC-1 | 无 `pub manpower: Vec<u64>` 字段 | `grep "pub manpower: Vec<u64>" crates/` 应为空 |
| HC-2 | `Treasury.cash_rm -=` 仅出现在 `Treasury::gov_buy()` 或测试代码 | grep + 评审 |
| HC-3 | `market_tick.rs` ↮ `planned_tick.rs` 不可互 import | `grep "use.*planned_tick" market_tick.rs` 应为空，反之亦然 |
| HC-4 | 装备 stockpile 写入仅来自军工 Building | grep `econ.stockpile[ci].insert\|.get_mut` 调用点 |
| HC-5 | 商品 `unlocked_by` 未满足时 supply/demand=0 | 单测 + 不变式 I-6 |
| HC-6 | imports/exports 流量全 £ 结算 | grep `market.imports[good] += ..` 必须伴随 `reserve_gbp -= ..` ✅ V6.E 已实现 |
| HC-7 | 法律 modifier 通过 `LawSet` 贡献，不通过 `Country.ideas` | grep `ideas.push("conscription_` 等应为空 |
| HC-8 | 计划经济下 Trade 法 is_locked=true | 单测 + 不变式 I-10 |
| HC-9 | 研究队列扣 cash 失败时 speed=0 | 单测 + 不变式 I-8 |

### 9.5 维护守则

任何新增的子系统、新增的接口、新增的 RON 字段，**必须**：
1. 在 §9.1 表里新增一行说明耦合接口
2. 在 §9.2 数据流图里画出位置
3. 通过 §9.4 所有 HC 检查
4. 若新增子系统是 8 套主循环之外的"第 10 个"，必须先开 V6 子路线评审，**不允许默默接进 tick**


## 10. 附录

### 10.1 关联文档
- [`ROADMAP_V5.md`](./ROADMAP_V5.md) — 顶层路线
- [`docs/vanilla_assets_used.md`](./docs/vanilla_assets_used.md) — vanilla 资产白名单（V6 不改）
- `HOI4_VANILLA_AI_NOTES.md` — 战争 AI 参考（V6 不改）

### 10.2 决策追溯（已 inline 但保留索引）
| 决策 | 用户答复 (2026-05-21) | 锁定位置 | inline 位置 |
|---|---|---|---|
| Q1 货币 | 方案 A 双货币 RM/£ | D8 / §0.3 | §4.5 全节 / §4.9.2 |
| Q2 事件语言 | 中文 | D9 | §4.5.6 / §4.6.6 / §4.9.3 / §5.1 events_v6/ |
| Q3 PP 消耗 | 阶梯 | D10 | §4.8.1 |
| Q4 阶级流动 | 打开 | D11 | §4.1.6 |
| Q5 装备整合 | 12 类 | D12 | §4.2.2 |
