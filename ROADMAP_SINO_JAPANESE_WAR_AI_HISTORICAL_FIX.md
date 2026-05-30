# 1937 中日战争 AI 历史表现修正路线图

## 目标

让 1937 年全面抗战爆发后，日本 AI 能在华北、山东、上海、南京方向表现出符合历史的初期推进能力，同时避免日本过早横扫中国。

期望节奏：

- 1937 年 7 月：卢沟桥事变后华北开战。
- 1937 年 8-10 月：日本能推进河北、察哈尔、山东方向。
- 1937 年末至 1938 年初：日本能威胁山东、上海、南京、徐州方向。
- 1938 年：日本仍有推进能力，但速度明显下降。
- 1939 年后：中国战场进入长期相持，日本受到补给、占领、治安和兵力分散压力。

## 文本语言要求

新增的抗日战争相关内容必须全部使用中文。

适用范围：

- 事件标题。
- 事件描述。
- 事件按钮。
- 新闻标题。
- 新闻正文。
- 决议名称。
- 决议描述。
- 情势名称。
- 情势描述。
- 国家精神名称。
- 国家精神描述。
- UI 中面向玩家展示的战役、阶段、提示文本。

不得新增面向玩家显示的英文标题、英文描述或英文按钮。

允许保留代码、tag、flag、id、文件名、内部 key 使用英文，例如：

- `japan.north_china_campaign`
- `china_incident_escalated`
- `jap_priority_shandong`
- `second_sino_japanese_war`

但这些内部 key 对应的本地化文本必须是中文。

## 当前问题概述

现有 `crates/hoi4-content/content/situations/second_sino_japanese_war.ron` 会在 1937-07-07 自动开战，并把中国主要地方势力一次性加入统一战线和战争。

当前日方初始陆军规模：

- `JAP`：50 师。
- `MAN`：8 师。
- `MEN`：3 师。
- `HBC`：4 师。
- 合计：65 师。

当前中方开战后可能参战规模：

- `CHI`：80 师。
- `SND`：14 师。
- `PRC`：5 师。
- `SHX`：10 师。
- `GXC`：15 师。
- `GDC`：8 师。
- `YUN`：8 师。
- `XAJ`：6 师。
- `SIC`：18 师。
- `XSM`：6 师。
- `SIK`：5 师。
- 合计：175 师。

主要问题不是中国兵力数量本身，而是当前游戏缺少中国地方军低协同、战区迟滞、地方派系保守、日本历史攻势目标和日本对华专属 AI 策略。结果是日本 AI 使用通用前线逻辑时，容易判断优势不足，推进不到山东。

## 总体方案

采用四层修正：

1. 战争脚本层：调整统一战线参战节奏。
2. AI 战略层：给日本专属对华作战策略。
3. 战区目标层：让 AI 知道华北、山东、上海、南京是优先目标。
4. 军事表现层：加入中国初期低协同、日本初期攻势势头、后期补给和占领压力。

不要只通过削减中国师数或增加日本师数解决问题。

## Phase 1：最小可见修复（已完成）

目标：1937 年开战后，日本 AI 至少能推进到山东方向，不再卡死在华北边境。

### 1.1 调整统一战线参战节奏（已完成）

修改文件：

- `crates/hoi4-content/content/situations/second_sino_japanese_war.ron`

开战时立即参战：

- `CHI`
- `SND`
- `SHX`
- `PRC`

开战时加入统一战线但暂不直接参战：

- `GXC`
- `GDC`
- `YUN`
- `XAJ`
- `SIC`
- `XSM`
- `SIK`

理由：

- `SND` 必须参战，保证山东方向存在真实战场。
- `SHX` 必须参战，保证山西、华北方向存在压力。
- 其他地方军不应在 1937 年 7 月 7 日就作为完整高效前线力量影响华北/山东战局。

### 1.2 新增中文事件：全面抗战初期动员（已完成）

新增或扩展事件文件：

- `crates/hoi4-content/content/CHI_events.ron`
- `crates/hoi4-content/content/JAP_events.ron`
- `crates/hoi4-content/content/news_events.ron`

事件文本必须使用中文。

建议事件：

- `china.united_front_forms`：国共与地方实力派宣布共同抗战。
- `china.regional_command_autonomy`：地方军政系统仍保持高度自主。
- `japan.china_incident_escalated`：日本扩大华北事变。
- `news.marco_polo_bridge_incident`：卢沟桥事变。

中文标题示例：

- `卢沟桥事变`
- `全面抗战爆发`
- `华北局势失控`
- `中国统一战线形成`

中文按钮示例：

- `战争已经开始。`
- `全国进入抗战状态。`
- `扩大华北作战。`

### 1.3 日本 AI 对华目标修正（已完成）

修改文件：

- `crates/hoi4-ai/src/profile.rs`

