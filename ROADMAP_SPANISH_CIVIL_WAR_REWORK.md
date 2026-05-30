# 西班牙内战重做路线图

目标：把现有西班牙内战从简单新闻播报和固定局势，重做为中文事件驱动、可游玩、可分支、有历史张力、有后果记忆的内容包。设计参考 HOI4 大型 DLC、Kaiserreich、TNO 的事件密度和叙事方法，但优先保证当前项目可落地。

本文是任务顺序路线图，不代表当前已实现。

## 当前实施范围锁定

当前第一轮实施不再追求一次性完成共和国、国民军、CNT、共和国胜利、CNT 胜利和所有战后路线。

第一轮锁定范围：

- 内战前内容完整制作：1936 人民阵线胜选、社会撕裂、军官密谋、街头暴力、教会/土地危机、最后内阁会议、政变爆发前评分全部要做。
- 内战爆发和阵营选择完整制作：`SPR` 开局、`SPA` 生成、玩家选择共和国或国民军、中文新闻、初始化和清理链必须可用。
- 内战中可玩主线只做国民军：国民军战争、军政府权力斗争、佛朗哥/莫拉/长枪党/卡洛斯派/教会/外援依赖全部进入第一轮。
- 战后路线只做国民军：佛朗哥胜利、长枪党胜利、长枪党架空佛朗哥与制度化国家工团主义三条国民军战后路线进入第一轮。
- 共和国内战中内容本轮只做最小支撑：保留必要事件、flags、AI/背景压力、被国民军事件读取的状态和新闻触发，不做完整共和国可玩路线。
- CNT/POUM 本轮只做触发和压力来源：用于五月事件风险、共和国崩坏背景、国民军胜利后的清算记忆，不做完整 CNT 可玩路线和 CNT 胜利战后路线。
- 国际干涉本轮优先服务国民军：德国、意大利、葡萄牙、英国直布罗陀观察和战后索偿优先；苏联、法国、墨西哥只保留战前/共和国背景所需最小事件。

第一轮以“战前完整 + 国民军内战完整 + 国民军战后完整”为交付标准。共和国完整内战路线、共和国胜利战后路线、CNT 胜利路线和完整外国干涉国玩法全部进入长期扩展。

## 强制规则

- 所有新增西班牙内战事件标题必须使用中文。
- 所有新增西班牙内战事件描述必须使用中文。
- 所有新增西班牙内战事件选项必须使用中文。
- 所有西班牙内战新闻事件也必须使用中文。
- 事件 id、flag、文件名、effect 名称继续使用 ASCII 英文，避免破坏内容加载和调试。
- 中文事件文本必须避免机器翻译腔，必须有人物、场景、政治压力和选择代价。
- 禁止只写“某地陷落，世界震惊”式空洞新闻。
- 重要事件至少要有一个后续 flag，让后续事件能记住玩家选择。
- 重要选择必须有代价，不允许所有选项都是纯奖励。
- 事件效果必须和当前项目的实际系统联动，优先作用于 flags、变量、决议机制条、POP、劳动力、满意度、激进化、建筑等级、建造队列、生产方式、商品供需、库存、贸易、财政、军工订单、装备补给、铁路/港口/基础设施、占领秩序、抵抗和外交关系。
- 禁止沿用 HOI4 式空泛效果模板，例如“消费品工厂 -10%”“增加军用工厂”“获得 2 个民用工厂”“生产效率 +10%”这类不经过当前建筑生产、市场、POP、财政和军需系统解释的效果。
- 如果某事件看似要“增加军工能力”，必须拆成当前系统可解释的后果：例如修复某州 `arms_industry`/`munition_plant` 建筑等级、改变对应生产方式、增加政府军工订单、补充机床/钢/煤/弹药库存、提高就业或转移劳动力，而不是直接给抽象军工厂。
- 如果某事件看似要“降低消费压力”，必须写成粮食、燃料、纺织品、工资、配给、进口、黑市、财政补贴、POP 需求满足率或满意度变化，而不是写“消费品需求 -X%”。

## KR/TNO 风格研究结论

网络抽样参考：TV Tropes 对 Kaiserreich 与 The New Order: Last Days of Europe 的介绍和条目总结。

Kaiserreich 风格要点：

- 强调架空史合理性和世界线互文。
- 大量国家都有自己的政治危机、内战、复国、革命或军政府路线。
- 事件常围绕“现实政治交易”展开，不只是意识形态口号。
- Civil War 内容常有多派系、多继承者、多种妥协和背叛。
- 真实历史人物被放进不同命运，但仍保留历史性格和政治逻辑。
- 事件经常使用现实历史的镜像、反讽和变体。

TNO 风格要点：

- 明确是 narrative-driven mod，偏政治模拟和事件叙事，而不是单纯战争征服。
- 氛围更黑暗、更压抑，强调制度腐败、社会崩坏、人物心理和国家机器压力。
- 大量事件、路线、人物、专属机制共同塑造国家体验。
- 战争不是唯一玩法，预算、经济、政治派系、社会危机都能成为核心体验。
- 事件文本更重视场景感、人物视角、末日感和长期后果。

转化到西班牙内战的设计原则：

- KR 式：西班牙各派系要有政治逻辑，不能只有红白二分。
- KR 式：真实历史事件可以有替代分支，但必须讲得通。
- KR 式：外国援助必须是交易，不是免费按钮。
- TNO 式：事件要有场景、人物、恐惧、疲惫和制度压力。
- TNO 式：共和国、国民军、CNT 都要有内部崩坏风险。
- TNO 式：玩家每次为了活下去做出的妥协，后面都要回来收账。

## 当前内容问题

现有西班牙内战内容主要位置：

- `crates/hoi4-content/content/situations/spanish_civil_war.ron`
- `crates/hoi4-content/content/situations/spanish_anarchist_uprising.ron`
- `crates/hoi4-content/content/news_events.ron`
- `crates/hoi4-content/content/scenarios/1936.ron`

现有问题：

- 西班牙事件几乎都是新闻描述，没有真正选择。
- 事件选项通常没有后果，不能改变后续路线。
- 共和国、国民军、CNT 的内部派系没有被系统表现。
- 战役新闻只是“某地陷落”，没有转化为政治危机、军队改革或国际反应。
- 国民军 `SPA` 是运行时生成国家，不能开局选择，只能内战后切换。
- 国家选择菜单当前主要只开放德国，开放 `SPR` 需要菜单和内容玩家同步。
- `ContentRuntimeState.player` 与 `world.player` 需要在选择国家和切换阵营时同步。
- 当前 `Effect` 缺少 `SwitchPlayerCountry` 一类效果，无法用 RON 事件直接切换玩家阵营。
- `spanish_anarchist_uprising` 当前固定日期触发，缺少共和国状态、内战状态和政治选择判断。

## 总体内容规模

当前第一轮不是“所有派系完整重做”，而是“战前完整 + 国民军内战/战后完整”。

第一轮可交付目标约 120 到 160 个事件。

长期完整重做目标约 200 到 260 个事件。

第一轮最小目标：

- 战前共和国危机事件：15 到 20
- 国民军内战事件：30 到 40
- 国民军战后路线事件：45 到 70
- CNT/POUM 压力和背景事件：4 到 8
- 国际干涉事件：12 到 18，其中德意葡和英国直布罗陀观察优先
- 战役新闻和隐藏自动事件：20 到 30

完整目标：

- 共和国事件：30
- 国民军事件：25
- CNT/POUM 事件：15
- 国际干涉事件：20
- 战役新闻事件：15
- 隐藏自动事件：15

完整目标保留给长期扩展，不作为第一轮验收标准。

## 文件结构目标

长期目标是拆出独立文件，不继续堆在 `news_events.ron`：

- `crates/hoi4-content/content/events/SPR_events.ron`
- `crates/hoi4-content/content/events/SPA_events.ron`
- `crates/hoi4-content/content/events/CNT_events.ron`
- `crates/hoi4-content/content/events/spanish_civil_war_news.ron`
- `crates/hoi4-content/content/events/spanish_civil_war_hidden.ron`
- `crates/hoi4-content/content/situations/spanish_civil_war.ron`
- `crates/hoi4-content/content/situations/spanish_anarchist_uprising.ron`

`news_events.ron` 长期只保留通用世界新闻或逐步迁出西班牙专属新闻。

## P-1：开工前旧内容清空

正式开始西班牙内战重做前，必须先清空旧西班牙局势和旧西班牙新闻，避免新旧链条同时触发。

### P-1.1 删除旧西班牙局势定义

目标：移除现有单薄的旧局势链。

任务：

- 删除或清空 `crates/hoi4-content/content/situations/spanish_civil_war.ron`。
- 删除或清空 `crates/hoi4-content/content/situations/spanish_anarchist_uprising.ron`。
- 从 `crates/hoi4-content/content/scenarios/1936.ron` 的 `situation_libraries` 中移除旧西班牙局势文件引用。
- 从 `crates/hoi4-content/content/scenarios/1936.ron` 的 `situation_ids` 中移除旧 `spanish_civil_war` 和 `spanish_anarchist_uprising`。
- 等新链条 P2 完成后，再以新结构重新加入西班牙内战局势。

验收：旧西班牙内战不会在 1936-07-17 自动触发。

### P-1.2 删除旧西班牙新闻事件

目标：移除旧的空洞西班牙新闻播报。

任务：

- 从 `crates/hoi4-content/content/news_events.ron` 中删除旧西班牙新闻事件。
- 删除目标包括：
  - `news.scw_outbreak`
  - `news.madrid_falls`
  - `news.barcelona_falls`
  - `news.scw_nationalist_victory`
  - `news.scw_republican_victory`
- 等 P1.3 时在 `spanish_civil_war_news.ron` 中用中文新闻重建。

验收：旧英文西班牙新闻不再存在。

### P-1.3 删除旧德国西班牙干涉触发引用

目标：避免旧 `germany.spanish_civil_war` 和新干涉链重复。

任务：

- 审查 `germany.spanish_civil_war` 当前定义和触发位置。
- 从旧西班牙局势 `on_start` 中移除 `TriggerEvent("germany.spanish_civil_war")`。
- 如果要保留德国干涉，必须在 P7.1 用新 `ger.scw_condor_legion` 中文事件链重建。

验收：德国不会因旧局势触发旧式干涉事件。

### P-1.4 建立临时空窗保护

目标：旧内容删掉后，未完成新内容前不产生缺失引用。

任务：

- 删除旧引用后运行内容加载验证。
- 确保没有 `TriggerEvent` 指向已删除事件。
- 确保 `1936.ron` 不引用已删除局势文件。
- 确保测试或 registry 里没有强制要求旧西班牙局势存在。

验收：删除旧内容后项目仍能加载 1936 场景。

## P0：技术基础和可玩性

### P0.1 审计玩家国家字段

目标：确认 `player_country`、`world.player`、`content.player` 的所有使用点。

任务：

- 搜索并记录所有玩家国家字段读写路径。
- 标记国家选择、事件系统、局势系统、AI 干预、UI 面板、存档读取中的玩家字段依赖。
- 确认当前国家选择只更新 `self.player_country` 和 `self.world.player` 的风险。

