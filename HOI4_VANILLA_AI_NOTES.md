# HOI4 Vanilla AI 逆向笔记

参考路径：`Hearts of Iron IV/common/`

## 目录结构

| 目录 | 用途 |
|---|---|
| `ai_strategy/` | 国家级策略声明（per-tag txt + default.txt）。声明式：触发条件 + 一组 ai_strategy 项。 |
| `ai_strategy_plans/` | 高层"计划"（historical / alternate）：focus 顺序、研究优先级、ideas |
| `ai_areas/` | 把战略地区聚成"区域"（horn_of_africa, north_africa, suez …），是 AI 决策的地理粒度 |
| `ai_focuses/` | 国策选择倾向（per major：ENG/FRA/GER/ITA/JAP/SOV/USA/generic） |
| `ai_navy/` | Goal/Objective 海军 AI 文档 + 实现 |
| `ai_equipment/` | 装备生产倾向（per major × 类别：tank/naval/planes） |
| `ai_faction_theaters/` | 阵营级"战区"定义 |
| `ai_templates/` | 师团模板（per major：templates_ITA.txt 等） |

## 核心数据流

```
┌─────────────────┐   每日触发条件评估   ┌──────────────────┐
│ ai_strategy/    │ ─────────────────→ │ 当前活跃的       │
│ ai_strategy_    │                    │ ai_strategy 集合 │
│ plans/          │                    └──────────────────┘
└─────────────────┘                            │
                                               ▼
                                    ┌──────────────────────┐
                                    │ 引擎读取每条          │
                                    │ ai_strategy 的        │
                                    │ type + value，调整     │
                                    │ 内部计分（front_      │
                                    │ control、area_        │
                                    │ priority 等）         │
                                    └──────────────────────┘
                                               │
                                               ▼
                                    ┌──────────────────────┐
                                    │ 师团 / 飞机 / 舰队    │
                                    │ 按计分分配到前线 /    │
                                    │ 战区 / 任务            │
                                    └──────────────────────┘
```

## 关键 ai_strategy 类型

### 陆军前线相关

| type | 用途 |
|---|---|
| `front_control` | 给某一条前线（vs tag X，或某个 strategic_region）设置控制：execution_type（careful / balanced / rush_weak / rush）+ ratio + priority + execute_order(yes/no) + manual_attack(yes/no) |
| `front_unit_request` | 调整某前线的师团需求量（增加 / 减少 N） |
| `front_armor_score` | 某前线对装甲的倾向分 |
| `force_defend_ally_borders` / `dont_defend_ally_borders` | 是否帮盟友守边界 |
| `area_priority` | 某 area（来自 ai_areas）的优先级（-200..+200） |
| `theatre_distribution_demand_increase` | 提高某 strategic_region 的师团需求 |
| `put_unit_buffers` | 是否在某区域后置 buffer 部队 |

### 进攻 / 入侵

| type | 用途 |
|---|---|
| `invade` | 计划登陆某 tag |
| `invasion_unit_request` | 登陆部队规模 |
| `prepare_for_war` | 表示备战，影响生产计划 |
| `declare_war` | 主动宣战意愿 |
| `antagonize` | 找茬意愿 |

### 装备 / 生产

| type | 用途 |
|---|---|
| `role_ratio` | 师角色比例（infantry: 80, armor: 8, garrison: 7 …） |
| `unit_ratio` | 单位比例（fighter / bomber / capital_ship …） |
| `equipment_production_factor` | 调整某装备生产倾向（百分比） |
| `equipment_production_min_factories` | 最少 N 个工厂在该装备 |
| `equipment_production_surplus_management` | 富余装备处理 |
| `equipment_market_trade_desire` | 进出口意愿 |
| `equipment_stockpile_surplus_ratio` | 储备过剩阈值 |

### 外交

| type | 用途 |
|---|---|
| `alliance` / `befriend` / `support` / `protect` | 对某 tag 的好感度 |
| `ignore` / `ignore_claim` | 忽略某 tag |
| `send_volunteers_desire` | 派志愿军意愿 |
| `send_lend_lease_desire` | 租借意愿 |
| `diplo_action_desire` / `diplo_action_acceptance` | 外交行为意愿 / 接受度 |

### 海空

| type | 用途 |
|---|---|
| `naval_avoid_region` | 避开某海区 |
| `naval_dominance` | 制海意愿 |
| `strategic_air_importance` | 战略空军重视度 |

## 实战示例：意大利打埃塞俄比亚（vanilla `ai_strategy/ITA.txt`）

