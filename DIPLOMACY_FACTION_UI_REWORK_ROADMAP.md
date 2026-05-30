# 外交系统、阵营系统与外交 UI 重做路线图

## 目标

重做外交与阵营系统，使其从当前的原型式“国家列表 + 直接按钮”升级为可解释、可扩展、可被 UI/AI/脚本统一调用的正式系统。

本路线图覆盖四个范围：

- 外交数据模型：国家关系、外交行动、请求、条约、战争目标、世界紧张度。
- 阵营系统：稳定阵营 ID、成员管理、邀请流程、战争联动、阵营概览。
- 外交主面板：统一成政治面板/财政面板同代的棕金卡片式主面板。
- 右键国家外交面板：复用同一套国家外交详情，不再维护另一套割裂 UI。

## 当前问题

### 系统层问题

- `DiplomacyState` 已有阵营、战争、战争目标、傀儡、opinion、世界紧张度、军事通行权，但整体仍偏薄。
- `FactionId` 目前实际依赖 `Vec` 下标，删除阵营后会重排，不适合复杂阵营系统。
- 外交行动没有统一的可用性判断和失败原因，UI 无法解释“为什么不能做”。
- 阵营邀请、军事通行权等行为目前是即时生效，没有请求、接受、拒绝、撤回流程。
- 脚本、AI、App 层存在绕过逻辑层直接修改外交状态的代码，容易产生两套规则。
- 和平会议目前可以由 UI 直接执行，没有投降、战争分数、胜负条件等约束。

### UI 层问题

- 外交主面板宽度 `360`、不可缩放，和政治/财政面板的 `540` 宽主面板不一致。
- 外交主面板没有使用 `components::panel_header`、`summary_strip`、状态横幅、卡片区块。
- 右键国家面板是独立 `egui::Window` 弹窗，视觉和外交主面板、政治面板、财政面板都不统一。
- 外交主面板与右键国家面板功能不一致：一个可直接邀请入阵营，一个只是 TODO 日志；一个没有正当化进度，一个有。
- 行动按钮缺少成本、风险、可用条件、AI 接受度和禁用原因。

## 设计原则

- 先收口规则，再重做 UI。UI 不应直接猜外交规则，应展示逻辑层给出的可用性与原因。
- 先做最小正确模型，再扩展复杂外交内容。
- 外交主面板和右键国家面板必须共享同一套国家外交详情渲染逻辑。
- AI、脚本、玩家 UI 尽量走同一套外交动作入口；若脚本需要强制效果，也要显式命名为 privileged effect。
- 保存兼容要提前考虑：状态结构改动前先确认 save roundtrip 覆盖。

## 阶段 0：盘点与护栏（已完成）

目标：在动模型前确认所有读写入口，避免遗漏。

任务：

- 列出所有读写 `world.diplomacy` 的位置。
- 标记直接写入 `factions`、`autonomy`、`pending_wargoals`、`military_access` 的代码。
- 检查存档序列化是否覆盖外交状态，补充必要 roundtrip 测试。
- 整理当前 UI 入口：外交主面板、右键国家信息面板、省份菜单宣战入口。

交付物：

- 已完成：`docs/diplomacy_phase0_audit.md` 跟踪清单。
- 已完成：外交状态已纳入文本/二进制存档 roundtrip。
- 已完成：当前外交行为基线测试通过。

验收：

- 已完成：能明确说明每种外交状态由谁写入。
- 已完成：AI、脚本、UI 入口已进入阶段 0 跟踪清单。

## 阶段 1：建立统一外交动作模型（已完成）

目标：让 UI、AI、脚本可以查询“能不能做、为什么、做了会怎样”。

建议新增或重构：

- `DiplomaticAction`
- `DiplomaticActionTarget`
- `ActionAvailability`
- `UnavailableReason`
- `ActionPreview`
- `DiplomacyError`
- `evaluate_action(world, actor, action) -> ActionAvailability`
- `execute_action(world, actor, action) -> Result<ActionOutcome, DiplomacyError>`

首批动作：

- `StartJustification`
- `DeclareWar`
- `CreateFaction`
- `InviteToFaction`
- `LeaveFaction`
- `RequestMilitaryAccess`
- `GrantMilitaryAccess`
- `RevokeMilitaryAccess`
- `ResolvePeace` 或临时 `DebugResolvePeace`

任务：

- 把外交主面板命令处理改为调用统一入口。
- 把右键国家面板命令处理改为调用统一入口。
- `DeclareWar` 的可用性必须能说明是否缺少 wargoal。
- `RequestMilitaryAccess` 必须能说明 opinion 是否不足。
- `InviteToFaction` 暂时可以保留即时加入，但入口必须先统一。

交付物：