将日本 AI 的目标从仅关注 `CHI`、`PRC` 扩展为：

- `SND`
- `CHI`
- `SHX`
- `PRC`

建议：

```rust
priority_targets: vec![
    "SND".into(),
    "CHI".into(),
    "SHX".into(),
    "PRC".into(),
],
force_attack_against: vec![
    "SND".into(),
    "CHI".into(),
    "SHX".into(),
],
```

### 1.4 山东和华北目标加分（已完成）

修改文件：

- `crates/hoi4-ai/src/ground.rs`

新增日本对华战争目标加分逻辑。

建议内部函数：

```rust
fn japan_china_area_bonus(world: &World, country: CountryId, state: StateId) -> f32
```

加分建议：

- 山东方向：`+45` 到 `+60`。
- 华北方向：`+35` 到 `+45`。
- 上海、南京方向：`+35` 到 `+50`。
- 徐州、武汉方向：`+20` 到 `+35`。

### 1.5 日本对华临时进攻偏置（已完成）

修改文件：

- `crates/hoi4-ai/src/ground.rs`
- `crates/hoi4-ai/src/orchestrator.rs`

当日本满足以下条件时：

- 国家 tag 为 `JAP`。
- 拥有 `china_incident_escalated` flag。
- 与任一中国势力交战。

对 `SND`、`CHI`、`SHX` 前线降低进攻阈值 10%-20%。

目标：

- 日本不因轻微力量比波动过早转入防御。
- 日本能在 1937 年持续施压山东、华北方向。

## Phase 2：历史战役链（已完成）

目标：让 1937 年不只是自动开战，而是出现华北、山东、上海、南京四条历史主线。

### 2.1 华北作战事件链（已完成）

新增中文事件：

- `japan.north_china_campaign`：华北作战扩大。
- `china.defense_of_north_china`：华北防御战。
- `news.fall_of_beiping_tianjin`：平津陷落。
- `news.battle_of_taiyuan`：太原会战。

效果建议：

- 日本获得短期华北进攻 AI 偏置。
- 中国 `SHX` 和 `CHI` 获得防御动员。
- 若日本推进过慢，轻微提高日本对华北方向 front demand。

### 2.2 山东作战事件链（已完成）

新增中文事件：

- `japan.shandong_campaign`：山东作战。
- `china.defense_of_shandong`：山东防御。
- `news.jinan_under_threat`：济南告急。
- `news.fall_of_jinan`：济南陷落。
- `news.qingdao_landing_or_pressure`：青岛方向战事扩大。

AI 要求：

- 日本在 `jap_priority_shandong` flag 存在时优先攻击 `SND`。
- 山东相关州获得最高早期目标加分。
- 若 1938 年 1 月仍未进入山东，触发额外 AI pressure。

### 2.3 淞沪会战事件链（已完成）

新增中文事件：

- `china.battle_of_shanghai`：淞沪会战。
- `japan.shanghai_expedition`：上海派遣军。
- `news.battle_of_shanghai_begins`：淞沪会战爆发。
- `news.shanghai_falls`：上海陷落。

效果建议：

- `GDC`、`GXC` 可以在此阶段正式参战或提供支援。
- 日本获得上海、南京方向目标加分。
- 中国获得全国抗战动员，但仍保留低协同惩罚。

### 2.4 南京阶段事件链（已完成）

新增中文事件：

- `japan.advance_on_nanjing`：进攻南京。
- `china.defense_of_nanjing`：南京保卫战。
- `news.nanjing_under_threat`：南京告急。
- `news.fall_of_nanjing`：南京陷落。

触发条件：

- 日本已控制上海或长江下游关键州。
- 或日本在华东方向取得足够进展。

要求：

- 不允许日本未推进到相关地区就自动触发南京陷落新闻。

## Phase 3：中国低协同与逐步动员（已完成）

目标：中国初期不能像现代统一军队一样高效，但中后期应逐步稳定。

### 3.1 中国初期低协同（已完成）

新增中文国家精神：

- `统一战线协调困难`
- `地方军政自主`
- `战区指挥迟滞`

建议效果：

- 初期组织度轻微下降。
- 初期进攻能力下降。
- 初期增援率下降。
- 地方军前线请求权重下降。

### 3.2 中国抗战动员（已完成）

新增中文国家精神：

- `全国抗战动员`
- `持久抗战方针`
- `后方工业迁移`

节奏：

- 1937 年：低协同最严重。
- 1938 年：低协同惩罚降低，防御和动员增强。
- 1939 年后：中国防御韧性提高，日本推进变慢。

## Phase 4：日本初期攻势与后期压力（已完成）

目标：日本 1937-1938 能打，1939 后难以继续高速推进。

### 4.1 日本初期攻势势头（已完成）

新增中文国家精神：