验收：形成明确同步点，避免后续只改一个字段。

### P0.2 新增统一玩家切换 helper

目标：所有切换玩家国家的地方都走同一条路径。

任务：

- 新增 helper，例如 `set_player_country_by_tag` 或 `set_player_country_by_id`。
- helper 同步更新 `self.player_country`、`self.world.player`、`self.content.player`。
- helper 清理 UI cache。
- helper 刷新可见性、国家标签、地图缓存。
- helper 对不存在 tag 返回 warning，不 panic。

验收：菜单选择和未来事件切换都调用同一 helper。

### P0.3 开放西班牙共和国开局游玩

目标：`SPR` 可从新游戏菜单选择。

任务：

- 在国家选择列表加入 `SPR`。
- 在国家名称映射加入 `SPR => "西班牙共和国"`。
- 将 `enabled` 从只允许 `GER` 改为允许 `GER` 与 `SPR`。
- 鼠标点击开始游戏路径调用 P0.2 helper。
- 键盘 Enter 开始游戏路径调用 P0.2 helper。

验收：开局可选择 `SPR`，进入游戏后事件、局势、UI 都以 `SPR` 为玩家国家。

### P0.4 新增切换玩家国家效果

目标：RON 事件选项能切换玩家到 `SPR` 或 `SPA`。

任务：

- 在 `Effect` 中新增 `SwitchPlayerCountry(String)` 或等价命令。
- 在效果执行报告或 app 层命令中传递切换请求。
- 切换请求调用 P0.2 helper。
- 目标国家不存在时记录 warning。
- 添加 `SPA` 运行时生成后可切换测试。

验收：事件选项可以把玩家切到国民军 `SPA`。

### P0.5 修正事件暂停和新闻行为

目标：阵营选择必须暂停，世界新闻不强制承担选择功能。

任务：

- 阵营选择事件使用 `scope: Country`。
- 世界新闻 `news.scw_outbreak` 保持 `scope: News`。
- 确保国家事件 pending 时暂停。
- 确保新闻事件不会阻塞阵营选择。

验收：内战爆发后先能做阵营选择，新闻仍可展示。

### P0.6 建立中文事件校验清单

目标：后续写事件时统一检查中文规范。

任务：

- 规定西班牙事件 `title`、`description`、`EventOption.name` 必须中文。
- 规定 id 和 flags 使用英文 snake_case。
- 规定每个重要事件必须有至少一个效果或 flag。
- 规定重要事件必须至少两个选项，纯新闻可一个选项。

验收：路线图和 PR 检查都能引用这套规范。

### P0.7 决议系统从政治面板解绑

目标：决议不再附属于政治面板，而是成为独立一级面板。

任务：

- 审计当前决议入口、政治面板入口、决议状态显示和决议执行路径。
- 从政治面板中移除决议 UI 入口和渲染责任。
- 新增独立 `Decisions` 面板入口。
- 顶栏或侧边栏提供稳定入口，不能藏在政治面板内部。
- 保留现有决议数据和执行逻辑，不在解绑阶段重写全部决议系统。
- 确保打开决议面板时能读取当前玩家国家。

验收：玩家可以不进入政治面板，直接打开独立决议面板查看和执行决议。

### P0.8 建立通用决议面板框架

目标：让决议面板支持普通决议、国家专属机制和小型玩法模块。

任务：

- 决议面板按国家、分类、可用状态、进行中状态分组。
- 支持普通按钮式决议。
- 支持持续进度式决议。
- 支持带冷却和时限的决议。
- 支持带说明文本、风味文本和后果预览。
- 支持国家专属 section，例如“共和国政府危机”“CNT 关系”“国民军派系统合”。
- 支持隐藏不可见决议、灰显不可用决议和可用高亮决议。
- 决议面板所有西班牙专属显示文本必须中文。

验收：普通决议和国家专属决议能在同一个独立面板中展示，但视觉上有明确分区。

### P0.9 建立 KR/TNO 式决议小游戏接口

目标：决议面板不仅是按钮列表，还能承载国家专属小机制。

任务：

- 定义 `DecisionMechanicKind` 或等价分型，用于区分普通决议和特殊机制。
- 支持数值条：影响力、紧张度、权威、革命热度、外援依赖。
- 支持多方竞争条：例如 CNT、共产党、共和政府三方权力平衡。
- 支持地图目标列表：例如关键州防御、边境补给、外援港口。
- 支持周期事件和决议联动：决议改变数值，事件读取数值或 flags。
- 支持国家专属 UI 文案，不同国家不能共用一套无味模板。

验收：决议系统能承载 KR/TNO 式小型机制，而不是只有“花 PP 点按钮”。

### P0.10 西班牙内战专属决议机制草案

目标：为西班牙第一轮重做准备最小可用的专属决议小游戏。

共和国专属机制：

- “共和国权威”：政府权威 vs 民兵自治。
- “苏联影响”：外援效率 vs 政治独立。
- “CNT 紧张度”：妥协、整编、镇压和五月事件风险。
- “马德里防御”：投入武器、民兵、国际纵队来延缓首都危机。

国民军专属机制：

- “佛朗哥权威”：军政府合议 vs 个人独裁。
- “右翼派系平衡”：长枪党、卡洛斯派、教会、非洲军团。
- “外援依赖”：德国影响、意大利影响和战后索偿风险。
- “占领区秩序”：恐怖镇压 vs 行政治理。

CNT 专属机制：

- “革命委员会”：集体化、民兵自治、共和国关系。
- “巴塞罗那紧张度”：电话局危机和五月事件风险。
- “革命输出 vs 生存妥协”：胜利后的方向。

外国干涉国机制：

- 德国：秃鹰军团实验、空军经验、国际舆论风险。
- 意大利：CTV 投入、威望、瓜达拉哈拉失败风险。
- 苏联：援助效率、黄金储备、政治条件。
- 法国：边境开放压力、国内右翼反弹。
- 英国：不干涉委员会和外交压力。

验收：西班牙内战不仅通过事件推进，也通过决议面板提供持续操作和风味机制。

## P1：内容文件拆分和事件骨架

### P1.1 新建西班牙事件库文件

目标：建立干净的西班牙内容结构。

任务：

- 新建 `SPR_events.ron`。
- 新建 `SPA_events.ron`。
- 新建 `CNT_events.ron`。
- 新建 `spanish_civil_war_news.ron`。
- 新建 `spanish_civil_war_hidden.ron`。

验收：文件存在，但可以先只放最小事件。

### P1.2 更新 1936 manifest

目标：让新事件库被 1936 场景加载。

任务：

- 在 `crates/hoi4-content/content/scenarios/1936.ron` 加入新事件库。
- 保证没有重复事件 id。
- 保证 `TriggerEvent` 引用目标存在。

验收：内容加载成功。

### P1.3 迁移现有西班牙新闻

目标：把西班牙新闻从通用 `news_events.ron` 中拆到专属新闻文件。

任务：

- 迁移 `news.scw_outbreak`。
- 迁移 `news.madrid_falls`。
- 迁移 `news.barcelona_falls`。
- 迁移 `news.scw_nationalist_victory`。
- 迁移 `news.scw_republican_victory`。
- 将标题、描述、选项全部改为中文。

验收：旧新闻仍能触发，但文本为中文，文件结构更清晰。

### P1.4 建立事件命名规范

目标：避免事件 id 混乱。

规范：

- 共和国国家事件：`spr.scw_xxx`
- 国民军国家事件：`spa.scw_xxx`
- CNT 国家事件：`cnt.scw_xxx`
- 西班牙世界新闻：`news.scw_xxx`
- 隐藏自动事件：`hidden.scw_xxx`
- 玩家选择事件：`spain.scw_xxx`

验收：后续所有事件使用统一命名。

### P1.5 建立第一批 flags

目标：让事件链能记住选择。

共和国 flags：

- `spr_armed_unions`
- `spr_refused_to_arm_unions`
- `spr_gold_to_moscow`
- `spr_soviet_advisors_empowered`
- `spr_protected_poum`
- `spr_cracked_down_on_poum`
- `spr_conceded_to_cnt`
- `spr_suppressed_cnt`
- `spr_government_fled_madrid`
- `spr_people_army_reformed`

国民军 flags：

- `spa_franco_supreme_command`
- `spa_mola_prominent`
- `spa_sanjurjo_dead`
- `spa_falange_empowered`
- `spa_carlist_concessions`
- `spa_unification_decree`
- `spa_church_crusade`
- `spa_white_terror_expanded`
- `spa_german_dependency`
- `spa_italian_dependency`

CNT/POUM flags：

- `cnt_collectivization_recognized`
- `cnt_militias_integrated`
- `cnt_refused_integration`
- `poum_protected`
- `poum_outlawed`
- `barcelona_may_days_triggered`
- `cnt_uprising_prevented`
- `cnt_uprising_started`

国际 flags：

- `ger_condor_legion_sent`
- `ita_ctv_sent`
- `sov_gold_received`
- `fra_border_opened`
- `eng_non_intervention_pressure`
- `mex_republican_aid`
- `por_nationalist_support`

验收：第一批事件只使用这批 flags，避免前期命名膨胀。

## P2：内战爆发和阵营选择

### P2.1 重写内战爆发 on_start 顺序

目标：保证 `SPA` 生成后再弹阵营选择。

任务：

- 确认 `SplitCountry(source_tag: "SPR", rebel_tag: "SPA")` 先执行。
- 生成双方前线部队。
- 拆分共和国原有部队。
- 给双方初始装备和经验。
- 触发 `spain.scw_choose_side`。
- 触发 `news.scw_outbreak`。

验收：选择国民军时 `SPA` 已存在。

### P2.2 新增中文阵营选择事件

目标：玩家在内战爆发时选择继续共和国或切到国民军。

事件：`spain.scw_choose_side`

标题示例：`西班牙裂成两半`

选项：

- `保卫共和国。`
- `加入国民军。`

任务：

- 事件描述必须中文。
- 选项 A 设置 `player_chose_republic`。
- 选项 A 切换玩家到 `SPR`。
- 选项 B 设置 `player_chose_nationalists`。
- 选项 B 切换玩家到 `SPA`。

验收：玩家从 `SPR` 开局后可在内战爆发时选择阵营。

### P2.3 重写中文世界新闻

目标：爆发新闻不再是空洞描述。

事件：`news.scw_outbreak`

要求：

- 中文标题。
- 中文描述。
- 描述要提到军队叛乱、工人武装、共和国合法政府、欧洲震动。
- 选项只作新闻确认，不承担切换玩家。

验收：非西班牙玩家看到中文新闻。

### P2.4 新增双方开局国家事件

目标：玩家选边后立刻进入该阵营内部叙事。

共和国事件：`spr.scw_government_in_crisis`

国民军事件：`spa.scw_junta_forms`

任务：

- 共和国事件强调政府、民兵、军队忠诚危机。
- 国民军事件强调军政府、非洲军团、佛朗哥和莫拉。
- 两者都必须有两个以上选项。
- 两者都必须设置后续 flags。