```
ITA_commit_to_ethiopian_theatre = {
    enable = {
        has_global_flag = second_italo_ethiopian_war_flag
        country_exists = ETH
        ETH = { has_country_flag = italian_major_offensive_against_ethiopia_flag }
        ITA_is_communist_ai = no
    }
    abort = {
        OR = {
            NOT = { has_global_flag = second_italo_ethiopian_war_flag }
            NOT = { country_exists = ETH }
            date > 1939.6.1
            any_enemy_country = { is_major = yes
                capital_scope = { is_on_continent = europe }
            }
        }
    }
    ai_strategy = { type = theatre_distribution_demand_increase
        id = 559   # Horn of Africa strategic region
        value = 2 }

    ai_strategy = { type = area_priority
        id = horn_of_africa
        value = 100 }

    ai_strategy = { type = front_control
        ratio = 0.5
        priority = 600
        execution_type = rush_weak
        execute_order = yes
        tag = ETH }
}
```

要点：
1. 触发条件依赖 **scripted flag** + **focus 完成** + **真实状态**（country_exists、has_war）
2. 用 `area_priority` + `theatre_distribution_demand_increase` 提高地理倾向
3. 用 `front_control` 给该前线下达"快速攻击弱敌"指令（execute_order=yes 表示真打，manual_attack=yes 表示不让玩家干预）
4. `priority = 600` 数值越高越优先（vanilla 普通前线 priority 100-200，重点战场 500-1000）

## 区域定义（`ai_areas/default.txt`）

```
horn_of_africa = {
    strategic_regions = { 559 ... }
}
italy = {
    strategic_regions = { 23, 21 }
}
north_africa = {
    strategic_regions = { 128, 225, 126, 182 }
}
```

→ AI 用 area 作为"我关心哪片地"的粒度，而不是单个省 / 州。

## 战区定义（`ai_faction_theaters/`）

```
western_europe = {
    name = theater_western_europe
    regions = { 19, 5, 7, 208, 6, 18, 1, 275, 20, 42, 21 }
    ai_will_do = {
        base = 0
        modifier = { add = 1 capital_scope = { is_on_continent = europe } }
        modifier = { add = 5 has_war_with = FRA }
        modifier = { factor = 10 is_historical_focus_on = yes original_tag = USA }
    }
    cancel = { has_war = no }
}
```

→ 战区是阵营级别的"是否要把兵力分到这里"决策。

## 海军 Goal/Objective 系统（`ai_navy/_documentation.md`）

新版海军 AI 用 **Goal / Objective** 模式：
- **Goal** = 高层操作（如 `convoy_protection`）
- **Objective** = goal × 具体目标（如"保护对法国航线"）
- 每个 goal 有 `min_priority..max_priority`
- 每个 objective 有 importance 0-1
- 综合分：`priority_min + (priority_max - priority_min) * importance`
- AI 按总分排序执行尽可能多的 objective

## 与本项目的对照

| Vanilla 概念 | 本项目当前对应 |
|---|---|
| `ai_strategy/<tag>.txt` 集合 | 暂无；硬编码在 hoi4-ai 子模块里 |
| `front_control` | `evaluate_ground` 的 `GroundPosture` |
| `area_priority` | 暂无；当前所有前线"等价" |
| `theatre_distribution_demand_increase` | 暂无；当前师团分配无地理偏好 |
| `front_unit_request` 数值化前线兵力 | 暂无；当前 distribute 只看"前线省数 / 总师数" |
| `execution_type=rush_weak` | 暂无；当前 Attack 总是同一执行 |
| `ai_areas` | 暂无；当前 AI 不知 area 概念 |
| `ai_faction_theaters` | 暂无；当前没有阵营级战区分配 |
| Goal/Objective 海军 | 部分（mission 在 hoi4-state 但没 objective scoring） |

## 重构方向（按优先级）

### 已做（J.10 + 当前轮次）

- ✅ 多前线分兵：`partition_divisions_by_segment` 把师按 BFS 最近 segment 分配
- ✅ Posture 持续性：14 天冷却防止攻防来回切
- ✅ 跨大陆隔离：MAX_RALLY_DISTANCE = 30 跳避免本土师被命令打殖民地战场
- ✅ 攻势分兵：`ATTACK_FRONT_BREADTH = 4` 把师分散到前 4 个高优先级目标

### 下一步可做

1. **Per-tag declarative AI 配置**：把 vanilla `ai_strategy/<tag>.txt` 简化版搬进项目（YAML / Rust 字面量），让 ETH/ITA/SPA/SPR 各有一组 strategy 项
2. **`area_priority`**：定义 horn_of_africa / italy / spain 等 area，给各 country AI 加倾向权重
3. **`front_control` 风格 execution_type**：careful / balanced / rush_weak / rush 影响 attack target 选择策略
4. **`front_unit_request` 数值化**：每条前线声明"需要多少师"，distribute 按 demand 分配
5. **战区级师团调度**：阵营内多国共同支援同一前线（盟军支援 SCW / ETH）