- 已完成：新增 `DiplomaticAction`、`ActionAvailability`、`UnavailableReason`、`ActionPreview`、`DiplomacyError`、`evaluate_action`、`execute_action`。
- 已完成：外交主面板、右键国家面板、省份菜单命令已改为调用统一动作入口。
- 已完成：外交主面板和右键国家面板按钮会基于统一动作可用性启用/禁用，并在 hover 中展示失败原因。

验收：

- 已完成：外交主面板宣战不可用时会显示“需要已正当化的战争目标”等明确原因。
- 已完成：右键面板和外交主面板共享统一动作可用性 DTO/展示与执行入口。
- 已完成：原有外交测试通过，并新增动作可用性测试。

## 阶段 2：稳定外交与阵营状态模型（已完成）

目标：消除下标式阵营 ID 和不可追踪关系，为复杂外交打基础。

建议模型：

```rust
pub struct FactionId(pub u32);
pub struct TreatyId(pub u32);
pub struct DiplomaticRequestId(pub u32);

pub struct DiplomaticRelation {
    pub a: CountryId,
    pub b: CountryId,
    pub opinion_ab: i16,
    pub opinion_ba: i16,
    pub threat_ab: f32,
    pub threat_ba: f32,
    pub trust: f32,
}

pub enum TreatyKind {
    MilitaryAccess,
    NonAggressionPact,
    GuaranteeIndependence,
    FactionMembership,
    SubjectRelation,
    Truce,
}

pub struct Treaty {
    pub id: TreatyId,
    pub kind: TreatyKind,
    pub parties: Vec<CountryId>,
    pub since_hour: u64,
    pub expires_at_hour: Option<u64>,
}
```

任务：

- 将 `FactionId` 改为稳定 ID，阵营存储不再依赖 Vec 下标语义。
- 为 `DiplomacyState` 添加 `next_faction_id`、必要索引或查找函数。
- 保留兼容 API：`faction_of(country)`、`at_war_with(a,b)`、`has_military_access(from,to)`。
- 军事移动、贸易、AI 等调用方继续通过 API 查询，不直接碰内部结构。

交付物：

- 已完成：`FactionId` 改为稳定 `u32` ID，阵营删除不再重排 ID 语义。
- 已完成：`DiplomacyState` 添加 `next_faction_id`、`faction`、`faction_mut`、`faction_members` 等查询/查找 API。
- 已完成：军事、战争、AI、脚本、渲染、content 调用点不再按 `FactionId` 下标直接读取阵营。

验收：

- 已完成：新增稳定阵营 ID 单测，删除阵营不会导致其他阵营 ID 语义变化。
- 已完成：`hoi4-state`、`hoi4-logic`、`hoi4-ui`、`hoi4-ai`、`hoi4-app` 相关检查通过。

## 阶段 3：外交请求与条约系统（已完成）

目标：把“直接生效”的外交动作变成可解释的请求/条约流程。

新增概念：

```rust
pub enum DiplomaticRequestKind {
    InviteToFaction { faction_id: FactionId },
    RequestMilitaryAccess,
    OfferNonAggressionPact,
    OfferPeace,
}

pub enum DiplomaticRequestStatus {
    Pending,
    Accepted,
    Rejected,
    Expired,
    Withdrawn,
}
```

任务：

- `InviteToFaction` 改为创建请求，而不是直接加入。
- `RequestMilitaryAccess` 改为创建请求，对方接受后生成通行权条约。
- AI 每日或每周评估待处理请求。
- 玩家收到请求后在外交面板显示“待处理外交请求”。
- 条约到期、撤销、取消进入 daily diplomacy tick。

交付物：

- 已完成：新增 `DiplomaticRequest`、`DiplomaticRequestKind`、`DiplomaticRequestStatus`、`Treaty`、`TreatyKind` 与稳定 ID。
- 已完成：`InviteToFaction` 与 `RequestMilitaryAccess` 改为创建请求，不再直接生效。
- 已完成：`tick_diplomatic_requests` 处理接受、拒绝、过期，并生成阵营加入或军事通行条约。
- 已完成：外交日 tick 接入请求/条约生命周期。
- 已完成：文本/二进制存档 roundtrip 覆盖请求、条约和 next id。

验收：

- 已完成：玩家邀请国家入阵营后创建请求，AI tick 可接受并加入阵营。
- 已完成：军事通行权请求接受后生成 `MilitaryAccess` 条约，`has_military_access` 由条约派生查询。
- 已完成：外交主面板显示待处理外交请求和结果。

## 阶段 4：阵营系统重做（已完成）

目标：让阵营从“成员数组”升级为可管理的外交实体。

建议扩展：

- 阵营领袖。
- 成员列表。
- 创建时间。
- 阵营战争列表。
- 可邀请候选。
- 阵营凝聚度或领导权评分，后续可选。
- 阵营加入规则：同意识形态、关系、世界紧张度、是否交战、是否被保证。

