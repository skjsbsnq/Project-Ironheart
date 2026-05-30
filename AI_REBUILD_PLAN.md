# AI 系统重构方案

## 背景：为什么现在的 AI 不能用

### 当前架构（按文件）

```
crates/hoi4-ai/src/
├── orchestrator.rs    264 行  每日驱动入口、cadence 调度
├── focus.rs           149 行  国策选择
├── research.rs        144 行  科技选择
├── production.rs      227 行  工厂建造 + 生产线
├── recruit.rs         121 行  师团招募（最近加的）
├── diplomacy.rs       208 行  外交决策
├── frontline.rs       185 行  前线探测
├── ground.rs          249 行  陆军 posture 决策
├── ground_orders.rs   583 行  师团调度执行（多次重写）
├── air.rs             206 行  空军任务
├── naval.rs           158 行  海军任务
├── personality.rs     104 行  AI 性格预设
├── strategic_profile.rs 421 行 阵营 / 大国剧本
├── scoring.rs          42 行  打分工具
└── constants.rs        58 行  调参常量
```

### 真实问题（不是细节 bug，是架构问题）

**1. 没有"国家状态机"**

vanilla HOI4 的 AI 是声明式的：每个国家在 `ai_strategy/<TAG>.txt` 里声明
"我现在想干什么"，引擎读取这些声明、综合打分、执行。

我们的 AI 是**命令式的子系统拼盘**：focus 模块做 focus、production 模块
做生产，互相不知道对方在干什么。结果：

- ITA 的工厂建在罗马，但师团生成在埃塞俄比亚（首都省 = 罗马，所以师团也
  生成在罗马，然后要 BFS 走 60 跳到非洲——根本走不到，因为中间是海）
- SPA 的 AI 不知道"我刚分裂出来、需要快速建立产线"，按通用规则慢慢建
- ETH 的 AI 不知道"我是被压制的弱国、应该全国总动员"，按通用规则招兵

**2. 没有"资源-装备-师团"闭环的端到端验证**

每个子系统单独看都"在跑"：
- production 每月造 1 个工厂 ✓
- recruit 每周尝试招 1 个师 ✓
- ground 每 3 天调度师团 ✓

但**这些是否能拼出一个能赢战争的国家**？没人验证过。结果：
- 工厂造在错的地方（基建低的州）
- 招兵在首都省，但首都省离前线 30 跳
- 师团调度走错路（BFS 走到海里）
- 战斗系统打不死人（4v4 简化）
- 占领判定要求 100% 翻 controller（一个师残留就永远不翻）

**3. 性能没设计**

每天每个国家 × 每个师 × BFS 60 跳。N=50 国家 × M=300 师 × 60 跳 BFS =
每天百万次邻接表查找。1937 卡顿是必然的。

vanilla 用的是**预计算地图分区**（strategic_region / area），AI 决策粒度
是"这个 area 给多少师"，不是"这个师走哪个省"。我们一开始就走错了抽象层。

**4. 没有"行动持续性"**

vanilla 的 `front_control execute_order = yes` 表示"持续打"。我们的 AI
每 3 天重评估、每次都可能改主意。师团接到攻击命令、走一半被改成防御、
再走一半被改成撤退。永远到不了战场。

我加的 posture stickiness 是补丁，治标不治本。

**5. 经济决策无目标导向**

production.rs 看到"民工厂少于军工厂的 1.5 倍" → 造民工。但**为什么**这个
比例？没有目标。vanilla 的 production 是"我要 N 个步兵师 → 需要 X 装备 →
需要 Y 工厂 → 现在有 Z → 缺 W → 造 W"。

---

## 重构方案：两层 AI

### 设计原则：通用 AI + per-country 配置覆盖

仿 vanilla HOI4 `default.txt` + `<TAG>.txt` 的两段式：