验收：选择阵营后不是直接回地图，而是进入阵营身份叙事。

## P2.5：第一轮事件变量系统补课

状态：已完成。

目标：第一轮不能只靠 flags 硬撑国民军权力斗争、共和国压力、CNT 风险和战后路线分流。至少要有轻量变量系统，让事件和决议机制能读写数值。

第一轮变量：

- `spr_soviet_influence`
- `spr_revolutionary_pressure`
- `spr_military_centralization`
- `spa_franco_authority`
- `spa_falange_power`
- `spa_army_loyalty`
- `spa_church_influence`
- `spa_carlist_anger`
- `spa_foreign_dependency`
- `spa_occupation_order`
- `spa_syndicalist_institutionalization`
- `foreign_intervention_pressure`

任务：

- ✅ 新增事件效果：增加、减少、设置、钳制变量。
- ✅ 新增事件触发条件：变量大于、小于、等于、区间判断。
- ✅ 变量必须按国家存储，避免 `SPR`、`SPA`、`CNT` 互相污染。
- ✅ 变量必须能被隐藏事件、决议机制、事件选项可用条件和 tooltip 读取。
- ✅ 变量变化必须能进入事件效果预览或日志，便于调试。

验收：✅ P3-P9 能用变量表达压力和路线权重，不需要为每个强弱档位都新增一个 flag。

实现细节：
- 在 `CountryStore` 添加 `variables: Vec<HashMap<String, f32>>`
- 新增 `Effect::SetVariable`, `AddToVariable`, `ClampVariable`
- 新增 `Trigger::VariableAtLeast`, `VariableAtMost`, `VariableInRange`, `VariableEquals`
- 在 `eval.rs` 实现变量读写和比较逻辑
- 在 `effect_summary` 添加中文变量效果预览

## P2.6：第一轮人物和领导人效果补课

状态：已完成。

目标：国民军内战和战后路线大量依赖桑胡尔霍、莫拉、佛朗哥、赫迪利亚、卡瓦内利亚斯、金德兰、巴雷拉、苏涅尔等人物命运，不能只靠文本假装人物变化。

第一轮最小效果：

- `SetLeader` 或等价效果。
- `KillCharacter` 或等价效果。
- `ExileCharacter` 或等价效果。
- `RecruitCharacter` 或等价效果。
- `SetCharacterAvailable` 或等价效果。
- 人物状态可被事件条件读取。

第一轮任务：

- ✅ 桑胡尔霍坠机必须能标记人物死亡。
- ✅ 莫拉死亡必须能改变国民军权力分流。
- ✅ 赫迪利亚被捕、妥协或掌握秘书处必须能记录人物状态。
- ✅ 佛朗哥成为考迪罗必须能改变国家领导人或至少改变可见领导状态。
- ✅ 人物效果暂不要求完整 portrait UI，但必须可调试、可保存、可被后续事件读取。

验收：✅ P5 和 P9 的人物命运不是纯文本，后续事件能读到人物是否死亡、被捕、流亡、可用或掌权。

实现细节：
- 在 `CountryStore` 添加 `characters: Vec<HashMap<String, CharacterRuntimeStatus>>`
- 新增 `CharacterRuntimeStatus` 枚举：Available, InOffice, Absent, Imprisoned, Exiled, Dead
- 新增 `Effect::KillCharacter`, `ExileCharacter`, `ImprisonCharacter`, `RecruitCharacter`, `SetCharacterStatus`, `SetLeader`
- 新增 `Trigger::CharacterDead`, `CharacterAvailable`, `CharacterIsLeader`
- 在 `eval.rs` 实现人物状态读写逻辑
- 在 `effect_summary` 添加中文人物效果预览

## P2.7：第一轮事件呈现和效果预览补课

状态：已完成。

目标：第一轮会有大量国家事件、新闻事件、隐藏事件和路线分流。事件必须让玩家看懂选择后果，不能只弹长文。

第一轮任务：

- ✅ 事件选项显示后果预览：flags、变量、人物状态、外交关系、财政/贸易/建筑/POP/补给等关键效果。
- ✅ 新闻事件和国家事件在版式或标识上有区分。
- ✅ 隐藏事件不弹窗，但必须能写入调试日志或事件日志。
- ✅ 多事件队列必须能连续阅读，不吞事件、不乱序、不让阵营选择被新闻盖住。
- 事件图片、音效可暂缓，但需要保留字段和默认占位，避免后续重写 schema。

验收：✅ P3-P9 的重要选择能在 UI 中看到可解释后果，不需要玩家猜 flags 和变量变化。

实现细节：
- 在 `event_panel.rs` 为事件选项添加效果预览（调用 `effect_summary()`）
- 为新闻事件添加视觉标识（📰 前缀和"世界新闻"标签）
- 在 `EventScheduler` 添加 `hidden_log: VecDeque<HiddenEventLogEntry>`
- 新增 `HiddenEventLogEntry` 结构记录隐藏事件触发
- 在 `fire()` 和 `fire_news()` 中调用 `push_hidden_log()` 记录隐藏事件
- 导出 `HiddenEventLogEntry` 供外部读取

## P3：战前危机链

### P3.1 人民阵线胜选

状态：已完成。

目标：让 1936 开局有政治背景。

事件：`spr.scw_popular_front_victory`

任务：

- ✅ 中文描述人民阵线胜选后的期待和恐惧。
- ✅ 选项区分温和改革、深化改革、拖延改革。
- ✅ 设置后续 flag。

实现细节：

- 新增 `spr.scw_popular_front_victory`，1936-02-17 后由 `SPR` 自动触发。
- 三个分支分别设置 `spr_popular_front_moderate_reform`、`spr_popular_front_deepened_reform`、`spr_popular_front_delayed_reform`。
- 选项调整 `spr_revolutionary_pressure` 与 `spr_military_centralization`，供后续土地、教会、军官密谋和共和国压力链读取。

### P3.2 土地和教会危机

状态：已完成。

目标：把社会撕裂做成机制压力。

事件：

- `spr.scw_land_seizures`
- `spr.scw_church_conflict`

任务：

- ✅ 土地事件影响稳定和左翼压力。
- ✅ 教会事件影响右翼反弹和国民军宣传。

实现细节：

- 新增 `spr.scw_land_seizures`，1936-03-10 后在人民阵线胜选链之后触发。
- 新增 `spr.scw_church_conflict`，1936-04-20 后在土地危机处理后触发。
- 土地危机通过 `spr_land_collectives_legalized`、`spr_land_arbitration_commissions`、`spr_land_seizures_repressed` 记录乡村选择。
- 教会危机通过 `spr_church_compromise`、`spr_church_secularization_pushed`、`spr_church_conflict_drifted` 记录政教冲突处理方式。
- 两个事件均调整 `AddStability`、`spr_revolutionary_pressure`、`spr_military_centralization` 与 `foreign_intervention_pressure`，供后续军官密谋、右翼宣传、外部干涉和内战开局读取。

### P3.3 街头暴力和军官密谋

状态：已完成。

目标：让政变有铺垫。

事件：

- `spr.scw_falange_street_violence`
- `spr.scw_mola_conspiracy`
- `spr.scw_officer_transfers`

任务：

- ✅ 每个事件都有选择。
- ✅ 选择影响内战开局双方优势。

实现细节：

- 新增 `spr.scw_falange_street_violence`，1936-05-10 后在教会危机处理后触发。
- 新增 `spr.scw_mola_conspiracy`，1936-06-05 后在街头暴力处理后触发。
- 新增 `spr.scw_officer_transfers`，1936-06-25 后在莫拉密谋处理后触发。
- 街头暴力通过 `spr_falange_banned`、`spr_street_patrols_expanded`、`spr_unions_policed_after_falange_clashes` 记录政府如何处理左右街头武装。
- 莫拉密谋通过 `spr_mola_network_raided`、`spr_mola_network_monitored`、`spr_mola_conspiracy_appeased` 记录政府如何面对军官阴谋。
- 军官调任通过 `spr_suspect_officers_transferred`、`spr_loyal_officer_network_prepared`、`spr_suspect_officers_appeased` 记录内战爆发前的军队布局。
- 三个事件均调整 `AddStability`、`AddWarSupport`、`PoliticalPower`、`ArmyExperience`、`spr_revolutionary_pressure`、`spr_military_centralization` 与 `foreign_intervention_pressure`，供 P3.4、P4、P5 和内战开局读取。

### P3.4 卡尔沃·索特洛遇刺和最后内阁会议

状态：已完成。

目标：内战爆发前最后转折。

事件：

- `spr.scw_calvo_sotelo_assassinated`
- `spr.scw_last_cabinet_meeting`

任务：

- ✅ 事件必须中文。
- ✅ 选择是否武装工会。
- ✅ 选择是否清洗可疑军官。
- ✅ 设置 `spr_armed_unions` 或 `spr_refused_to_arm_unions`。

实现细节：

- 新增 `spr.scw_calvo_sotelo_assassinated`，1936-07-13 后在军官调任事件后触发。
- 新增 `spr.scw_last_cabinet_meeting`，1936-07-16 后在索特洛遇刺事件后触发。
- 索特洛事件通过 `spr_calvo_case_public_inquiry`、`spr_calvo_case_suppressed`、`spr_calvo_funeral_watched` 记录政府如何处理刺杀政治后果。
- 最后内阁会议通过 `spr_armed_unions`、`spr_refused_to_arm_unions`、`spr_suspect_officers_purged`、`spr_officer_purge_delayed`、`spr_legal_chain_of_command_preserved` 记录内战前最后决策。
- 调整 `spr.scw_government_in_crisis`：内战爆发后的共和国危机事件不再设置或覆盖 `spr_armed_unions` / `spr_refused_to_arm_unions`，改为设置 `spr_scw_command_chain_prioritized` 或 `spr_scw_weapon_depots_opened`，确保 P4 读取的是 P3.4 战前选择。
- 两个事件均调整 `AddStability`、`AddWarSupport`、`AddPoliticalPower`、`AddManpower`、`ArmyExperience`、`spr_revolutionary_pressure`、`spr_military_centralization` 与 `foreign_intervention_pressure`，让内战爆发前的选择影响开局状态。

验收：P3 完成后，内战爆发不再像固定日期脚本，而是战前选择的结果。

## P4：共和国战中最小支撑路线

本阶段第一轮只做共和国在内战中作为玩家可选阵营、AI 对手、事件压力来源和国民军叙事镜像所必需的内容。完整共和国可玩路线和共和国胜利战后路线进入长期扩展。

### P4.1 共和国三条压力轴

状态：已完成。

目标：建立国民军路线能读取的共和国状态，而不是本轮完成完整共和国玩法。

压力轴：

- 国家权威：民兵自治 vs 中央集权人民军。
- 革命程度：保卫共和国 vs 社会革命。
- 外国依赖：自主抗战 vs 苏联依赖。

任务：