任务：

- 阵营成员加入/退出走统一 action。
- 阵营邀请走 request。
- 阵营战争传播规则单独封装，避免散在 `declare_war` 和 daily reconcile 中。
- 阵营解散、转让领导、踢出成员统一可用性判断。
- AI 的 `strategic_profile` 不再直接 `join_faction`，而是发起或接受请求。

交付物：

- 已完成：阵营创建、加入、退出、踢出、解散、转让领导继续走统一逻辑层并使用稳定 ID。
- 已完成：阵营邀请走 request 流程。
- 已完成：阵营战争传播规则集中在 `declare_war` / `reconcile_war_membership`，并改为稳定 ID 查询。
- 已完成：AI `strategic_profile` 不再直接 `join_faction`，改为发起邀请请求并通过请求 tick 处理。
- 已完成：阵营管理、战争传播、请求流程测试通过。

验收：

- 已完成：历史阵营创建仍由 AI profile 推进，加入行为改为请求后接受。
- 已完成：玩家邀请、退出、创建阵营由统一 action 返回明确结果或禁用原因。
- 已完成：阵营成员入战规则保留并由外交集成测试覆盖。

## 阶段 5：外交主面板 UI 重做（已完成）

目标：把外交主面板升级为政治/财政同代的主面板。

布局要求：

- 使用 `SidePanel::left("diplomacy_panel")`。
- `default_width(540.0)`。
- `min_width(460.0)`。
- `resizable(true)`。
- 使用 `components::panel_header`。
- 顶部使用 `components::summary_strip`。
- 增加外交状态 banner。
- 内容区使用卡片式 section。

建议结构：

- 顶部摘要：世界紧张度、当前战争数、我国阵营、待处理请求、正在正当化。
- 状态横幅：和平稳定、外交孤立、战争风险、请求待处理、正当化完成等。
- 我国外交卡：阵营、附庸、通行权、当前外交请求。
- 国家列表卡：搜索、筛选、排序、国家行。
- 选中国家详情卡：关系、条约、阵营、战争目标、可执行行动。
- 阵营总览卡：所有阵营、成员数量、领袖、是否与我国敌对。
- 战争与和平卡：战争双方、分数、战争目标、和平按钮与禁用原因。

UI DTO 建议：

```rust
pub struct DiplomacyPanelData {
    pub summary: DiplomacySummaryView,
    pub status: DiplomacyStatusView,
    pub countries: Vec<CountryDiplomacyRow>,
    pub selected_country: Option<CountryDiplomacyDetail>,
    pub factions: Vec<FactionView>,
    pub wars: Vec<WarView>,
    pub requests: Vec<DiplomaticRequestView>,
}

pub struct AvailableActionView {
    pub action: DiplomaticAction,
    pub label: String,
    pub enabled: bool,
    pub reason: Option<String>,
    pub preview: String,
    pub risk_level: RiskLevel,
}
```

任务：

- 拆分 `diplomacy.rs` 渲染函数，不再所有内容塞进 `show`。
- 新增外交卡片 helper，风格对齐 `politics_card` / `finance_card`。
- 国家列表不要直接堆按钮，按钮移动到详情卡。
- 所有 action 按钮展示 disabled reason 或 hover reason。
- 和平会议英文硬编码改为本地化或中文一致文案。

交付物：

- 已完成：新外交主面板使用 `SidePanel::left("diplomacy_panel")`、`default_width(540.0)`、`min_width(460.0)`、`resizable(true)`。
- 已完成：外交主面板使用 `components::panel_header`、`summary_strip`、状态 banner 和棕金卡片区块。
- 已完成：国家列表只负责选择国家，外交按钮移动到选中国家详情卡。
- 已完成：所有 action 按钮基于统一动作可用性启用/禁用，并在 hover 中显示原因。

验收：

- 已完成：外交面板宽度、标题、卡片、摘要、状态横幅与政治/财政统一。
- 已完成：玩家能从主面板完成正当化、宣战、邀请、请求通行、和平处理。
- 已完成：不可用动作有明确原因。

## 阶段 6：右键国家面板统一（已完成）

目标：消除右键国家信息面板和外交主面板割裂。

推荐方案：

- 右键国家默认打开外交主面板并选中该国家。
- 如果保留弹窗，弹窗内部也必须复用 `render_country_diplomacy_detail`。

任务：

- 抽出国家外交详情渲染函数。
- `country_info_panel.rs` 不再单独维护一套外交按钮逻辑。
- 经济/军事简况可以保留，但外交动作必须来自统一 `AvailableActionView`。
- 右键面板的邀请入阵营 TODO 必须删除或接入统一动作。

交付物：

