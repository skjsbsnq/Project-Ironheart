# 西班牙内战专属决议机制草案

对应 P0.9 和 P0.10。技术接口为 `DecisionMechanicKind`，普通决议仍使用 `Standard`。

## 共和国

- `SpainRepublicAuthority`：共和国权威。政府权威越高，人民军整编和行政效率越强；民兵自治越低，CNT/POUM 怨恨越高。
- `SpainSovietInfluence`：苏联影响。提高外援效率、坦克和顾问收益；增加政治条件、POUM 压力和战后依赖。
- `SpainCntTension`：CNT 紧张度。妥协降低短期内乱风险；整编和镇压提高中央效率但推进五月事件风险。
- `SpainMadridDefense`：马德里防御。投入武器、民兵、国际纵队和政治资本来延缓首都危机。

## 国民军

- `SpainFrancoAuthority`：佛朗哥权威。权威越高，军政府命令统一；权威不足时统一指挥和战后独裁需要额外代价。
- `SpainRightBalance`：右翼派系平衡。长枪党、卡洛斯派、教会、非洲军团之间互相牵制。
- `SpainForeignDependency`：外援依赖。德国/意大利援助提高战斗力；战后索偿、外交束缚和舆论风险上升。
- `SpainOccupationOrder`：占领区秩序。恐怖镇压提高短期控制；行政治理提高长期稳定但消耗资源。

## CNT/POUM

- `SpainCntRevolutionaryCommittee`：革命委员会。集体化和民兵自治提供革命动员；削弱共和国协调。
- `SpainBarcelonaTension`：巴塞罗那紧张度。电话局危机、POUM 命运和苏联影响共同推动五月事件风险。
- `SpainRevolutionOrCompromise`：革命输出 vs 生存妥协。决定 CNT 是保卫共和国、夺取革命领导权，还是在孤立中崩溃。

## 面板要求

- 独立 `Decisions` 面板显示普通决议与专属机制，不依附政治面板。
- 不可见决议隐藏，不可用决议灰显，可用决议高亮。
- 所有西班牙专属 section 标题、说明、风味和后果预览必须为中文。
- 决议效果应设置 flags 或变量，事件链必须能读取这些后果。