- ✅ 用 flags 表示路线选择。
- ✅ 可使用 P2.5 的轻量变量系统；flags 只用于离散选择和历史记忆，变量用于压力强弱和路线权重。
- ✅ 每条压力轴至少保留 1 到 2 个关键事件或隐藏汇总事件。
- ✅ 这些状态必须能影响国民军战役、外援、五月事件风险和战后清算文本。
- ✅ P4 只能读取和延伸 P3 的战前选择，不得重新覆盖 `spr_armed_unions`、`spr_refused_to_arm_unions`、政变准备度等战前结果。

实现细节：

- 新增 `hidden.scw_republic_pressure_axes_init`，内战开始后由 `SPR` 自动触发一次。
- 国家权威轴写入 `spr_axis_authority_militia_autonomy`、`spr_axis_authority_state_command`、`spr_axis_authority_purged_command`、`spr_axis_authority_compromised_staff`、`spr_axis_authority_centralizing`。
- 革命程度轴写入 `spr_axis_revolution_social_revolution`、`spr_axis_revolution_republican_defense`、`spr_axis_revolution_land_collectives`、`spr_axis_revolution_rural_repression_memory`、`spr_axis_revolution_high_pressure`。
- 外国依赖/外部压力轴写入 `spr_axis_foreign_autonomous_war`、`spr_axis_foreign_catholic_alarm`、`spr_axis_foreign_rightist_propaganda_gift`、`spr_axis_foreign_isolated_republic`，并初始化 `spr_soviet_influence = 0`，供 P4.3 后续苏联线改写。
- 隐藏汇总只读取并延伸 P3 的 `spr_armed_unions`、`spr_refused_to_arm_unions`、土地/教会/军官/索特洛处理 flags，不清除、不重设这些战前结果。
- 汇总继续调整并钳制 `spr_revolutionary_pressure`、`spr_military_centralization`、`foreign_intervention_pressure`，让后续国民军宣传、五月事件风险、外援压力和战后清算可以读取统一状态。

### P4.2 武装工人和民兵委员会

状态：已完成。

第一轮事件：

- `spr.scw_arm_the_workers`
- `hidden.scw_republic_internal_pressure_tick`

第一轮任务：

- ✅ 中文文本。
- ✅ 选项必须体现短期兵力和长期秩序代价。
- ✅ 读取 P3.4 设置的 `spr_armed_unions` 或 `spr_refused_to_arm_unions`，不得重新设置或覆盖这两个战前 flags。
- ✅ 根据战前是否武装工会，设置 `spr_militia_autonomy`、`spr_militia_contained` 或等价战中 flags，供国民军、CNT 风险和战役新闻读取。
- ✅ 效果必须落到人力、装备库存、民兵单位、州控制风险、POP 激进化或决议机制条，不得写抽象战斗 buff。

实现细节：

- 新增 `spr.scw_arm_the_workers`，内战开始且 P4.1 共和国压力轴初始化后触发。
- `spr.scw_arm_the_workers` 不设置、不清除 `spr_armed_unions` / `spr_refused_to_arm_unions`，只读取这两个 P3.4 战前结果作为选项条件。
- 若承认民兵委员会，设置 `spr_militia_autonomy`、`spr_militia_committees_recognized`，获得短期人力和军需订单，但降低稳定与军队集中度、提高革命压力。
- 若登记民兵并挂入军官名册，设置 `spr_militia_contained`、`spr_militia_registered_by_state`，获得较少人力和军需订单，但提高政府命令链。
- 若此前拒绝武装工会，可继续限制街头武器，设置 `spr_militia_contained`、`spr_workers_arms_restricted`，保住军官体系但激化 CNT/工会怨恨。
- 新增 `hidden.scw_republic_internal_pressure_tick`，由 `spr.scw_arm_the_workers` 选项触发，汇总 `spr_militia_autonomy`、`spr_militia_contained`、`spr_workers_arms_restricted` 为 `spr_cnt_may_days_risk`、`spr_nationalist_propaganda_red_militias`、`spr_state_kept_militia_registers`、`spr_cnt_distrusts_command_chain`、`spr_cnt_anger_over_arms` 等后续可读 flags。
- 效果使用 `AddManpower`、`AddGovernmentOrder`、`ArmyExperience`、`AddStability`、`AddWarSupport`、`spr_revolutionary_pressure`、`spr_military_centralization`，不使用抽象战斗 buff。

暂缓事件：

- `spr.scw_militia_committees`
- `spr.scw_barcelona_barricades`

暂缓说明：完整民兵委员会路线和巴塞罗那革命自治玩法进入共和国完整路线扩展。

### P4.3 黄金储备和苏联援助

状态：已完成。

第一轮事件：

- `spr.scw_gold_reserves`
- `spr.scw_soviet_advisors_arrive`

第一轮任务：

- ✅ 苏联援助不是免费。
- ✅ 接受援助设置 `spr_gold_to_moscow` 和 `spr_soviet_advisors_empowered`。
- ✅ 后续 POUM/CNT 事件读取这些 flags。
- ✅ 效果写成黄金/外汇、进口装备、顾问影响、装备库存、贸易/外交依赖和 POUM/CNT 压力，不写免费工厂或抽象生产效率。

实现细节：

- 新增 `spr.scw_gold_reserves`，在武装工人事件处理后触发。
- 新增 `spr.scw_soviet_advisors_arrive`，在黄金储备事件处理后触发。
- 黄金送往莫斯科设置 `spr_gold_to_moscow`、`sov_gold_received`，并通过苏联轻武器/弹药贸易、军工订单、苏联影响变量体现援助代价。
- 分散黄金或留作国内供应会降低苏联依赖，但获得的装备、贸易和外援效率更弱。
- 苏联顾问事件设置 `spr_soviet_advisors_empowered`、`spr_soviet_advisors_limited` 或 `spr_soviet_advisors_kept_at_distance`，并影响 `spr_soviet_influence`、`spr_military_centralization` 和 POUM/CNT 压力。
- `hidden.scw_republic_internal_pressure_tick` 已读取 `spr_soviet_advisors_empowered`、`spr_soviet_conditions_accepted` 和 `spr_soviet_influence`，写入 `spr_soviet_dependency_visible`、`spr_poum_cnt_suspect_soviet_line`、`spr_foreign_dependency_soviet_line`、`spr_soviet_influence_high`。

暂缓事件：

- `spr.scw_tanks_from_odessa`
- `spr.scw_nkvd_pressure`

暂缓说明：苏联线完整政治条件和 NKVD 压力进入共和国完整路线扩展。

### P4.4 人民军整编

状态：已完成。

第一轮事件：

- `spr.scw_people_army_reform`
- `spr.scw_militias_resist_discipline`

第一轮任务：

- ✅ 整编效果写成民兵单位整编、军官/人力分配、装备补给、军队组织状态或决议机制条变化。
- ✅ 整编激化 CNT 和民兵不满。
- ✅ 设置 `spr_people_army_reformed` 或 `spr_militia_autonomy_preserved`。
- ✅ 后续由 `hidden.scw_republic_internal_pressure_tick` 汇总给五月事件、国民军宣传和共和国崩溃风险读取。

实现细节：

- 新增 `spr.scw_people_army_reform`，在苏联顾问抵达后触发。
- 新增 `spr.scw_militias_resist_discipline`，在人民军整编决策后触发。
- 强制人民军整编设置 `spr_people_army_reformed`，提高军队集中度和军队经验，但提高革命压力与 CNT 不信任。
- 部分整编或保留前线自治会设置 `spr_partial_people_army_reform`、`spr_militia_autonomy_preserved`，保留民兵积极性但削弱中央命令链。
- 民兵抵制事件设置 `spr_militias_forced_into_discipline`、`spr_militias_integrated_by_negotiation` 或 `spr_militias_refused_discipline`，供五月事件、CNT 怨恨和国民军宣传读取。
- 效果使用 `ArmyExperience`、`AddManpower`、`AddGovernmentOrder`、稳定/战争支持和压力轴变量，不使用抽象战斗 buff。

暂缓事件：

- `spr.scw_commissars_at_the_front`

暂缓说明：政治委员制度细化进入共和国完整路线扩展。

### P4.5 共和国政府路线

状态：已完成。

第一轮事件：

- `spr.scw_largo_caballero_government`
- `spr.scw_prieto_warns_the_cabinet`

第一轮任务：

- ✅ 人物必须进入文本。
- ✅ 本轮不做完整共和国政府更替路线，只做关键 flags：苏联依赖、CNT 怨恨、民主合法性和政府权威。
- ✅ 这些 flags 必须能被国民军战后清算、外国索偿和新闻文本读取。

实现细节：

- 新增 `spr.scw_largo_caballero_government`，在民兵抵制纪律事件后触发。
- 新增 `spr.scw_prieto_warns_the_cabinet`，在拉尔戈政府成形后触发。
- 拉尔戈事件通过 `spr_broad_wartime_cabinet`、`spr_democratic_legitimacy_preserved`、`spr_largo_worker_cabinet`、`spr_cnt_expectations_raised`、`spr_security_and_supply_cabinet`、`spr_government_authority_visible` 记录政府权威、工会关系和战时行政方向。
- 普列托事件通过 `spr_democratic_legitimacy_emphasized`、`spr_republican_legality_memory`、`spr_soviet_conditions_accepted`、`spr_conceded_to_cnt`、`cnt_collectivization_recognized` 记录民主合法性、苏联条件和 CNT 妥协。
- `hidden.scw_republic_internal_pressure_tick` 已读取政府、CNT、苏联和人民军相关 flags，供后续国民军战后清算、外国索偿和新闻文本读取。

暂缓事件：

- `spr.scw_negrin_government`

暂缓说明：内格林长期抗战路线、民主合法性完整分歧和共和国胜利后果进入长期扩展。

验收：P4 第一轮完成后，共和国不是空壳，国民军事件和新闻能读取共和国战前/战中选择；完整共和国玩家路线不在本轮验收。

## P5：国民军内战核心路线

本阶段是第一轮内战中可玩内容的重点。目标不是只让国民军赢得战争，而是让玩家在内战过程中塑造国民军：谁掌权、谁被收编、谁被牺牲、外援账单如何累积、占领区靠恐怖还是行政维持、战后会滑向哪一种右翼国家。

### P5.1 国民军压力轴和分流变量

状态：已完成。

压力轴：

- 领导权：军政府合议 vs 佛朗哥独裁。
- 右翼整合：派系共存 vs 强制统一。
- 恐怖统治：军事管制 vs 白色恐怖扩大。
- 外援依赖：自主胜利 vs 德意债务和战后索偿。
- 占领秩序：临时军管 vs 教会/地方行政 vs 长枪党地方委员会。

第一轮任务：

- ✅ 用 flags 或变量表示 `spa_franco_authority`、`spa_falange_power`、`spa_army_loyalty`、`spa_church_influence`、`spa_carlist_anger`、`spa_foreign_dependency`。
- ✅ 每条压力轴至少有 3 个可见事件或隐藏汇总事件。
- ✅ 每个重要选择必须影响 P9.1A、P9.1B、P9.1C 或 P9.1D 的战后路线分流。
- ✅ 效果必须联动当前系统：人力、装备库存、军队补给、港口/铁路、占领区抵抗、POP 满意度/激进化、建筑控制权、贸易、财政债务、外援 flags 和决议机制条。