```
┌─────────────────────────────────────────────────────────┐
│ 通用 AI 引擎 (在 Rust 代码里)                              │
│                                                          │
│ - 知道怎么"建工厂"（找空槽、按需求选类型）                    │
│ - 知道怎么"研究科技"（按未解锁清单、年代、解锁内容打分）        │
│ - 知道怎么"生产装备"（步兵优先、空闲工厂自动追加）              │
│ - 知道怎么"招募师团"（库存 + 人力 + 模板齐了就招）             │
│ - 知道怎么"调度师团"（按前线分配、posture 稳定）              │
│ - 知道怎么"决定 stance"（看战时 / 力量比 / 是否被压制）       │
│                                                          │
│ 这套引擎对所有国家都跑——大国小国都一样。                      │
└─────────────────────────────────────────────────────────┘
                       ↑ 读取
┌─────────────────────────────────────────────────────────┐
│ per-country 配置（数据驱动，TOML 文件）                     │
│                                                          │
│ 例：data/ai_profiles/ITA.toml                             │
│   aggression = 0.8                                       │
│   target_div_count_peace = 30                            │
│   target_div_count_war = 80                              │
│   civ_to_mil_ratio = 1.2  # ITA 备战重武器                │
│   priority_areas = ["italy", "horn_of_africa"]           │
│   force_attack_against = ["ETH"]  # 强制对 ETH 攻势        │
│                                                          │
│ 例：data/ai_profiles/default.toml （通用兜底）             │
│   aggression = 0.5                                       │
│   target_div_count_peace = 24                            │
│   target_div_count_war = 60                              │
│   civ_to_mil_ratio = 1.5                                 │
│                                                          │
│ 加载顺序：default.toml → <TAG>.toml 覆盖                  │
│ 没有 <TAG>.toml 的国家走 default。                         │
└─────────────────────────────────────────────────────────┘
```

**关键不变量**：通用引擎能让**任何国家**（包括没配置的小国）打完一场完整
的战争——建工厂、研究、招兵、调度、获胜。per-country 配置只用于**调味**
（让 ITA 比 ETH 更激进、让 SOV 比 POL 更工业化）——而不是让小国"能打仗"
的前提。

### 通用 AI 必须能做的事（所有国家通用）

| 能力 | 通用规则 | 配置可覆盖 |
|---|---|---|
| 建民工厂 | 当 civ/mil < target_ratio 时建 | ratio 值、年份切换点 |
| 建军工厂 | 战时 + 已达 ratio 时建 | 战时倍率 |
| 研究 | 选已解锁前置 + 年代≤当前的科技、解锁分高 | 类别偏好（陆/海/空/工业） |
| 生产装备 | 60% 步兵 / 20% 火炮 / 20% 支援 | 自定义比例 |
| 招募 | 师数 < target、库存 ≥ 1 师、人力 ≥ 1 师 | target_div_count |
| 防御部署 | 师均匀分配到与敌邻接的本国前线省 | (no) |
| 进攻部署 | 集中到 N 个最弱敌方目标 | breadth |
| stance 决策 | 战时 + 力量比 → 攻/守/退/扫荡 | aggression 修偏 |

**没有 per-country 配置的国家**走 default 配置，所有这些能力都齐全——
不会出现"没配置的小国不会研究科技 / 不会招兵 / 不会调度"这种情况。

### 灵感来自 vanilla HOI4 + 经典 RTS AI