- 已完成：主面板与右键面板共享 `CountryDiplomacyDetail` 和 `render_country_diplomacy_detail`。
- 已完成：右键弹窗保留经济/军事简况，但外交动作区复用主面板同一套动作展示和命令映射。

验收：

- 已完成：同一目标国家在两个入口看到的外交状态和可执行动作一致。
- 已完成：右键面板邀请入阵营、请求军事通行权已接入统一动作，不再只是日志或无反馈。

## 阶段 7：战争目标与和平会议完善（已完成）

目标：让战争和和平从调试按钮升级为正式流程。

任务：

- 外交面板展示 pending wargoal、justified wargoal、战争目标来源。
- 宣战按钮只在有可用 wargoal 时启用，否则显示缺少 wargoal。
- 和平会议增加可用性判断：参战方、战争分数、投降状态、目标是否可执行。
- 暂时保留 debug resolve 时必须明确标记为 debug 或开发按钮。
- 后续加入和平提议请求和 AI 接受度。

交付物：

- 已完成：外交主面板和右键国家面板共享的国家详情中展示 pending/justified 战争目标、进度、剩余天数、目标州和来源。
- 已完成：宣战按钮继续基于统一 action 可用性启用/禁用，缺少战争目标时展示明确原因。
- 已完成：和平按钮接入统一 action 可用性判断，检查参战方、获胜方、战争分数和敌方投降状态。
- 已完成：战争与和平卡展示和平条款预览、按钮预览和禁用原因。

验收：

- 已完成：玩家能在国家详情中看到战争目标状态，并在宣战不可用时看到缺少正当化战争目标等原因。
- 已完成：玩家能在战争卡中看到和平条款预览，并在和平不可用时看到战争分数/投降条件不足等原因。
- 已完成：新增 `ResolvePeace` action 可用性回归测试。

## 阶段 8：关系解释与外交可读性（已完成）

目标：外交面板不仅能操作，还能解释国际关系。

新增视图：

- opinion 总值。
- opinion 来源列表。
- threat 来源。
- ideology affinity。
- trade relation。
- border tension。
- faction relation。
- subject/overlord relation。

任务：

- 从简单 `OpinionMatrix` 逐步升级为可解释修饰项。
- UI 显示“关系构成”卡片。
- AI 接受度也显示主要原因。

交付物：

- 已完成：国家外交详情新增“关系构成”列表，展示双方 opinion、战争状态、阵营关系和附庸关系。
- 已完成：主外交面板和右键国家面板共享同一套关系解释 DTO 与渲染。
- 已完成：军事通行/邀请等 action 仍显示统一可用性原因，关系不足时展示当前值和需求值。

验收：

- 已完成：玩家能通过国家详情中的关系构成和 action 禁用原因理解目标国为什么接受/拒绝关键外交请求。

## 阶段 9：测试与回归

每阶段都应维护测试，最终补齐以下测试：

- 外交动作可用性测试。
- 外交请求生命周期测试。
- 条约创建、撤销、过期测试。
- 阵营稳定 ID 测试。
- 阵营邀请/加入/退出/解散/转让领导测试。
- 阵营战争传播测试。
- UI DTO 构建测试。
- 存档 roundtrip 测试。
- AI 阵营行为测试。
- 脚本 effect 不绕过逻辑层测试。

建议常用命令：

```powershell
cargo test -p hoi4-state
cargo test -p hoi4-logic diplomacy
cargo test -p hoi4-ai
cargo test -p hoi4-ui
cargo check
```

## 推荐实施顺序

优先级最高：

1. 阶段 0：盘点与护栏。
2. 阶段 1：统一外交动作模型。
3. 阶段 5：外交主面板视觉重做第一版。
4. 阶段 6：右键国家面板统一。

中期：

1. 阶段 2：稳定外交与阵营状态模型。
2. 阶段 3：外交请求与条约系统。
3. 阶段 4：阵营系统重做。

后期：

1. 阶段 7：战争目标与和平会议完善。
2. 阶段 8：关系解释与外交可读性。
3. 阶段 9：测试与回归。

## 最小可交付版本

如果只想先快速改善体验，最小版本可以这样做：

- 外交主面板改为 `540px` 可缩放。
- 使用 `components::panel_header`。
- 添加 summary strip 和状态 banner。
- 把外交内容拆成卡片。
- 国家列表只负责选择国家，不再塞三个按钮。
- 右侧或下方国家详情显示外交行动。
- 统一右键国家面板和外交主面板的 action availability。
- 宣战不可用时显示“需要已正当化战争目标”。
- 邀请入阵营和请求通行权至少在两个入口行为一致。

这个最小版本不要求马上引入完整 treaty/request 系统，但必须先完成动作可用性统一，否则 UI 仍然会和规则脱节。