实现细节：

- 扩展 `spa.scw_junta_forms`，在玩家进入国民军叙事时触发 `hidden.scw_initialize_rebel_state`，并写入领导权初始 flags 与变量变化。
- 新增 `spa.scw_command_table`、`spa.scw_rightist_committees`、`spa.scw_rear_security_orders`、`spa.scw_foreign_aid_accounting`、`spa.scw_local_administration_choice` 五个可见国民军压力轴事件。
- 扩展 `hidden.scw_initialize_rebel_state` 初始化六个国民军路线变量。
- 新增 `hidden.scw_nationalist_pressure_axes_tick` 汇总佛朗哥权威、长枪党权力、军队忠诚、教会影响、卡洛斯派怨恨和德意外援依赖，写入 P9 战后路线候选与风险 flags。
- 已用 `cargo check` 验证 RON 内容和 Rust 工程可加载；仅剩项目既有 warning。

### P5.2 起义军军政府和初始合法性

状态：已完成。

第一轮事件：

- `spa.scw_junta_forms`
- `spa.scw_junta_of_national_defense`
- `spa.scw_cabanellas_warns_the_junta`
- `hidden.scw_initialize_rebel_state`

第一轮任务：

- ✅ `SPA` 生成后立即建立国民军初始状态：首都、军政府、核心州、初始部队、后方补给和政治 flags。
- ✅ 通过卡瓦内利亚斯警告事件明确“给佛朗哥权力不会归还”的风险。
- ✅ 军政府事件必须初始化领导权、军队忠诚、右翼派系和外援依赖机制条。
- ✅ 效果写成指挥权、初始军队组织、军官忠诚、国民军后方稳定、补给通道和玩家决议机制，不写免费工厂或抽象全国加成。

实现细节：

- 扩展 `hidden.scw_initialize_rebel_state`，写入 `spa_rebel_state_initialized`、`spa_capital_burgos_provisional`、`spa_junta_of_national_defense_created`、`spa_core_rebel_zone_secured`、`spa_initial_rebel_divisions_split`、`spa_rear_depots_burgos_seville`、`spa_northern_rail_supply_prioritized`、`spa_port_supply_chain_guarded` 等初始状态 flags。
- 初始化 `spa_rear_supply`、`spa_junta_legitimacy` 两个 P5.2 支撑变量，并补充人力、燃油、军需订单和北方/塞维利亚相关基础设施等级，表现后方补给和初始军队组织。
- 新增 `spa.scw_junta_of_national_defense`，表现国防委员会从叛乱军官会议转为临时参谋政府，提供“军政府制度化”与“前线将领授权”两种代价路径。
- 新增 `spa.scw_cabanellas_warns_the_junta`，明确卡瓦内利亚斯对佛朗哥集中权力的警告，并写入 `spa_cabanellas_warning_heeded` 或 `spa_cabanellas_warning_ignored` 供 P5/P9 后续读取。
- 扩展 `spa.scw_junta_forms`，在两个开局选项后接入 `spa.scw_junta_of_national_defense`，让国民军玩家选边后立即进入军政府合法性链。
- 扩展 `hidden.scw_nationalist_pressure_axes_tick`，汇总军政府合法性、后方补给、卡瓦内利亚斯警告和佛朗哥个人权威风险，写入后续路线权重 flags。
- 已用 `cargo check` 验证 RON 内容和 Rust 工程可加载；仅剩项目既有 warning。

### P5.3 桑胡尔霍、莫拉和佛朗哥权力斗争

状态：已完成。

第一轮事件：

- `spa.scw_sanjurjo_plane_crash`
- `spa.scw_mola_northern_plan`
- `spa.scw_franco_crosses_the_strait`
- `spa.scw_supreme_command_question`
- `spa.scw_franco_and_mola_map_room`
- `spa.scw_mola_dies_in_air_crash`

第一轮任务：

- ✅ 桑胡尔霍死亡制造权力真空。
- ✅ 莫拉代表北方阴谋网络和军政府合议路线。
- ✅ 佛朗哥代表非洲军团、外援谈判和谨慎攫权路线。
- ✅ 最高统帅问题不能是单选奖励，必须在军队忠诚、佛朗哥权威、莫拉派不满、外援效率和战后独裁倾向之间取舍。
- ✅ 效果写成指挥链 flags、军队组织、北方战线补给、非洲军团运输、德意外援依赖和后续 P9 分流权重。

实现细节：

- 新增 `spa.scw_sanjurjo_plane_crash`，用 `KillCharacter("spa_jose_sanjurjo")` 和 `spa_sanjurjo_death_power_vacuum` / `spa_sanjurjo_death_used_for_unity` 表现桑胡尔霍死亡后的权力真空与宣传利用。
- 新增 `spa.scw_mola_northern_plan`，用北方铁路/军区网络、军政府合议记忆、卡洛斯派怨恨和军队忠诚变量表现莫拉路线。
- 新增 `spa.scw_franco_crosses_the_strait`，用非洲军团人力、燃油消耗、德意运输 flags、外援依赖和佛朗哥权威表现海峡运输。
- 新增 `spa.scw_supreme_command_question`，在佛朗哥最高统帅和委员会副署权之间设置取舍，影响军队忠诚、外援依赖、军政府合法性与 P9 佛朗哥路线权重。
- 新增 `spa.scw_franco_and_mola_map_room`，表现佛朗哥和莫拉围绕北方计划、南方推进和外援谈判的合作或排挤。
- 新增 `spa.scw_mola_dies_in_air_crash`，用 `KillCharacter("spa_emilio_mola")`、莫拉遗产保留或佛朗哥接收北方网络，推动 P9 权力分流。
- 调整 `spa.scw_junta_forms`，让国民军开局先进入桑胡尔霍坠机，再进入国防委员会链，权力真空先于军政府制度化出现。
- 扩展 `hidden.scw_nationalist_pressure_axes_tick`，汇总桑胡尔霍权力真空、莫拉北方网络、佛朗哥最高统帅、莫拉遗产和佛朗哥继承北方网络等 P9 权重 flags。
- 已用 `cargo check` 验证 RON 内容和 Rust 工程可加载；仅剩项目既有 warning。

暂缓事件：

- 桑胡尔霍存活低概率路线。
- 莫拉长期存活并压制佛朗哥的完整替代路线。

暂缓说明：第一轮保留替代空间，但不做完整莫拉胜利路线。

### P5.4 非洲军团、海峡运输和殖民兵员

状态：已完成。

第一轮事件：

- `spa.scw_franco_crosses_the_strait`
- `spa.scw_army_of_africa`
- `spa.scw_moroccan_recruitment`
- `spa.scw_alcazar_symbol`

第一轮任务：

- ✅ 非洲军团必须是国民军早期军事优势的来源，也是暴力、殖民兵员承诺和战后摩洛哥问题的来源。
- ✅ 德意空运/海运支援要设置 `spa_german_dependency`、`spa_italian_dependency` 或外援依赖变量。
- ✅ 摩洛哥兵员不能只给人力，必须有军饷、宗教承诺、殖民行政和战后债务记忆。
- ✅ 阿尔卡萨尔解围作为宣传神话，同时影响佛朗哥权威和国民军叙事合法性。
- ✅ 效果写成单位转移、运输/港口、装备库存、人力、军饷财政、外援 flags、殖民兵员忠诚和战后摩洛哥钩子。

实现细节：新增 `spa.scw_army_of_africa`、`spa.scw_moroccan_recruitment`、`spa.scw_alcazar_symbol`，并扩展既有 `spa.scw_franco_crosses_the_strait` 后续读取链。

### P5.5 恐怖宣传、占领区秩序和后方治理

状态：已完成。

第一轮事件：

- `spa.scw_radio_seville`
- `spa.scw_badajoz_aftermath`
- `spa.scw_order_in_occupied_zones`
- `spa.scw_yague_after_badajoz`
- `hidden.scw_nationalist_occupation_order_tick`

第一轮任务：

- ✅ 描述要有压迫感，但避免猎奇。
- ✅ 奎波电台必须体现恐怖宣传的短期动员和中央权威风险。
- ✅ 巴达霍斯后果必须影响白色恐怖、国际舆论、占领区抵抗、军队纪律和战后清算路线。
- ✅ 占领区秩序要提供至少三种选择：恐怖镇压、军管行政、教会/地方精英合作、长枪党地方委员会。
- ✅ 效果写成抵抗、POP 激进化、地方稳定、军事警备消耗、补给安全、教会影响、长枪党权力和 `spa_white_terror_expanded`。

实现细节：新增 `spa.scw_radio_seville`、`spa.scw_badajoz_aftermath`、`spa.scw_yague_after_badajoz`、`spa.scw_order_in_occupied_zones` 和 `hidden.scw_nationalist_occupation_order_tick`。

### P5.6 长枪党继承危机和蓝衫权力

状态：已完成。

第一轮事件：

- `spa.scw_jose_antonio_absent`
- `spa.scw_falange_martyrdom`
- `spa.scw_hedilla_succession_crisis`
- `spa.scw_old_shirts_and_new_men`
- `spa.scw_hedilla_meets_serrano_suner`
- `spa.scw_falange_seizes_secretariat`

第一轮任务：

- ✅ 何塞·安东尼奥必须作为“缺席者”而不是普通人物使用。
- ✅ 赫迪利亚危机决定长枪党是被佛朗哥收编、保留正统性，还是获得秘书处优势。
- ✅ 老衫派和投机者冲突必须影响 `spa_falange_power`、行政效率、地方控制和后续长枪党战后路线。
- ✅ 效果写成党组织力、地方任命、宣传、工团制度化进度、军队不满和 P9.1B/P9.1C 分流权重。

实现细节：新增何塞·安东尼奥缺席/殉道、赫迪利亚继承危机、旧衫派与新人、赫迪利亚-塞拉诺会面、长枪党秘书处事件链。

### P5.7 卡洛斯派、教会和传统右翼

状态：已完成。

第一轮事件：

- `spa.scw_carlist_demands`
- `spa.scw_requetes_at_the_front`
- `spa.scw_church_crusade`
- `spa.scw_ceda_and_alfonsists`
- `spa.scw_varela_and_fal_conde`

第一轮任务：

- ✅ 卡洛斯派要提供北方兵力、宗教合法性和王位承诺压力。
- ✅ 教会十字军叙事要影响国民天主教、教育/社会秩序、国际形象和战后 P9.1A/P9.1C 分流。
- ✅ CEDA、王党和旧保守精英要作为行政能力来源，也会抵制长枪党革命。
- ✅ 效果写成北方人力/单位、教会影响、地方稳定、教育/社会控制、卡洛斯派怨恨、王党不满和军队忠诚。

实现细节：新增 `spa.scw_carlist_demands`、`spa.scw_requetes_at_the_front`、`spa.scw_church_crusade`、`spa.scw_ceda_and_alfonsists`、`spa.scw_varela_and_fal_conde`。

### P5.8 德意外援依赖和战争债务