```
┌──────────────────────────────────────────────────────────────┐
│ 战略层 (Strategic Layer) - per country, every 7 days         │
│                                                               │
│ 1. 评估世界状态 → 选择"国家状态" (NationalStance)              │
│    - Peacetime: BuildingUp / Rearming                        │
│    - Wartime:   Defending / Attacking / MoppingUp            │
│                                                               │
│ 2. 根据 stance + 当前资源 → 设置"目标"                          │
│    - 军队规模目标: target_divisions = N                       │
│    - 工业目标: target_civ_factories = X, target_mil = Y       │
│    - 前线分配: front[ETH] = need 12 divs                      │
│                                                               │
│ 3. 把目标拆成给战术层的"指令"                                   │
└──────────────────────────────────────────────────────────────┘
                              ↓ commands
┌──────────────────────────────────────────────────────────────┐
│ 战术层 (Tactical Layer) - per country, every 1 day            │
│                                                               │
│ 1. 工业指令 → 工厂建造 + 生产线分配                              │
│    - "需要 +5 个 mil_factory" → 找空槽 → 建造                  │
│    - "需要步兵装备 / 月" → 分配工厂                             │
│                                                               │
│ 2. 招募指令 → 在前线集结点生成新师                                │
│    - 不在首都生成，在前线最近的可达友方省生成                       │
│                                                               │
│ 3. 师团指令 → posture-stable 调度                              │
│    - 每个师跟着所属"前线"，前线指令变了才换师                       │
│    - 师团有 `assignment` 字段，不是每天重派                        │
└──────────────────────────────────────────────────────────────┘
                              ↓ events
┌──────────────────────────────────────────────────────────────┐
│ 物理层 (Physics Layer) - existing, stays as-is                │
│                                                               │
│ daily_movement_tick / daily_occupation_tick / arbiter         │
└──────────────────────────────────────────────────────────────┘
```

### 核心数据结构（新增）

```rust
/// 一个国家的"战略意图"快照（每 7 天更新）
pub struct NationalIntent {
    pub stance: NationalStance,
    pub target_divisions: u32,
    pub target_civ_factories: u32,
    pub target_mil_factories: u32,
    pub front_assignments: HashMap<CountryId, FrontAssignment>,
    pub last_updated: i64, // game day
}

pub enum NationalStance {
    /// 和平期、扩军备战
    BuildingUp { focus: BuildFocus },
    /// 战时、防守
    Defending { primary_threat: CountryId },
    /// 战时、进攻
    Attacking { primary_target: CountryId },
    /// 战胜中、清残余
    MoppingUp { remaining: Vec<CountryId> },
}

pub enum BuildFocus {
    Industry, Military, Balanced
}

pub struct FrontAssignment {
    pub posture: GroundPosture,
    pub target_div_count: u32,        // 这条前线想要多少师
    pub assigned_divs: Vec<DivisionId>, // 实际分配的师（持续多 tick）
    pub priority_target: Option<ProvinceId>, // 主攻目标省
}
```

### 师团也带"任务标签"（解决"打着打着不打了"）

```rust
// 在 DivisionStore 里加：
pub assignments: Vec<DivisionAssignment>,

pub struct DivisionAssignment {
    pub front: Option<CountryId>,    // 属于对哪个敌国的前线
    pub role: DivisionRole,           // Garrison / Assault / Reserve
    pub target: Option<ProvinceId>,   // 当前任务目标
    pub assigned_at: u64,             // 派遣时间（hours）
}

pub enum DivisionRole {
    /// 守某省（即使敌人不来也守着）
    Garrison(ProvinceId),
    /// 攻击某省（直到拿下）
    Assault(ProvinceId),
    /// 后备（没事干）
    Reserve,
}
```

师团一旦被派 Assault → **持续推进该目标直到拿下或破碎**，不会因为下一次
tactical eval 改主意而改向。这就是 vanilla `execute_order = yes` 的语义。

---

### per-country 配置草案（AiProfile）

