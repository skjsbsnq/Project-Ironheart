# 测试与验收规范

本文档定义 Project Ironheart 重构线的测试策略。目标是避免“乱测、白测”：每个测试必须证明一个系统契约、业务不变量或用户可见结果。

## 1. 测试原则

### 1.1 不接受的测试

以下测试不算有效验收：

- 只检查不 panic。
- 只检查数字大于 0。
- 只跑一天然后打印日志。
- 只截图但没有比较标准。
- 只测 mock 数据，不测真实 1936 数据。
- 只测单函数，不测系统边界。
- 测试名和断言无业务含义。

### 1.2 有效测试必须说明

每个测试文件顶部必须说明：

- 被测系统。
- 业务契约。
- 输入数据。
- 期望结果。
- 误差容忍。
- 失败时说明什么问题。

### 1.3 验收分层

```text
unit tests
    -> contract tests
    -> subsystem scenario tests
    -> integration replay tests
    -> visual acceptance
    -> performance acceptance
```

单元测试负责公式和边界。验收测试负责玩家可见结果。

## 2. 功能完整性验收

### 2.1 功能矩阵

建立 `feature_parity_matrix.ron` 或同等表格，每个旧功能必须有状态：

- `NotStarted`
- `Skeleton`
- `DataPorted`
- `BehaviorPorted`
- `UiPorted`
- `Tested`
- `Accepted`

不得用“已迁移”这种模糊状态。

### 2.2 必须覆盖的功能域

- 启动和加载。
- 国家选择。
- 地图相机。
- 省份选择。
- 建造。
- 部队选择和命令。
- 时间推进。
- 事件和通知。
- 政治、法律、决议。
- 人口。
- 市场。
- 财政。
- 建造。
- 研究。
- 外交。
- 陆军、海军、空军。
- 后勤。
- 存档和设置。

每个域至少要有：

- 一个 contract test。
- 一个 UI snapshot 或 model test。
- 一个真实 1936 场景验收。

## 3. 渲染验收

### 3.1 固定截图场景

必须固定相机、分辨率、日期、国家、地图模式：

- 欧洲战略远景。
- 中欧军队密集区。
- 地中海和北非。
- 英伦海峡。
- 苏德边境。
- 中国战区。
- 沙漠。
- 雪地。
- 夜间/昼夜边界。
- 国家选择 UI。

### 3.2 每张截图必须附带 metadata

```text
frame_id
camera
map_mode
zoom_class
resource_quality
fallback_count
frame_graph_nodes
counter_raw_count
counter_aggregated_count
counter_hidden_count
collision_count
cpu_ms
gpu_ms
```

### 3.3 视觉通过标准

战略远景：

- 兵牌不能互相覆盖到不可读。
- 国家边界可辨。
- 政治色可辨。
- 水体不能压过陆地信息。
- 顶栏和侧边栏不遮挡关键交互。

中景：

- 前线方向可读。
- 选中单位突出。
- 省份 hover 可见。
- 道路/铁路/补给不和边界混成噪声。

近景：

- 地形、河流、海岸、城市、建筑可辨。
- 标签不闪烁。
- 兵牌展开清楚。

### 3.4 资源质量验收

生产视觉验收要求：

- critical fallback = 0。
- debug fixture = false。
- mock overlay = 0。
- hardcoded route/arrow = 0。

如果 fallback 存在，截图只能标记为 degraded，不能进入 accepted。

### 3.5 图像比较

图像比较不能只用整图像素 diff。需要组合：

- 直方图。
- 边缘密度。
- 标签/兵牌遮挡率。
- 关键区域 crop。
- 人工审阅 checklist。

## 4. V9 UI 验收

### 4.1 UI model tests

每个面板必须测试：

- 输入 snapshot 生成正确行/按钮/状态。
- 空状态。
- 错误状态。
- 权限不足状态。
- command 输出。

### 4.2 Layout snapshot tests

分辨率：

- 1280x720。
- 1920x1080。
- 2560x1440。
- 3440x1440。

验收：

- 无文字溢出。
- 无控件重叠。
- 无旧色值。
- 无非 V9 panel shell。
- 无英文残留，除非该字段明确为 tag/id。

### 4.3 V9 token 守卫

静态检查：

- 面板代码禁止私有 `Color32::from_rgb` 等硬编码颜色。
- 面板代码禁止私有 spacing 常量。
- 面板代码禁止直接设置随意字号。
- 所有按钮、tabs、table、card、modal 使用 V9 primitives。

## 5. 经济验收

### 5.1 历史初始化验收

主要国家：

- USA
- GER
- SOV
- ENG
- FRA
- JAP
- ITA
- CHI

断言：

- 人口误差 <= 2%。
- GDP 误差 <= 5%。
- 三产份额误差 <= 8 个百分点。
- 债务/GDP 误差 <= 5 个百分点。
- 政府收入/GDP 误差 <= 5 个百分点。

失败必须输出 calibration report。

### 5.2 GDP 核算验收

必须证明：