状态：已完成。

第一轮事件：

- `spa.scw_german_dependency`
- `spa.scw_italian_dependency`
- `ger.scw_condor_legion`
- `ita.scw_send_ctv`
- `por.scw_nationalist_supply_routes`

第一轮任务：

- ✅ 德意援助必须是交易，不是免费装备。
- ✅ 德国援助偏空军、运输、顾问、机床/装备输入和战后矿产/基地/外交期待。
- ✅ 意大利援助偏 CTV、地中海威望、宣传压力和战后外交索偿。
- ✅ 葡萄牙通道影响后方补给、边境运输和外交安全。
- ✅ 效果写成进口装备/燃料/机器、装备库存、顾问 flags、运输/港口/铁路、财政债务、贸易承诺和 `spa_foreign_dependency`。

实现细节：新增 `spa.scw_german_dependency`、`spa.scw_italian_dependency`、`ger.scw_condor_legion`、`ita.scw_send_ctv`、`por.scw_nationalist_supply_routes`。

### P5.9 统一法令和右翼强制整合

状态：已完成。

第一轮事件：

- `spa.scw_unification_decree_drafted`
- `spa.scw_unification_decree`
- `spa.scw_hedilla_arrested`
- `spa.scw_hedilla_compromise`
- `spa.scw_caudillo_or_revolution`

第一轮任务：

- ✅ 统一法令是国民军内战路线的核心分叉，不能只是设置一个 flag。
- ✅ 佛朗哥权威高时可逮捕赫迪利亚，锁定或强化佛朗哥路线。
- ✅ 长枪党权力高时可达成妥协或让秘书处坐大，开启 P9.1B/P9.1C 候选。
- ✅ 卡洛斯派和教会必须对统一法令有反应。
- ✅ 效果写成党组织、地方任命、军队忠诚、卡洛斯派怨恨、教会妥协、占领区秩序和战后分流权重。

实现细节：新增 `spa.scw_unification_decree_drafted`、`spa.scw_unification_decree`、`spa.scw_hedilla_arrested`、`spa.scw_hedilla_compromise`、`spa.scw_caudillo_or_revolution`。

### P5.10 佛朗哥成为考迪罗和战后路线候选

状态：已完成。

第一轮事件：

- `spa.scw_franco_caudillo`
- `spa.scw_kindelan_questions_the_caudillo`
- `hidden.scw_nationalist_internal_pressure_tick`

第一轮任务：

- ✅ 读取前置 flags。
- ✅ 如果佛朗哥权威不足，给出更高代价。
- ✅ 如果长枪党或卡洛斯派被激怒，后续结局读取。
- ✅ 金德兰/王党疑虑必须表现出“佛朗哥是否会归还王位”的问题。
- ✅ `hidden.scw_nationalist_internal_pressure_tick` 周期汇总佛朗哥权威、长枪党、军队、教会、卡洛斯派、外援依赖和占领秩序。
- ✅ 战后不能直接固定为佛朗哥路线，必须按 P5 选择给出 P9.1A、P9.1B、P9.1C 的候选权重。

实现细节：新增 `spa.scw_franco_caudillo`、`spa.scw_kindelan_questions_the_caudillo` 和 `hidden.scw_nationalist_internal_pressure_tick`。

### P5.11 国民军内战第一轮暂缓内容

状态：已完成。

暂缓事件：

- `spa.scw_jose_antonio_letters_from_prison`
- `spa.scw_franco_receives_news_of_jose_antonio`
- `spa.scw_quiepo_broadcasts_without_permission` 的完整地方军阀线。
- 莫拉长期存活路线。
- 卡洛斯派单独胜利路线。
- 国民军内战期完整经济重建线。

暂缓说明：第一轮必须优先完成国民军主线和三条战后分流前置，不扩散到所有替代胜利者。

实现细节：暂缓清单保留不落地，P5 第一轮已聚焦国民军主线、三条战后分流前置和 P9 可读取记忆，不扩散到莫拉长期存活、卡洛斯派单独胜利和完整经济重建线。

验收：✅ P5 完成后，国民军玩家能感到自己在“赢得国民军”，而不只是赢得内战；P9.1A-D 的路线分流必须能追溯到 P5 的内战选择。

## P6：CNT/POUM 和五月事件最小支撑

本阶段第一轮只做 CNT/POUM 作为共和国压力、五月事件风险和国民军宣传/清算对象的最小内容。完整 CNT 可玩路线、CNT 胜利和革命战后路线进入长期扩展。

### P6.1 移除固定日期必然起义设计

状态：已完成。

目标：CNT 起义由政治后果触发。

任务：

- ✅ 审查 `spanish_anarchist_uprising.ron` 固定 1937-05-03 触发。
- ✅ 增加共和国存在、内战仍在、CNT 风险 flags 等条件。
- ✅ 避免共和国已灭亡后 CNT 仍起义。

实现细节：

- `spanish_anarchist_uprising.ron` 当前保持空局势，不再有固定 1937-05-03 必然起义。
- 新增 `hidden.scw_cnt_uprising_check` 作为 CNT/POUM 压力汇总器，由 CNT、POUM、电话局和五月事件相关选择触发。
- 五月事件由 `spr_cnt_may_days_risk`、`poum_outlawed`、`spr_soviet_influence_high`、`spr_militias_forced_into_discipline` 或 `spr_revolutionary_pressure >= 16` 等政治后果共同推动。
- `cnt_uprising_prevented`、`barcelona_may_days_triggered` 和 `spr_barcelona_telephone_exchange_crisis` 防止重复触发或在已缓和后继续起义。

### P6.2 工厂集体化和民兵自治

状态：已完成。

第一轮事件：

- `cnt.scw_collectivized_factories`
- `cnt.scw_militias_refuse_orders`

第一轮任务：

- ✅ 中文文本。
- ✅ 选项体现革命收益和国家秩序代价。
- ✅ 集体化效果写成军工订单、工资/配给叙事、委员会合法性、地方控制和共和国权威变化；当前项目暂无 POP/ownership 直接 effect，先落到 flags、稳定、战争支持、军工订单和变量。
- ✅ 民兵自治效果给 P6.4 五月事件风险和国民军宣传/战后清算文本提供 flags。

实现细节：

- 扩展 `cnt.scw_collectivized_factories`，不再是骨架事件，改为在 `spr.scw_prieto_warns_the_cabinet` 后由 `SPR` 触发。
- 集体化路线设置 `cnt_collectivization_recognized`、`cnt_factory_committees_legalized`，增加短期军工订单和战争支持，但降低稳定、提高革命压力、削弱中央集权。
- 共和国监督路线设置 `cnt_factory_committees_supervised`、`spr_government_authority_visible`，提高中央集权但消耗政治资本并留下 CNT 怨恨。
- 新增 `cnt.scw_militias_refuse_orders`，读取人民军整编、强制纪律或民兵自治状态，写入 `cnt_refused_integration`、`cnt_militias_integrated`、`cnt_militia_autonomy_bargain` 等 flags。
- `cnt_refused_integration` 会被 `hidden.scw_cnt_uprising_check` 汇总为 `spr_cnt_may_days_risk` 和 `spr_nationalist_propaganda_red_militias`，供五月事件、国民军宣传和战后清算读取。

暂缓事件：

- `cnt.scw_rural_communes`

暂缓说明：农村公社和 CNT 完整经济玩法进入 CNT 完整路线扩展。

### P6.3 POUM 命运

状态：已完成。

第一轮事件：

- `spr.scw_poum_accused`
- `spr.scw_poum_trial`

第一轮任务：

- ✅ 读取 `spr_soviet_advisors_empowered`、`spr_soviet_conditions_accepted` 和 `spr_soviet_influence`。
- ✅ 保护 POUM 会激怒苏联线。
- ✅ 镇压 POUM 会激怒 CNT 线。
- ✅ 设置 `poum_protected` 或 `poum_outlawed`。
- ✅ 后果被 `hidden.scw_cnt_uprising_check`、国民军宣传 flags 和共和国胜利长期扩展钩子读取。

实现细节：

- 新增 `spr.scw_poum_accused`，在苏联顾问抵达且苏联影响足够后触发。
- 保护 POUM 设置 `poum_protected`、`spr_republican_legality_memory`，降低苏联影响并降低苏联好感。
- 宣布 POUM 非法设置 `poum_outlawed`、`spr_cracked_down_on_poum`，换取苏联好感和短期军工订单，但提高革命压力。
- 公开调查设置 `spr_poum_public_inquiry`，用政治资本换取拖延空间，但仍提高苏联影响和革命压力。
- 新增 `spr.scw_poum_trial`，让公开审理、快速定罪或解除武装三种处理方式留下 `spr_poum_trial_public`、`spr_poum_trial_political_sentence`、`spr_poum_disarmed_but_not_destroyed` 等后续记忆。

暂缓事件：

- `spr.scw_andres_nin_disappears`

暂缓说明：安德烈斯·宁失踪和审讯细节进入共和国/CNT 完整路线扩展。

### P6.4 巴塞罗那五月事件

状态：已完成。

第一轮事件：

- `spr.scw_barcelona_telephone_exchange`
- `spr.scw_may_days`
- `hidden.scw_cnt_uprising_check`

第一轮任务：

- ✅ 不是固定触发。
- ✅ 由 CNT 怨恨、苏联影响、POUM 处理、人民军整编共同决定。
- ✅ 只做“共和国危机/国民军镜像”的五月事件结果：设置 `barcelona_may_days_triggered`、稳定/人力/激进化变量变化和国民军宣传钩子；当前项目暂无 POP 激进化直接 effect，先落到 `spr_revolutionary_pressure` 与 flags。

实现细节：

- 新增 `spr.scw_barcelona_telephone_exchange`，由 `hidden.scw_cnt_uprising_check` 在风险达标时触发。
- 电话局事件可选择强攻、妥协或退让。强攻会触发 `spr.scw_may_days`；妥协或退让设置 `cnt_uprising_prevented`，但分别付出政治资本、中央权威或战争支持代价。
- 新增 `spr.scw_may_days`，设置 `barcelona_may_days_triggered`，并提供武力镇压、工会停火或归咎 POUM/地方委员会三种后果。
- 五月事件写入 `spr_may_days_suppressed_by_force`、`spr_suppressed_cnt`、`spr_may_days_negotiated_ceasefire`、`spr_may_days_blame_poum_and_committees`、`spr_nationalist_propaganda_red_purge` 等后续可读 flags。
- 五月事件效果使用稳定、战争支持、人力、苏联关系、`spr_military_centralization`、`spr_revolutionary_pressure` 和 `spr_soviet_influence`，不使用抽象战斗 buff。

暂缓事件：

- `cnt.scw_break_with_the_republic`

暂缓说明：CNT 公开决裂、独立起义和 CNT 可玩路线进入长期扩展。

验收：✅ P6 第一轮完成后，五月事件不是固定日期脚本，而是由共和国状态触发的压力后果；CNT 完整路线不在本轮验收。

## P7：国际干涉链