```rust
/// per-country AI 配置。所有字段有默认值（=default.toml 的值），
/// 国家专属文件覆盖任意子集。
#[derive(Debug, Clone)]
pub struct AiProfile {
    // ───── 性格（影响 stance / 决策权重）─────
    pub aggression: f32,           // 0-1，越高越倾向 Attack
    pub civilian_focus: f32,       // 0-1，越高越倾向先建工业
    pub doctrine_priority: f32,    // 0-1，越高越早研究学说

    // ───── 工业目标 ─────
    pub target_civ_factories: u32, // 平时目标
    pub target_mil_factories: u32, // 平时目标
    pub civ_to_mil_ratio: f32,     // 切到军工的临界值
    pub mil_focus_year: u16,       // 这年开始优先军工

    // ───── 军队目标 ─────
    pub target_div_count_peace: u32,
    pub target_div_count_war: u32,
    pub division_role_ratio: [f32; 5], // [infantry, armor, motorized, mountaineers, marines]

    // ───── 装备生产 ─────
    pub equipment_ratio: EquipmentRatio,

    // ───── 战术倾向 ─────
    pub priority_targets: Vec<String>, // 强制对这些 tag 的前线优先（如 ITA → ETH）
    pub force_attack_against: Vec<String>, // 与对方开战立即 Attack 不需要力量比
    pub max_war_duration_days: u32, // 长期消耗战 → 触发停战意愿

    // ───── 阵营 ─────
    pub faction_role: FactionRole, // LeadFaction / JoinFaction / Solo / AwaitInvite
    pub faction_target_name: Option<String>,
}

pub struct EquipmentRatio {
    pub infantry: f32,    // 默认 0.6
    pub artillery: f32,   // 默认 0.2
    pub support: f32,     // 默认 0.2
    pub armor: f32,       // 默认 0.0（解锁后才大于 0）
    pub fighter: f32,     // 默认 0.0
}
```

### 例：ITA 配置（让意大利历史化）

```toml
# data/ai_profiles/ITA.toml
aggression = 0.75
civilian_focus = 0.4
doctrine_priority = 0.6

target_civ_factories = 35
target_mil_factories = 60
civ_to_mil_ratio = 1.2  # 比 default(1.5) 更早切军工
mil_focus_year = 1937

target_div_count_peace = 30
target_div_count_war = 80

priority_targets = ["ETH", "GRE", "YUG"]
force_attack_against = ["ETH"]  # 1936 年开战立即攻势

faction_role = "AwaitInvite"
```

### 例：default.toml（兜底，所有没配置的国家用这个）

```toml
# data/ai_profiles/default.toml
aggression = 0.5
civilian_focus = 0.5
doctrine_priority = 0.5

target_civ_factories = 20
target_mil_factories = 30
civ_to_mil_ratio = 1.5
mil_focus_year = 1939

target_div_count_peace = 12
target_div_count_war = 30

priority_targets = []
force_attack_against = []

faction_role = "Solo"
```

**关键性质**：哪怕一个国家完全没配置（比如玻利维亚），它也会按 default
跑通整套循环——建工厂、研究、招兵、防御。不会出现"小国不会打仗"。

---

## 实施计划（按优先级）

### Phase 0: 验证当前问题（半天）
- [x] 加 `[ai_log]` 输出到一个 ringbuffer，每 7 天打印一行 per country：
      `[ETH] divs=5/24 mil=2 stockpile_inf=1500 mp=80k stance=Defending front[ITA]=2divs`
- [ ] 跑 1936-1937 整年，看 SPA / ETH / SPR 的 stockpile 是否增长、师数是否增加、
      前线分配是否合理
- [ ] **不修代码**，只看数据，找出真实瓶颈

### Phase 1: NationalIntent 战略层 + per-country 配置（3-4 天）

**1a. 通用引擎数据结构**
- [x] 新模块 `crates/hoi4-ai/src/intent.rs`，定义 `NationalIntent` + `NationalStance`
- [x] `evaluate_intent(world, country, profile) -> NationalIntent`：
      根据"是否战时 / 师力对比 / 前线情况 + profile 调味"算出当前 stance
- [x] orchestrator 每 7 天调一次，存到 `CountryAiState.intent`

**1b. per-country 配置层**
- [x] 新模块 `crates/hoi4-ai/src/profile.rs`，定义 `AiProfile`（结构体，
      所有字段有默认值，对应通用引擎的可调参数）