- `大陆攻势势头`
- `华北派遣军扩编`
- `中国事变扩大`

建议效果：

- 1937-1938 年攻击偏置提高。
- 组织恢复略增。
- 战区目标评分提高。
- 对华北、山东、上海方向 front demand 提高。

### 4.2 日本深入中国后的惩罚（已完成）

新增中文国家精神或动态惩罚：

- `占领区治安压力`
- `大陆补给线拉长`
- `兵力分散`

触发建议：

- 日本控制中国核心州 >= 5：轻微补给和占领压力。
- 日本控制中国核心州 >= 10：组织恢复、移动或补给惩罚。
- 日本控制中国核心州 >= 18：进攻能力下降，守备需求提高。

目标：

- 防止日本改强后一口气灭亡中国。
- 让战线在 1939 年后自然相持。

## Phase 5：战区级 AI 系统（已完成）

目标：让 AI 不只按敌国判断前线，而能理解战区优先级。

新增文件：

- `crates/hoi4-ai/src/china_theater.rs`

已实现结构：

```rust
pub enum ChinaWarPhase {
    MarcoPolo,
    NorthChina,
    Shandong,
    ShanghaiNanjing,
    Wuhan,
    Stalemate,
}

pub struct ChinaTheaterStrategy {
    pub active: bool,
    pub phase: ChinaWarPhase,
    pub priority_enemy_tags: Vec<&'static str>,
    pub force_attack_enemy_tags: Vec<&'static str>,
    pub preferred_state_ids: Vec<u16>,
    pub attack_bias: f32,
    pub min_front_ratio: f32,
}
```

用途：

- 判断日本是否处于对华战争。
- 判断当前战役阶段。
- 给华北、山东、上海、南京、武汉方向提供目标加分。
- 给 `ground.rs` 和 `ground_orders.rs` 提供进攻偏置和分兵权重。

## Phase 6：验证标准（已完成）

已补充三组可重复自动验证，并完成一次 headless smoke 运行检查。

验证记录：

- `cargo test -p hoi4-ai --test phase6_sino_japanese_validation`：3 项通过。
- `cargo run -p hoi4-app -- --headless --headless-days 1`：通过，系统调度不崩溃。
- 说明：当前 `hoi4-app` headless 入口只跑 `SystemSchedule`，不执行主 UI 循环里的 situation runtime，因此中日战争 1937 触发与三阶段节奏用新增的 Phase 6 AI 自动验证覆盖。

### 6.1 1937 年 7 月至 1938 年 1 月（已完成）

合格标准：

- 日本能在华北取得推进。
- 日本能进入或威胁山东。
- 中国不能在 1937 年反推满洲。
- 日本不能完全无阻力地一路打穿中国。

### 6.2 1938 年全年（已完成）

合格标准：

- 日本可以继续推进，但速度应低于 1937 年。
- 上海、南京、徐州、武汉方向应由实际控制进展决定是否触发新闻。
- 中国防线应逐步稳定。

### 6.3 1939 年至 1941 年（已完成）

合格标准：

- 日本不应轻松占领重庆。
- 中国不应轻松反推东北。
- 战线应进入长期消耗和相持。

## 推荐第一批改动清单

优先做这些，能最快修复“日本连山东都打不到”：

1. 已完成：修改 `second_sino_japanese_war.ron`，延迟部分中国地方军正式参战。
2. 已完成：修改 `profile.rs`，让日本强制攻击 `SND`、`CHI`、`SHX`。
3. 已完成：修改 `ground.rs`，给日本对华战争中的山东、华北目标加分。
4. 已完成：修改 `ground.rs`，日本对 `SND`、`CHI`、`SHX` 的进攻阈值临时降低。
5. 修改 `ground_orders.rs`，日本对华前线分兵时提高山东、华北方向权重。
6. 已完成：新增中文事件：卢沟桥事变、华北作战扩大、山东作战、淞沪会战、南京保卫战。
7. 已完成：新增中文国家精神中的“中国初期低协同”、“日本大陆攻势势头”和“日本占领区治安压力”已完成。

## 禁止事项

- 不要只通过大幅削减中国师数解决问题。
- 不要只通过大幅增加日本师数解决问题。
- 不要新增英文事件标题、英文事件描述、英文按钮或英文新闻。
- 不要让南京、武汉等新闻在日本未实际推进到相关地区时自动触发。
- 不要让日本 1937 年获得永久性无限攻势加成。

## 最终验收

路线图完成后，中日战争应呈现以下历史节奏：

- 日本 1937 年具备明显初期攻势能力。
- 山东不再成为日本 AI 无法突破的早期瓶颈。
- 中国初期混乱但不会快速崩盘。
- 日本深入后受补给、占领和兵力分散限制。
- 1939 年后战场进入长期相持。
- 所有新增抗日战争事件和内容均以中文展示。