第一轮国际干涉优先服务国民军内战和战后路线。德国、意大利、葡萄牙和英国直布罗陀/地中海压力优先实现；苏联、法国、墨西哥只实现共和国背景和外援压力所需最小事件。

### P7.1 德国干涉

事件：

- `ger.scw_condor_legion`
- `ger.scw_air_war_laboratory`
- `news.scw_guernica_bombed`
- `ger.scw_condor_legion_returns`

要求：

- 中文西班牙新闻。
- 德国援助换取经验、影响和战后期待。

### P7.2 意大利干涉

事件：

- `ita.scw_send_ctv`
- `ita.scw_mediterranean_prestige`
- `ita.scw_guadalajara_humiliation`
- `ita.scw_expand_or_withdraw`

要求：

- 意大利有威望收益和失败风险。

### P7.3 苏联和墨西哥援助

第一轮事件：

- `sov.scw_advisors_to_spain`
- `sov.scw_gold_in_moscow`
- `sov.scw_political_conditions`

第一轮任务：

- 苏联援助与共和国政治路线绑定。
- 只做到共和国背景、POUM/CNT 压力和国民军反共宣传可读取。
- 效果写成黄金/外汇、进口装备、顾问影响、外交依赖和政治条件。

暂缓事件：

- `mex.scw_aid_the_republic`

暂缓说明：墨西哥援助作为政治象征保留到共和国完整路线或国际干涉扩展。

### P7.4 英法不干涉和葡萄牙通道

第一轮事件：

- `eng.scw_non_intervention_committee`
- `por.scw_nationalist_supply_routes`
- `news.scw_foreign_volunteers_questioned`

第一轮任务：

- 英国不干涉必须影响国际干涉压力、直布罗陀警惕和国民军战后外交。
- 葡萄牙支持影响国民军后方。
- 葡萄牙通道效果写成补给通道、贸易/运输、边境后方稳定和国民军外援依赖。

暂缓事件：

- `fra.scw_border_question`

暂缓说明：法国边境开放/关闭对共和国补给的完整玩法进入共和国完整路线扩展；第一轮可只保留一个 flag 或新闻引用。

验收：P7 完成后，外国干涉是交易和压力，不是免费装备。

## P8：战役、新闻和隐藏触发

### P8.1 建立战役 hidden triggers

目标：用隐藏事件监听关键州控制。

任务：

- 马德里。
- 巴塞罗那。
- 毕尔巴鄂。
- 桑坦德。
- 阿斯图里亚斯。
- 特鲁埃尔。
- 埃布罗。
- 加泰罗尼亚。

验收：关键地图变化能触发新闻和国内事件。

### P8.2 马德里战役链

第一轮事件：

- `news.scw_madrid_under_siege`
- `news.scw_madrid_falls`
- `hidden.scw_madrid_status_router`

第一轮任务：

- 马德里不是单一陷落新闻。
- 国民军逼近、围城和陷落必须触发国民军权威、共和国崩溃风险、国际反应和战后清算记忆。
- 效果写成首都控制、补给/铁路、POP 难民、士气/稳定、国民军宣传和隐藏 flags。

暂缓事件：

- `spr.scw_no_pasaran`
- `spr.scw_government_to_valencia`

暂缓说明：共和国玩家视角的马德里防御和政府迁都路线进入共和国完整路线扩展。

### P8.3 北方战役链

第一轮事件：

- `news.scw_guernica_bombed`
- `news.scw_bilbao_falls`
- `news.scw_northern_front_collapses`
- `hidden.scw_northern_industry_router`

第一轮任务：

- 战役影响必须落到巴斯克矿区/钢铁/港口/铁路控制、国民军外援依赖、德国秃鹰军团舆论风险和共和国北方崩溃 flags。

暂缓事件：

- `spr.scw_basque_autonomy`

暂缓说明：巴斯克自治作为共和国政治路线内容进入共和国完整路线扩展。

### P8.4 埃布罗和加泰罗尼亚链

第一轮事件：

- `news.scw_ebro_battle`
- `news.scw_barcelona_falls`
- `hidden.scw_catalonia_collapse_router`

第一轮任务：

- 埃布罗是共和国最后赌博。
- 加泰罗尼亚崩溃必须触发难民、工业/港口控制、共和国终局、国民军胜利倒计时和国际新闻。
- 效果写成州控制、建筑/港口/铁路、POP 难民、补给和隐藏 flags。

暂缓事件：

- `spr.scw_ebro_offensive`
- `spr.scw_international_brigades_last_stand`

暂缓说明：共和国玩家视角的埃布罗赌博和国际纵队撤离进入共和国完整路线扩展。

验收：P8 第一轮完成后，关键战役能推动国民军路线、共和国崩溃状态、地图控制、建筑/补给变化和国际新闻；共和国专属战役玩法暂缓。

## P9：结局和战后清算

第一轮只实现国民军胜利和国民军战后路线。共和国胜利、CNT 胜利和对应战后路线只保留新闻、清理、防卡局势和长期扩展钩子。

### P9.1 国民军胜利结局

事件：

- `spa.scw_victory`
- `spa.scw_repression_after_victory`
- `spa.scw_church_and_state`
- `spa.scw_axis_or_neutrality`
- `ger.scw_claims_after_franco_victory`
- `ita.scw_claims_after_franco_victory`

要求：

- 读取德国/意大利依赖 flags。
- 读取长枪党、卡洛斯派、白色恐怖 flags。
- 触发 `hidden.scw_route_nationalist_victory`，按佛朗哥权威、长枪党权力、赫迪利亚处理、军队忠诚、教会影响和卡洛斯派怨恨分流。
- 胜利效果必须包含清理战争状态、迁都/统一核心、抵抗和占领秩序、财政/债务、外援索偿、军队复员或继续动员，不得只给稳定加成。

### P9.1A 佛朗哥胜利战后路线

第一轮事件：

- `spa.franco_victory_the_caesar_of_burgos`
- `spa.franco_cabinet_of_families`
- `spa.franco_law_of_political_responsibilities`
- `spa.franco_national_catholic_state`
- `spa.franco_tame_the_falange`
- `spa.franco_autarky_and_hunger`
- `spa.franco_axis_temptation`
- `spa.franco_monarchy_without_a_king`
- `spa.franco_ini_foundation`
- `spa.franco_black_market_and_ration_cards`

第一轮任务：

- 确立佛朗哥路线的核心玩法：个人仲裁、政治家族平衡、国民天主教、驯服长枪党、战后饥荒和自给经济。
- 事件效果必须落到政治家族 flags、教会/长枪党/军队权力、配给和黑市、财政压力、建筑投资、贸易封锁或外交孤立。
- 后续可以接技术官僚和冷战解冻，但不阻塞第一轮。

暂缓事件：

- `spa.franco_hendaye_meeting`
- `spa.franco_blue_division`
- `spa.franco_postwar_isolation`
- `spa.franco_concordat_and_bases`
- `spa.franco_technocrats_enter`
- `spa.franco_stabilization_plan`
- `spa.franco_spanish_miracle`
- `spa.franco_successor_question`

暂缓说明：二战外交后半段、冷战解冻、技术官僚和继承问题可在国民军第一轮后继续扩展。

### P9.1B 长枪党胜利战后路线

第一轮事件：

- `spa.falange_victory_blue_dawn`
- `spa.falange_hedilla_or_junta`
- `spa.falange_council_of_the_revolution`
- `spa.falange_twenty_six_points`
- `spa.falange_national_syndicalist_revolution`
- `spa.falange_vertical_unions`
- `spa.falange_church_subordinated`
- `spa.falange_axis_alignment`
- `spa.falange_syndicalist_planning_board`
- `spa.falange_battle_for_bread`

第一轮任务：

- 确立长枪党路线的核心玩法：党国压倒军政府、国家工团主义、垂直工会、教会冲突、亲轴倾向和粮食危机。
- 事件效果必须落到党权力、军队忠诚、工团制度化、工资/配给、建筑 ownership、财政和 POP 满意度/激进化。
- 长枪党路线不能只给强力军政加成，必须有行政混乱、保守派反扑和民生压力。

暂缓事件：

- `spa.falange_crush_carlism`
- `spa.falange_take_gibraltar_question`
- `spa.falange_after_axis_defeat`
- `spa.falange_revolution_besieged`
- `spa.falange_government_of_blue_victory`

暂缓说明：路线尾声、轴心失败后伪装和完整政府班底可后续补充。

### P9.1C 长枪党架空佛朗哥与制度化国家工团主义路线

第一轮事件：

- `spa.syndicalist_shadow_the_caudillo_and_the_secretariat`
- `spa.syndicalist_dual_power_settlement`
- `spa.syndicalist_control_the_movement_council`
- `spa.syndicalist_caudillo_as_symbol`
- `spa.syndicalist_vertical_syndicates_charter`
- `spa.syndicalist_church_compact`
- `spa.syndicalist_law_of_national_organization`
- `spa.syndicalist_cortes_of_corporations`
- `spa.syndicalist_three_year_reconstruction_plan`
- `spa.syndicalist_wage_tables_and_price_boards`

第一轮任务：

- 确立架空路线的核心玩法：佛朗哥保留象征权威，秘书处、工团和制度程序逐步吃掉个人独裁。
- 事件效果必须落到 `spa_syndicalist_institutionalization`、秘书处权力、军队忠诚、教会妥协、工资表、价格委员会、建造队列和财政压力。
- 该路线要体现“制度化”而不是单纯长枪党夺权。

暂缓事件：

- `spa.syndicalist_postwar_rebranding`
- `spa.syndicalist_technocrats_absorbed`
- `spa.syndicalist_institutional_state`
- `spa.syndicalist_when_franco_fades`
- `spa.syndicalist_government_of_the_hidden_machine`

暂缓说明：长期制度成熟、技术官僚吸收和佛朗哥退场后的继承危机后续扩展。

### P9.1D 国民军战后外交、债务和轴心重建协定

第一轮事件：

- `ger.scw_claims_after_nationalist_victory`
- `ita.scw_claims_after_nationalist_victory`
- `spa.postwar_tungsten_and_debt`
- `spa.postwar_portuguese_pact`
- `eng.postwar_spain_and_gibraltar`
- `ger.spain_offer_axis_reconstruction_pact`
- `spa.axis_reconstruction_pact_arrives`
- `spa.german_aid_first_shipments`
- `spa.german_credits_for_reconstruction`
- `spa.retooling_the_armories`
- `spa.recovery_of_the_workshops`

第一轮任务：

- 外援必须有账单：德意索偿、矿产贸易、港口/铁路优先权、基地压力、财政债务和主权代价。
- 德国援助效果必须写成进口机器/煤/药品、财政现金流、建造队列、铁路/港口修复、军工建筑 PM 和装备库存变化。
- 英国事件必须连接直布罗陀、海峡安全、封锁/贸易压力和西班牙亲轴风险。

暂缓事件：

- `spa.operation_felix_at_last`
- `news.gibraltar_falls_to_spain`
- `news.axis_armies_enter_cairo`
- `eng.request_the_glorious_peace`
- `spa.claims_in_morocco_and_algeria`
- `spa.treaty_of_rabat`
- `spa.governorate_of_new_africa`