```text
GDP = PrimaryVA + SecondaryVA + TertiaryVA + GovernmentVA + Adjustments
```

断言：

- 各 sector value_added 有来源。
- GDP 不是目标值直接赋值。
- 建筑增加/破坏只影响相关 sector，不直接重写国家 GDP。
- 服务业 GDP 可以存在于非建筑 sector capacity。

### 5.3 POP 验收

断言：

- 国家人口 = 各州 POP 合计。
- 劳动力 <= 总人口。
- 就业人数 <= 劳动力。
- POP income 有来源。
- disposable income = income - taxes + transfers。
- 消费订单不长期超过可支配收入。
- 征兵减少对应可用劳动力或兵源池。

### 5.4 财政验收

每日财政账：

```text
closing_cash = opening_cash + income - expense + financing
```

断言：

- 每个现金变化有 ledger entry。
- 债务本金变化有 liability entry。
- 利息 = 本金 * 利率 / 365，误差在货币 rounding 内。
- MEFO/隐性融资不混入普通税收。
- 军费采购能追踪到预算、市场订单、装备交付。

### 5.5 市场验收

断言：

- fulfilled demand <= demand。
- used supply <= production + stockpile + imports。
- unmet demand 有记录。
- 政府采购不会无限创造商品。
- POP 消费会影响需求和满意度。
- 价格变化有上限和原因。

## 6. 经济场景测试

### Scenario E-1936-Init

加载 1936，检查主要国家人口、GDP、财政、三产份额。

### Scenario E-GER-90D

德国运行 90 天：

- GDP 不应被硬锁为历史值。
- 三产结构变化在合理范围。
- 财政 ledger 平衡。
- MEFO 发行和债务增长可解释。
- 军事采购影响预算和市场。

### Scenario E-Construction

开始建设：

- 建设产生材料需求。
- 建设产生工资支出。
- 建设进度受材料和财政影响。
- 完工后生成资产。
- GDP 通过 sector account 后续变化，不是完工瞬间强行加目标 GDP。

### Scenario E-Pop-Tax

提高税率：

- 税收上升。
- disposable income 下降。
- POP 消费下降或结构变化。
- 满意度下降。
- GDP 不应因为税率变化直接被重写。

### Scenario E-War-Mobilization

战时动员：

- 士兵 POP 或兵源变化。
- 军费上升。
- 消费品供给/需求变化。
- 军工 sector 上升。
- 民用 sector 受到挤压。

## 7. 模拟验收

### 7.1 确定性

同一 seed、同一输入、同一命令序列，运行 365 天结果必须一致。

### 7.2 调度

断言：

- daily 系统一天只执行一次。
- weekly 系统一周只执行一次。
- monthly 系统一月只执行一次。
- 暂停时不推进模拟。
- 高速时不丢日 tick。

### 7.3 性能

经济和 UI 不允许靠每帧全量重算。

指标：

- 经济 daily tick 有预算。
- UI frame model 构建有预算。
- 渲染 prepare 有预算。
- 365 天 replay 不超过基线门槛。

## 8. 回归验收

### 8.1 Golden Snapshots

保存：

- `economy_snapshot_1936_01_01.json`
- `economy_snapshot_1936_04_01.json`
- `ui_snapshot_topbar_1936.json`
- `render_metadata_europe_strategic.json`

任何变更必须解释 snapshot diff。

### 8.2 允许变化

允许变化必须写明原因：

- 公式修正。
- 数据修正。
- 新历史资料。
- 性能优化导致排序变化。
- UI layout 更新。

### 8.3 不允许变化

- 现金账不平。
- GDP 失去三产合计关系。
- POP 总人口无原因变化。
- critical fallback 混入 accepted screenshot。
- V9 以外 UI 新增。

## 9. CI 分组

### fast

每次提交跑：

- 单元测试。
- 合约测试。
- schema 测试。
- V9 token 守卫。
- frame graph plan 测试。

### standard

PR 跑：

- 1936 初始化。
- 90 天主要国家经济。
- UI snapshot。
- 固定截图 metadata。

### heavy

里程碑跑：

- 365 天 replay。
- 多国家 AI。
- 视觉截图全套。
- 性能 profiling。

## 10. 测试命名规范

测试名必须描述业务结果：

好：

```text
gdp_equals_sector_value_added_sum_for_1936_germany
fiscal_ledger_balances_after_mefo_financed_procurement
strategic_zoom_aggregates_division_counters_in_europe
v9_finance_panel_shows_debt_and_cashflow_without_overlap
```

差：

```text
test_gdp
test_tick
smoke
works
no_panic
```

## 11. 验收出口

一个系统只有满足以下条件才能标记 accepted：

1. 功能矩阵状态为 `Accepted`。
2. 有至少一个真实数据场景测试。
3. 有 contract tests。
4. 有快照或可解释输出。
5. 有性能预算结果。
6. 文档说明数据来源和公式。
7. 没有 critical fallback。
8. 没有 debug fixture 污染生产路径。