- [x] 数据文件 `data/ai_profiles/default.toml` + 主要国家文件
      （ITA / ETH / SPR / SPA / GER / ENG / FRA / SOV / JAP / USA / POL）
      — 当前用 `ProfileRegistry` + `hardcoded_profiles()` 代替 TOML（不引新 crate）
- [x] 启动时加载到 `HashMap<String, AiProfile>`，spawn_country 时按 tag 查找
- [x] 没有 tag 配置的国家用 default 副本

**1c. 现有子系统改造**
- [x] `production` / `recruit` / `ground` 都改成读 `intent + profile`，不再硬编码
- [x] 删除 `personality.rs`（被 `AiProfile` 取代）
- [x] 删除 `strategic_profile.rs` 里的硬编码（搬到 `profile.rs` 的 `AiProfile`）

### Phase 2: 师团 assignment 持续性（1-2 天）
- [x] `DivisionStore` 加 `assignments: Vec<DivisionAssignment>`
- [x] `ground_orders` 改成"按 front 分配师 → 给师设 Assault/Garrison → 不再每天重派"
- [x] `should_reassign` 改成只看 assignment 是否完成（target 拿下 / 师破碎）
- [x] 师团 spawn 在前线集结点（不是首都）

### Phase 3: 工业目标导向（1 天）
- [x] `production` 读取 `intent.target_civ_factories / target_mil_factories`
- [x] 算 gap = target - current → 优先建缺的
- [x] 生产线按 `target_divisions × template_needs / 6 月` 算月产能 → 反推工厂数

### Phase 4: 性能（1 天）
- [x] 把 `next_step` 的 BFS 缓存到 `World.path_cache: HashMap<(from, dest), next>`
      （TTL = 一天，daily 清空）
- [x] AI 决策从 daily 改成 weekly（除了战术撤退）
- [x] 战斗仲裁器分批：每小时只处理 1/4 的对立省份对（4h 一轮 = 一天全扫一次）

### Phase 5: 删冗余 + 文档（半天）
- [x] 删除 `ground_orders.rs` 的旧分兵逻辑（已被 intent 取代）
- [x] 删除 personality.rs 里没用的字段
- [x] 把 strategic_profile.rs 里的硬编码搬到 data 文件

**总预估**: 7-9 天 / 实际编码工作（多 1 天用于 profile 数据驱动层）

---

## 这次重构会解决的问题

| 当前问题 | 重构后 |
|---|---|
| SPA 不会反攻 | NationalStance::Attacking 强制持续推进 |
| 师团打着打着不打了 | DivisionAssignment 持续到目标拿下/师破碎 |
| 1937 年卡顿 | path_cache + weekly cadence + 战斗分批 |
| AI 不会建工厂 | intent.target_factories 驱动建造 |
| AI 不会招兵 | intent.target_divisions 驱动招募 |
| AI 不会清残余 | NationalStance::MoppingUp |
| 跨大陆调度 | front_assignments 按地理分组、不跨段 |

## 不重构的部分（保留）

- `frontline.rs`（前线探测）：算法是对的
- `ground.rs::evaluate_ground` 里的 ratio / encirclement 算法：保留作为打分依据
- `arbiter` / `daily_movement_tick`：物理层不动
- `focus` / `research`：勉强能用

## 关键约束

1. **不引入新 crate**，全部在 `hoi4-ai` 里
2. **保留 `World` API 不变**，只读不改外部接口
3. **AI 每周决策预算 ≤ 5ms / country**（实测）

---

## 你需要做的决策

1. **Phase 0 先做诊断还是直接进 Phase 1**？我建议先 Phase 0 半天，因为不
   知道真实瓶颈就重构等于继续盲修
2. **要不要保留 SCW 双方分兵 50%**？我说"保留"
3. **战斗模型要不要重做**？现在的 4v4 简化太弱。我建议这次重构**不动战斗**，
   先把调度修对；战斗弱可以靠"AI 集中数倍兵力"绕过
4. **要不要 commit 当前所有改动**再开始重构？建议先 commit，万一重构过程
   想回滚有基线