暂缓说明：轴心胜利、地中海总攻、北非战利品和帝国扩张是国民军战后扩展，不阻塞第一轮国民军战后路线。

### P9.2 共和国胜利结局

状态：长期扩展，本轮暂缓。

事件：

- `spr.scw_victory`
- `spr.scw_trial_of_the_generals`
- `spr.scw_restore_constitution`
- `spr.scw_soviet_shadow`
- `spr.scw_cnt_question_after_victory`

要求：

- 读取苏联援助、POUM、CNT、民主合法性 flags。
- 共和国胜利不一定代表民主共和国完好无损。

### P9.3 CNT 特殊结局

状态：长期扩展，本轮暂缓。

事件：

- `cnt.scw_victory`
- `cnt.scw_iberian_commune`
- `cnt.scw_international_isolation`
- `cnt.scw_revolution_or_survival`

要求：

- CNT 胜利是特殊路线，不是默认目标。

### P9.4 清理临时状态

任务：

- 清理临时战争 ideas。
- 清理临时前线 flags。
- 统一核心、控制权和首都。
- 移除已灭亡派系单位。
- 触发外国志愿军撤回。

验收：内战结束后西班牙进入稳定战后状态。

## P10：中文写作质量和风格验收

### P10.1 中文事件文本审查

目标：避免“AI 味”和空洞描述。

审查规则：

- 是否有具体场景。
- 是否有人物或机构。
- 是否有政治压力。
- 是否有选择代价。
- 是否有后续记忆。
- 是否符合中文表达习惯。

### P10.2 KR 式合理性审查

目标：替代分支必须讲得通。

审查规则：

- 分支是否符合历史人物利益。
- 派系是否有自己的政治目标。
- 外国干涉是否有回报诉求。
- 事件是否能与世界局势互文。

### P10.3 TNO 式氛围审查

目标：重大事件要有压迫感和制度压力。

审查规则：

- 文本是否只是在播报，还是让玩家感到局势逼迫。
- 是否体现战争对社会、城市、家庭、政府、军队的消耗。
- 是否避免纯爽文式胜利。
- 是否把胜利写成带代价的结果。

### P10.4 事件效果审查

目标：每个重要事件有机制意义。

审查规则：

- 至少一个选项设置 flag 或改变数值。
- 至少一个后续事件读取该 flag。
- 没有纯奖励三选一。
- 新闻事件之外，不允许只有空 effects。

## P11：长期扩展

本节收纳第一轮主动暂缓的完整路线，不应阻塞“战前完整 + 国民军内战/战后完整”的交付。

### P11.0 暂缓的西班牙路线

目标：在国民军第一轮完成后，再补齐其他派系。

任务：

- 完整共和国战中路线：共和国权威、苏联影响、人民军整编、民主合法性、内格林/卡瓦列罗路线差异。
- 共和国胜利战后路线：审判叛军、恢复宪政、苏联阴影、CNT 问题、军队重建。
- 完整 CNT/POUM 路线：革命委员会、集体化、巴塞罗那五月事件、共和国决裂、CNT 胜利。
- 完整外国干涉国玩法：德国、意大利、苏联、英法、葡萄牙、墨西哥的国家内政和外交选择。
- 卡洛斯派或其他国民军内部特殊胜利路线，如有需要在国民军三条主线完成后再评估。

### P11.1 西班牙专属国策树

目标：共和国、国民军、CNT 拥有自己的国策体验。

任务：

- 共和国战时树。
- 国民军整合树。
- CNT 革命树。
- 战后重建树。

### P11.2 国家专属决议小游戏扩展

目标：每个主要可玩国家都拥有不同的决议小游戏和风味机制，而不是共用一套通用按钮列表。

任务：

- 西班牙共和国：民兵整编、苏联援助、CNT 谈判、马德里防御。
- 国民军西班牙：佛朗哥权威、右翼派系统合、外援依赖、占领区秩序。
- CNT：集体化、革命委员会、巴塞罗那紧张度、革命输出。
- 德国：重整军备、经济压力、外部干涉实验场。
- 意大利：地中海威望、军队现代化、殖民战争压力。
- 苏联：清洗、工业化、安全区、国际援助。
- 英国：帝国稳定、再武装、殖民压力、议会舆论。
- 法国：政治分裂、边境安全、工潮、外交妥协。
- 日本：中国战线、军部派系、资源南进/北进。
- 中国：抗战动员、地方军阀、工业迁移、统一战线。

设计要求：

- 每国至少一个独特机制条或小游戏。
- 每国决议面板文案必须符合该国政治氛围。
- 决议小游戏必须和事件、flags、局势联动。
- 不允许所有国家只有换皮的“花 PP 得 buff”。

### P11.3 人物系统长期增强

状态：第一轮最小人物效果已前移到 P2.6。本节只保留后续增强。

目标：在第一轮能记录人物命运的基础上，增强人物 UI、顾问、肖像和 tooltip。

任务：

- 人物 portrait 实际显示。
- 人物 tooltip 展示状态、派系、历史说明和当前可用性。
- 顾问、将领、部长和国家领导人使用统一人物状态来源。
- 人物状态变更能在国家信息、事件、决议面板中一致显示。

### P11.4 变量系统长期增强

状态：第一轮轻量变量系统已前移到 P2.5。本节只保留后续增强。

目标：把第一轮变量系统扩展为通用内容工具，服务其他国家和长期路线。

任务：

- 支持变量衰减、周期增长、上下限模板和批量调试面板。
- 支持跨国家变量比较，例如外国影响、债务压力、联盟信任。
- 支持变量历史趋势，用于 UI 图表和事件 tooltip。
- 支持变量驱动的 AI 权重和决议可见性。

### P11.5 事件呈现长期增强

状态：第一轮事件预览和队列稳定性已前移到 P2.7。本节只保留后续增强。

目标：靠图片、音效、版式和历史日志提升事件氛围。

任务：

- 事件图片实际显示。
- 重要事件音效。
- 国家事件、新闻事件、隐藏日志、结局事件使用不同视觉版式。
- 建立事件历史日志，允许玩家回看关键选择。

### P11.6 后续事件工具增强

目标：补齐第一轮不阻塞内容制作、但长期有用的事件工具。

任务：

- 事件链调试视图：显示触发条件、失败原因、已设置 flags 和变量。
- 事件覆盖率检查：列出未被任何条件触发的孤儿事件。
- 内容 lint：检查中文字段、空效果、未使用 flags、未引用变量和重复事件 id。
- 批量模拟：从 1936 开局自动跑到内战结束，统计路线分流和事件触发率。

## 不做事项

第一阶段不要做：

- 一次性写 200 个事件。
- 第一轮同时做完整共和国、完整 CNT 和所有外国干涉国可玩路线。
- 先做完整国策树再修基础玩法。
- 把所有内容继续堆进 `news_events.ron`。
- 固定日期强行触发 CNT 起义。
- 只有长文本但没有后果的事件。
- 只用新闻事件替代国家事件。
- 英文事件标题、英文描述、英文选项。
- 继续把决议藏在政治面板里。
- 所有国家共用一套无差异决议按钮。
- 使用 HOI4 vanilla 式“消费品工厂/军用工厂/生产效率百分比”作为主要事件效果。
- 写无法被当前经济、POP、建筑生产、市场、财政、军需或决议系统解释的抽象 buff。

第一阶段必须保证：

- `SPR` 能开局游玩。
- 内战爆发能选择共和国或国民军。
- 所有西班牙新增事件文本为中文。
- 事件有选择、有代价、有 flags、有后续回响。
- 战前共和国危机完整，且影响内战初始局势。
- 国民军内战和战后路线完整，至少包含佛朗哥、长枪党、架空佛朗哥三条战后分流。
- 共和国和 CNT 在第一轮至少作为压力系统、背景事件和国民军镜像存在。
- 外国援助有交易和后果，优先能被国民军战后索偿和依赖路线读取。
- 战役事件能引发政治变化。
- 决议系统有独立面板。
- P2 后必须补齐第一轮轻量变量系统、人物命运效果和事件后果预览，不能等到 P11。
- 事件效果必须落到当前项目系统：flags/变量、决议机制条、POP、建筑、生产方式、商品供需、库存、贸易、财政、军工订单、补给、占领秩序、抵抗或外交。
- 西班牙至少有国民军决议机制第一版；共和国和 CNT 决议机制可先保留草案。

## 第一轮推荐实施顺序

严格顺序：

1. P-1.1 删除旧西班牙局势定义。
2. P-1.2 删除旧西班牙新闻事件。
3. P-1.3 删除旧德国西班牙干涉触发引用。
4. P-1.4 建立临时空窗保护。
5. P0.1 审计玩家国家字段。
6. P0.2 新增统一玩家切换 helper。
7. P0.3 开放西班牙共和国开局游玩。
8. P0.4 新增切换玩家国家效果。
9. P0.5 修正事件暂停和新闻行为。
10. P0.6 建立中文事件校验清单。
11. P0.7 决议系统从政治面板解绑。
12. P0.8 建立通用决议面板框架。
13. P0.9 建立 KR/TNO 式决议小游戏接口。
14. P0.10 西班牙内战专属决议机制草案。
15. P1.1 新建西班牙事件库文件。
16. P1.2 更新 1936 manifest。
17. P1.3 迁移现有西班牙新闻并中文化。
18. P1.4 建立事件命名规范。
19. P1.5 建立第一批 flags。
20. P2.1 重写内战爆发 on_start 顺序。
21. P2.2 新增中文阵营选择事件。
22. P2.3 重写中文世界新闻。
23. P2.4 新增双方开局国家事件。
24. P2.5 补齐第一轮事件变量系统。
25. P2.6 补齐第一轮人物和领导人效果。
26. P2.7 补齐第一轮事件呈现和效果预览。
27. P3.1 到 P3.4 完成战前危机链。
28. P4.1 到 P4.5 完成共和国战中最小支撑路线。
29. P5.1 到 P5.11 完成国民军内战核心路线第一版。
30. P6.1 到 P6.4 完成 CNT/POUM 最小支撑和五月事件风险链。
31. P7.1、P7.2、P7.4 优先完成德意葡英相关干涉，P7.3 只做共和国背景最小事件。
32. P8.1 到 P8.4 完成战役新闻第一版。
33. ✅ P9.1 完成国民军胜利结局和 `hidden.scw_route_nationalist_victory` 分流。
34. P9.1A 完成佛朗哥胜利战后路线第一轮。
35. P9.1B 完成长枪党胜利战后路线第一轮。
36. P9.1C 完成长枪党架空佛朗哥与制度化国家工团主义路线第一轮。
37. P9.1D 完成国民军战后外交、债务和轴心重建协定第一轮。
38. P9.4 完成清理；P9.2、P9.3 暂缓。
39. P10.1 到 P10.4 做中文、KR、TNO、机制审查。

第一轮到 P10 完成后，西班牙内战应达到“战前完整、国民军可玩、有中文叙事、有选择、有后果、有系统联动、有战后分流”的标准。
