# 西班牙内战事件大纲与索引

用途：本文件只列事件规划、归属、触发和作用。所有事件正文、标题、选项未来必须写中文。事件 id 保持英文 ASCII。

风格目标：参考 KR/TNO 的路线式叙事，但不照搬文本。事件链应强调人物权力斗争、派系代价、战争遗产和战后制度化，而不是只有“获得加成”。国民军路线重点扩展为三种胜利后国家形态：佛朗哥胜利、长枪党胜利、长枪党架空佛朗哥并制度化国家工团主义。

## 当前第一轮范围

第一轮事件索引按“战前完整 + 国民军内战/战后完整”执行，不按全派系完整重做执行。

- 内战前全部内容进入第一轮：共和国战前危机、军官密谋、社会撕裂、政变评分和内战爆发前夜都要完整。
- 内战中只完整制作国民军可玩路线：国民军军政府、佛朗哥上升、莫拉竞争、长枪党继承危机、卡洛斯派/教会/非洲军团/外援依赖都进入第一轮。
- 战后只完整制作国民军路线：佛朗哥胜利、长枪党胜利、长枪党架空佛朗哥并制度化国家工团主义三条路线进入第一轮。
- 共和国战中事件本轮只做最小支撑：用于玩家选共和国不空壳、AI 行为、国民军事件读取、战役新闻和结局清理，不做完整共和国胜利战后路线。
- CNT/POUM 本轮只做压力来源和触发条件：五月事件风险、共和国左翼裂痕、国民军宣传与战后清算记忆，不做完整 CNT 可玩路线。
- 国际干涉优先服务国民军：德国、意大利、葡萄牙、英国直布罗陀/地中海压力优先；苏联、法国、墨西哥只做共和国背景所需最小事件。

## 事件效果硬规则

本项目经济和建筑生产系统已经不是 HOI4 vanilla 的民工/军工/船坞槽位模型。事件效果不得照搬 HOI4 式抽象 buff。

- 禁止把主要后果写成“消费品工厂 -10%”“增加军用工厂”“获得 2 个民用工厂”“生产效率 +10%”这类不可解释模板。
- 需要增强军工时，应写成当前系统效果：修复或扩建 `arms_industry`、`munition_plant`、`engine_plant`、`vehicle_factory`、`machine_tool_works` 等建筑，调整生产方式，增加政府军工订单，补充钢、煤、机床、弹药或装备库存，或改善就业与物流瓶颈。
- 需要处理民生压力时，应写成粮食、燃料、纺织品、药品、工资、配给、黑市、进口、财政补贴、POP 需求满足率、满意度或激进化变化。
- 需要表现战后重建时，应写成铁路、港口、电网、建设公司、州建筑等级、建造队列、财政债务、外汇、贸易协定、劳动力转移和 POP 生活水平变化。
- 新闻事件可以没有直接数值效果，但必须触发或解锁国家事件、隐藏事件、flags、决议机制条或外交压力。

## 归属说明

- `SPR`：西班牙共和国事件，玩家或 AI 共和国可见。
- `SPA`：国民军西班牙事件，内战爆发后 `SPA` 生成才可用。
- `CNT`：无政府派/CNT/POUM 相关事件。
- `GER`：德国干涉事件。
- `ITA`：意大利干涉事件。
- `SOV`：苏联干涉事件。
- `FRA`：法国边境/不干涉事件。
- `ENG`：英国/不干涉委员会事件。
- `POR`：葡萄牙援助国民军事件。
- `MEX`：墨西哥援助共和国事件。
- `NEWS`：世界新闻事件。
- `HIDDEN`：隐藏自动事件，用于触发、修正、清理、转接。
- `SPAIN`：不属于某一阵营的玩家选择或全西班牙事件。

## 类型说明

- `国家事件`：给某个国家看的事件，有选择和后果。
- `新闻事件`：所有国家可见，主要承担世界叙事。
- `隐藏事件`：玩家不可见，自动设置 flag、触发新闻或修正状态。
- `选择事件`：用于玩家选择阵营或路线。
- `权力斗争事件`：路线分歧中的关键分叉，通常影响派系分数。
- `人物互动事件`：两个或多个具名人物在同一政治场景中对话、交易、威胁或互相试探。
- `结局事件`：内战结束后触发。

## 路线变量建议

- `spa_franco_authority`：佛朗哥个人权威。来自非洲军团、外援谈判、最高统帅任命、战役胜利。
- `spa_falange_power`：长枪党权力。来自何塞·安东尼奥神话、赫迪利亚危机处理、宣传、工团组织。
- `spa_army_loyalty`：军队忠诚。决定将领是否接受政治党化。
- `spa_church_influence`：教会影响。决定国民天主教路线强度和长枪党世俗化阻力。
- `spa_carlist_anger`：卡洛斯派怨恨。决定统一法令后是否有北方反弹。
- `spa_foreign_dependency`：对德意依赖。影响战后外交选择和索偿事件。
- `spa_syndicalist_institutionalization`：国家工团主义制度化进度。只在长枪党架空佛朗哥路线使用。

## 人物使用与镜头轮换规则

问题修正：后续事件不能反复让佛朗哥、苏涅尔、赫迪利亚、吉隆、卡雷罗、巴雷拉等少数核心人物承担所有场景。KR/TNO 风格的重点不是“每个事件都让领袖讲话”，而是让不同层级的人物、机构和普通社会视角共同证明路线正在改变国家。

- 核心人物只用于路线分歧、重大法令、外交条约、政变、清洗、结局和继承危机。普通经济、地方治理、配给、铁路、学校、港口、军工、教会和殖民行政事件应优先使用二线人物或机构代表。
- 同一条事件链中，连续两个可见事件不应使用同一个核心人物作为主视角；如果必须出现，第三个事件必须换成秘书、部长次官、地方总督、军区参谋、工会干部、主教、企业代表、外国顾问或普通社会视角。
- 每 10 个可见事件中，佛朗哥最多作为主视角出现 2 次；苏涅尔、赫迪利亚、吉隆、卡雷罗、巴雷拉各最多作为主视角出现 1 到 2 次。其余事件可以让他们作为签字者、被引用者或缺席压力存在。
- 人物互动事件必须至少包含一个“非顶层人物”：副部长、地方官、军区参谋、新闻审查官、银行家、铁路工程师、工厂主任、港务官、教区代表、红贝雷民兵队长、蓝衫青年干部、共和国市政官、CNT 工会代表、POUM 律师等。
- 国家事件不一定要让大人物出场。可以写“部长会收到报告”，但镜头落在统计员、打字员、码头装卸队、医院院长、车间主任、村镇神父、军需官、学校督学或边境宪兵身上。
- 新闻事件应减少国内核心人物露面，改写外国报纸、驻外使馆、流亡者、银行、电台、铁路公司、难民和港口传闻如何理解西班牙变化。
- 历史人物不足时，允许使用“有职务的无名人物”承载场景，但不要创造会与真实历史大人物竞争的虚构国家领袖。无名人物应服务于制度和社会后果，而不是成为新主角。

### 可轮换的二线人物与机构视角

- 佛朗哥路线二线人物：尼古拉斯·佛朗哥、胡安·路易斯·贝格贝德尔、何塞·拉拉斯、华金·本胡梅亚、胡安·安东尼奥·苏安塞斯、何塞·伊瓦涅斯·马丁、布拉斯·佩雷斯·冈萨雷斯、何塞·玛丽亚·德阿雷尔萨、费尔南多·玛丽亚·卡斯蒂耶拉。
- 长枪党路线二线人物：佩德罗·加梅罗·德尔卡斯蒂略、何塞·安东尼奥·埃洛拉、纳西索·佩拉莱斯、旧衫派省级领袖、青年阵线干部、女性部门督导、垂直工会地方代表、党报审查员、社会援助组织负责人。
- 军队与安全系统：菲德尔·达维拉、路易斯·奥尔加斯、何塞·索尔查加、卡米洛·阿隆索·维加、安东尼奥·阿兰达、穆尼奥斯·格兰德斯、米兰-阿斯特赖、军区参谋长、宪兵司令、军需处长、军事法庭书记员。
- 王党、卡洛斯派与教会：佩德罗·赛因斯·罗德里格斯、托马斯·多明格斯·阿雷瓦洛、何塞·玛丽亚·吉尔-罗夫莱斯、红贝雷地方指挥官、主教会议代表、修会学校校长、乡镇神父、天主教行动组织干部。
- 共和国与左翼二线人物：胡利安·贝斯特罗、何塞·迪亚斯、比森特·乌里韦、费德丽卡·蒙塞尼、胡安·加西亚·奥利韦尔、塞普里亚诺·梅拉、恩里克·利斯特、何塞·米亚哈、何塞·希拉尔、共和国省长、市政委员会书记、人民军政委。
- 外国与外交视角：德国驻西使节、秃鹰军团联络官、SOFINDUS 商务代表、意大利军事顾问、英国驻直布罗陀参谋、法国边境警察、葡萄牙边防军官、苏联顾问、墨西哥外交官、国际纵队招募人。
- 经济与社会视角：财政部统计员、外汇局官员、国家工业机构工程师、铁路调度员、港务局长、矿山经理、银行清算员、配给办公室主任、黑市调查员、医院药剂师、孤儿院院长、退伍军人办事员。
- 殖民与北非视角：摩洛哥老兵代表、哈里发宫廷联络官、部落长老、卡迪法官、法国定居者市政代表、阿尔及利亚民族主义律师、港口翻译、传教学校校长、矿区承包商、殖民宪兵分队长。

### 事件镜头模板

- 最高层镜头：只用于路线决定、政权形式、外交大交易和结局。例：考迪罗签字、部长会议投票、柏林仲裁、罗马抗议。
- 中层镜头：用于政策如何落地。例：次官分配预算、军区参谋改写命令、党务官任命省代表、主教会议要求删改教育法。
- 地方镜头：用于展示政策代价。例：市长接收配给卡、港务官给德国船让泊位、矿区经理赶走旧工会、学校督学更换教材。
- 社会镜头：用于避免人物重复并增强 TNO 式压迫感。例：退伍兵等不到抚恤、护士清点德国药箱、工人听见机床重启、摩洛哥老兵发现承诺被改写。
- 缺席镜头：核心人物可以不出现，只通过命令、照片、签名、广播、密电和他人恐惧存在。这样能保留权威感，同时避免反复写同一个人进房间。

## 国民军战后主要人物池

写作要求：KR/TNO 风格不应只写抽象派系，但也不能让少数核心人物反复霸占镜头。每条路线都必须让历史人物和机构人物出场，并通过他们体现路线内部矛盾。事件正文应尽量交替出现“谁在房间里、谁签字、谁执行、谁被命令影响、谁被边缘化”。

### 佛朗哥路线人物

- 弗朗西斯科·佛朗哥：考迪罗、最高仲裁者，核心不是思想而是平衡术。
- 拉蒙·塞拉诺·苏涅尔：佛朗哥内兄，早期亲轴、法西斯化、外交和国民运动整合的关键人物。
- 弗朗西斯科·戈麦斯-霍尔达纳：保守外交和谨慎中立的代表。
- 何塞·恩里克·巴雷拉：军队和传统天主教军人利益代表。
- 路易斯·卡雷罗·布兰科：后期行政连续性、海军官僚和秩序派代表。
- 阿尔韦托·马丁-阿尔塔霍：战后天主教外交和国际解冻代表。
- 阿尔韦托·乌利亚斯特雷斯、马里亚诺·纳瓦罗·鲁维奥、劳雷亚诺·洛佩斯·罗多：技术官僚和发展主义经济代表。

### 长枪党胜利路线人物

- 曼努埃尔·赫迪利亚：最适合担任长枪党胜利路线的领袖，代表“何塞·安东尼奥遗产”的党内合法性。
- 拉蒙·塞拉诺·苏涅尔：可以成为蓝衫国家的外交和法制建筑师，也可能试图把党国变成自己的机器。
- 雷蒙多·费尔南德斯-奎斯塔：组织派和党务官僚，适合掌管国民运动、司法或党纪律。
- 何塞·安东尼奥·吉隆：劳工、福利、垂直工会和社会民粹主义代表。
- 何塞·路易斯·阿雷塞：意识形态纯化、党国宪章和长枪党教义代表。
- 迪奥尼西奥·里德鲁埃霍：宣传、文学化革命语言和青年动员代表，也可成为路线内的良心裂缝。
- 皮拉尔·普里莫·德里维拉：女性部门、社会服务、家庭道德和何塞·安东尼奥家族象征。
- 胡安·亚圭：亲长枪党军人、非洲军团和军队党化的桥梁。
- 阿古斯丁·阿斯纳尔、桑乔·达维拉：旧衫派、民兵暴力和党内激进压力代表。

### 架空佛朗哥路线人物

- 佛朗哥：仍是胜利象征和最终签字者，但逐渐被制度包围。
- 赫迪利亚：如果未被清洗，是秘书处路线的党内合法性来源。
- 费尔南德斯-奎斯塔：把革命变成章程、任命和纪律的组织官僚。
- 吉隆：通过劳工部、福利和垂直工会把国家工团主义制度化。
- 阿雷塞：负责把长枪党教义写入国家组织法。
- 塞拉诺·苏涅尔：早期提供法制和外交框架，后期可能被秘书处疑惧。
- 卡雷罗·布兰科：保守行政和军队信任的缓冲器，可能与蓝衫官僚争夺“过滤考迪罗”的位置。
- 巴雷拉、金德兰、奎波·德·利亚诺：军队边界、反党化和胜利者内部威胁的具名代表。

## 战前与内战主要人物池

写作要求：战前和内战中也必须让人物出场，但核心人物不应成为所有事件的默认叙事口。KR/TNO 风格的事件不应只写“政府”“军队”“长枪党”，也不应每次都写“阿萨尼亚在内阁桌前沉默”“莫拉把命令夹进信封”“赫迪利亚在蓝衫总部等待电话”。应把镜头分散到地方省长、军区参谋、市政官、民兵队长、工会代表、报社编辑、教区神父和普通士兵身上。

### 共和国人物

- 曼努埃尔·阿萨尼亚：共和国总统，代表合法性、疲惫和对军人叛乱的误判。
- 圣地亚哥·卡萨雷斯·基罗加：内战前总理，代表犹豫、拒绝过早武装群众和内阁崩溃。
- 迭戈·马丁内斯·巴里奥：短暂调停者，可用于“最后一次给将军们打电话”的失败事件。
- 弗朗西斯科·拉尔戈·卡瓦列罗：社会主义左翼和战争动员政府代表。
- 胡安·内格林：集中抗战、苏援、纪律和“抵抗到底”代表。
- 因达莱西奥·普列托：温和社会主义、军政现实主义和对苏联影响的警告者。
- 比森特·罗霍：共和国军事专业化和人民军整编代表。
- 多洛雷斯·伊巴露丽：共和国宣传、马德里动员和“不许通过”代表。
- 布埃纳文图拉·杜鲁蒂：CNT 武装革命、前线神话和民兵自由精神代表。
- 安德烈斯·宁：POUM、反斯大林主义左翼和苏联压力下的牺牲者。
- 路易斯·孔帕尼斯：加泰罗尼亚自治、巴塞罗那权力结构和与 CNT 的紧张关系代表。

### 国民军人物

- 何塞·桑胡尔霍：旧政变领袖和王政门面，他的死亡打开国民军权力真空。
- 埃米利奥·莫拉：政变设计者、“导演”，代表阴谋网络、北方军队和冷酷计划。
- 弗朗西斯科·佛朗哥：非洲军团、谨慎野心和逐步吃掉同僚的权力上升。
- 米格尔·卡瓦内利亚斯：国防委员会主席，早期警惕佛朗哥成为独裁者。
- 冈萨洛·奎波·德·利亚诺：塞维利亚电台、恐怖宣传和地方军阀式权力。
- 胡安·亚圭：非洲军团、巴达霍斯、亲长枪党军人和战场残酷性。
- 何塞·恩里克·巴雷拉：传统主义军人、教会/卡洛斯派同情和反长枪党过度党化。
- 阿尔弗雷多·金德兰：空军、王党和军内反佛朗哥独裁疑虑。
- 曼努埃尔·赫迪利亚：何塞·安东尼奥死后长枪党继承危机核心。
- 何塞·安东尼奥·普里莫·德里维拉：多数路线中作为“缺席者”和死后合法性出现。
- 拉蒙·塞拉诺·苏涅尔：佛朗哥阵营中的法制、外交和长枪党化桥梁。
- 曼努埃尔·法尔·孔德：卡洛斯派政治领袖，红贝雷民兵和王位承诺的压力来源。

## P0/P1 基础事件

| 编号 | 事件 id | 归属 | 类型 | 触发 | 大纲 | 关键后果 |
|---|---|---|---|---|---|---|
| E0001 | `spain.scw_choose_side` | SPAIN | 选择事件 | 内战爆发、`SPA` 已生成 | 玩家选择保卫共和国或加入国民军。 | 切换玩家到 `SPR` 或 `SPA`，设置 `player_chose_republic` / `player_chose_nationalists` |
| E0002 | `news.scw_outbreak` | NEWS | 新闻事件 | 内战爆发 | 世界得知西班牙军队叛乱、工人武装、共和国分裂。 | 无或仅新闻确认 |
| E0003 | `spr.scw_government_in_crisis` | SPR | 国家事件 | 玩家选择共和国后 | 政府在军队叛乱和街头武装之间寻找权威。 | 设置共和国初始路线 flag |
| E0004 | `spa.scw_junta_forms` | SPA | 国家事件 | 玩家选择国民军后 | 起义军军政府成立，佛朗哥、莫拉、卡瓦内利亚斯、奎波和非洲军团成为焦点。 | 初始化国民军派系分数 |
| E0005 | `hidden.scw_initialize_rebel_state` | HIDDEN | 隐藏事件 | `SPA` 生成后 | 给 `SPA` 补状态、初始 flags、AI 所需基础。 | 修正 `SPA` 可玩性 |
| E0006 | `hidden.scw_cleanup_old_content_flags` | HIDDEN | 隐藏事件 | 新链初始化 | 清理旧西班牙事件残留 flags。 | 避免旧链污染 |

## SPR 战前危机事件

| 编号 | 事件 id | 归属 | 类型 | 触发 | 大纲 | 关键后果 |
|---|---|---|---|---|---|---|
| E0101 | `spr.scw_popular_front_victory` | SPR | 国家事件 | 1936 开局 | 人民阵线胜选，左翼群众期待改革，右翼恐惧失控。 | `spr_popular_front_mandate` 或 `spr_moderate_cabinet` |
| E0102 | `spr.scw_prisoners_released` | SPR | 国家事件 | E0101 后 | 政治犯释放引发左翼欢呼和右翼怒火。 | 稳定变化，`spr_left_mobilized` |
| E0103 | `spr.scw_land_seizures` | SPR | 国家事件 | 1936 春 | 农民占地与地方地主冲突。 | `spr_land_reform_accelerated` 或 `spr_land_reform_delayed` |
| E0104 | `spr.scw_church_conflict` | SPR | 国家事件 | 1936 春 | 教会、学校、共和国世俗化冲突扩大。 | `spr_church_alienated` 或 `spr_church_compromise` |
| E0105 | `spr.scw_falange_street_violence` | SPR | 国家事件 | 1936 春 | 长枪党与左翼民兵街头冲突。 | `spr_falange_crackdown` 或 `spr_street_violence_spreads` |
| E0106 | `spr.scw_officer_transfers` | SPR | 国家事件 | 1936 春 | 政府考虑调离可疑将领。 | 影响政变准备程度 |
| E0107 | `spr.scw_mola_conspiracy` | SPR | 国家事件 | 1936 夏 | 莫拉的密谋浮出水面，但证据不完整。 | `spr_ignored_mola` / `spr_investigated_mola` |
| E0108 | `spr.scw_carlist_mobilization` | SPR | 国家事件 | 1936 夏 | 卡洛斯派在北方秘密集结。 | 影响国民军北方初始力量 |
| E0109 | `spr.scw_calvo_sotelo_assassinated` | SPR | 国家事件 | 1936-07 前 | 卡尔沃·索特洛遇刺，政治空气彻底撕裂。 | 政变风险提升 |
| E0110 | `spr.scw_arm_the_unions_debate` | SPR | 国家事件 | 内战前夕 | 是否向工会发放武器。 | `spr_armed_unions` 或 `spr_refused_to_arm_unions` |
| E0111 | `spr.scw_last_cabinet_meeting` | SPR | 国家事件 | 内战前夕 | 最后的内阁会议：镇压、妥协、等待。 | 决定开战时稳定/民兵/军官忠诚 |
| E0112 | `hidden.scw_prewar_score_republic` | HIDDEN | 隐藏事件 | 内战爆发前 | 汇总战前选择，计算共和国初始状态。 | 设置开战修正 flags |
| E0113 | `spr.scw_azana_reads_the_reports` | SPR | 国家事件 | 1936 春 | 阿萨尼亚阅读军官调动、教堂冲突和街头暴力报告，意识到共和国权威正在漏水。 | 合法性/稳定选择 |
| E0114 | `spr.scw_casares_quiroga_refuses_to_panic` | SPR | 人物互动事件 | 政变风险中高 | 卡萨雷斯·基罗加拒绝把传闻当作战争，普列托和工会代表要求更激烈措施。 | 是否提前打击军官或避免刺激军队 |
| E0115 | `spr.scw_martinez_barrio_last_phone_call` | SPR | 人物互动事件 | 政变爆发前夜 | 马丁内斯·巴里奥尝试给莫拉阵营打最后一通调停电话，但另一端只剩沉默和条件。 | 调停失败，影响开战稳定 |
| E0116 | `spr.scw_prieto_warns_azana` | SPR | 人物互动事件 | 军官阴谋暴露 | 普列托警告阿萨尼亚：共和国既不能信任军队，也不能完全交给街头。 | 温和集中路线或群众武装路线倾向 |
| E0117 | `spr.scw_companys_and_the_unions` | SPR | 人物互动事件 | 加泰紧张高 | 孔帕尼斯在加泰政府和 CNT 之间寻找平衡，巴塞罗那的枪支比命令更有说服力。 | 加泰稳定/CNT 影响变化 |
| E0118 | `spa.prewar_mola_letters_to_the_garrisons` | SPA | 隐藏事件 | 内战前阴谋推进 | 莫拉把密令送往各驻军，要求叛乱必须冷酷、迅速、协调。 | 提高国民军初始组织或政变风险 |
| E0119 | `spa.prewar_franco_hesitates_in_the_canaries` | SPA | 隐藏事件 | 政变前夜 | 佛朗哥在加那利等待更明确的胜算，信件、飞机和死亡消息把他推向叛乱。 | 佛朗哥权威初始值变化 |
| E0120 | `spa.prewar_fal_conde_promises_requetes` | SPA | 隐藏事件 | 卡洛斯派动员 | 法尔·孔德承诺红贝雷将为天主教和王位而战，但要求起义者给出政治回报。 | 北方国民军兵力/卡洛斯派怨恨 |

## SPR 共和国战争路线事件

| 编号 | 事件 id | 归属 | 类型 | 触发 | 大纲 | 关键后果 |
|---|---|---|---|---|---|---|
| E0201 | `spr.scw_arm_the_workers` | SPR | 国家事件 | 内战爆发后 | 发枪给工人，换取民兵和街头控制。 | 民兵增强，稳定下降 |
| E0202 | `spr.scw_militia_committees` | SPR | 国家事件 | E0201 后 | 民兵委员会开始替代国家权力。 | `spr_militia_autonomy` |
| E0203 | `spr.scw_madrid_arsenal` | SPR | 国家事件 | 马德里仍控制 | 武器库归谁控制。 | 影响马德里防御和 CNT 关系 |
| E0204 | `spr.scw_gold_reserves` | SPR | 国家事件 | 1936 秋 | 黄金储备去向：莫斯科、分散、留在国内。 | `spr_gold_to_moscow` / `spr_gold_retained` |
| E0205 | `spr.scw_soviet_advisors_arrive` | SPR | 国家事件 | 接受苏援 | 苏联顾问抵达，带来装备和政治要求。 | `spr_soviet_advisors_empowered` |
| E0206 | `spr.scw_tanks_from_odessa` | SPR | 国家事件 | 苏援线 | 苏联坦克和飞机到港。 | 装备/经验，苏联影响提升 |
| E0207 | `spr.scw_international_brigades` | SPR | 国家事件 | 1936 秋 | 国际纵队成立，理想主义者抵达。 | 人力/战争支持，国际化 flag |
| E0208 | `spr.scw_people_army_reform` | SPR | 国家事件 | 内战持续 | 是否把民兵整编为人民军。 | `spr_people_army_reformed` 或 `spr_militia_autonomy_preserved` |
| E0209 | `spr.scw_militias_resist_discipline` | SPR | 国家事件 | 整编后 | 民兵抵制军纪和军衔。 | CNT 怨恨上升 |
| E0210 | `spr.scw_commissars_at_the_front` | SPR | 国家事件 | 苏联/整编线 | 政治委员制度进入前线。 | 军队组织提升，非共派不满 |
| E0211 | `spr.scw_largo_caballero_government` | SPR | 国家事件 | 1936 后期 | 卡瓦列罗政府试图团结左翼。 | `spr_largo_government` |
| E0212 | `spr.scw_negrin_government` | SPR | 国家事件 | 1937 | 内格林上台，集中抗战。 | `spr_negrin_government`、苏联线加强 |
| E0213 | `spr.scw_prieto_warns_the_cabinet` | SPR | 国家事件 | 苏联影响高 | 普列托警告共和国正在失去自主。 | 可制衡苏联或继续依赖 |
| E0214 | `spr.scw_government_to_valencia` | SPR | 国家事件 | 马德里受威胁 | 政府是否撤往瓦伦西亚。 | `spr_government_fled_madrid` 或士气加成 |
| E0215 | `spr.scw_no_pasaran` | SPR | 国家事件 | 马德里围城 | “他们不会通过”成为共和国口号。 | 马德里防御/战争支持 |
| E0216 | `spr.scw_food_rationing` | SPR | 国家事件 | 战争持续 | 后方粮食配给和城市疲惫。 | 稳定/战争支持取舍 |
| E0217 | `spr.scw_refugees_in_barcelona` | SPR | 国家事件 | 国民军推进 | 难民涌入巴塞罗那。 | 加泰 POP 压力、住房/粮食需求、满意度和激进化变化 |
| E0218 | `spr.scw_war_industry_east` | SPR | 国家事件 | 内战中期 | 工业向东部和加泰转移。 | 迁移部分建筑等级/建造队列，增加铁路和机床瓶颈 |
| E0219 | `spr.scw_casado_plot` | SPR | 国家事件 | 共和国濒危 | 卡萨多派试图结束战争。 | 停战/内部分裂风险 |
| E0220 | `hidden.scw_republic_internal_pressure_tick` | HIDDEN | 隐藏事件 | 周期触发 | 汇总苏联影响、CNT 怨恨、军事中央集权。 | 触发 POUM/CNT/政府危机 |
| E0221 | `spr.scw_largo_and_durruti_argue_at_the_front` | SPR | 人物互动事件 | 卡瓦列罗政府且 CNT 影响高 | 卡瓦列罗要求民兵服从战争纪律，杜鲁蒂反问革命若被锁进军令还剩下什么。 | 整编进度/CNT 怨恨变化 |
| E0222 | `spr.scw_rojo_demands_a_real_army` | SPR | 人物互动事件 | 人民军改革可用 | 比森特·罗霍向内阁展示地图和伤亡数字，说明民兵勇气无法替代参谋、后勤和军纪。 | 人民军改革推动 |
| E0223 | `spr.scw_negrin_and_prieto_over_moscow` | SPR | 人物互动事件 | 苏联影响高 | 内格林主张继续依靠莫斯科坚持战争，普列托担心共和国正在用主权支付弹药。 | 苏联影响/政府团结变化 |
| E0224 | `spr.scw_pasionaria_in_madrid` | SPR | 国家事件 | 马德里受威胁 | 伊巴露丽在马德里广播和集会上把恐惧变成口号，把撤退说成背叛。 | 马德里防御/战争支持 |
| E0225 | `spr.scw_companys_between_cnt_and_valencia` | SPR | 人物互动事件 | 加泰压力高 | 孔帕尼斯面对瓦伦西亚政府和 CNT 委员会双重压力，必须决定谁能在巴塞罗那发号施令。 | 加泰自治/CNT 怨恨/中央权威变化 |
| E0226 | `spr.scw_andres_nin_questioned` | SPR | 人物互动事件 | POUM 被指控前 | 安德烈斯·宁被要求解释 POUM 的立场，苏联顾问、共和国警察和左翼盟友都在房间里。 | POUM 危机进度 |
| E0227 | `spr.scw_azana_writes_from_exhaustion` | SPR | 国家事件 | 战争持续且稳定低 | 阿萨尼亚在信件和日记中记录共和国如何在保卫自身时失去自身。 | 合法性/战争支持取舍 |

## SPA 国民军战争与权力斗争事件

| 编号 | 事件 id | 归属 | 类型 | 触发 | 大纲 | 关键后果 |
|---|---|---|---|---|---|---|
| E0301 | `spa.scw_sanjurjo_plane_crash` | SPA | 国家事件 | 内战初期 | 桑胡尔霍坠机，旧领袖缺席，起义军没有公认的王政门面。 | `spa_sanjurjo_dead`，佛朗哥/莫拉竞争开启 |
| E0302 | `spa.scw_mola_northern_plan` | SPA | 国家事件 | 内战初期 | 莫拉组织北方战线和政变网络，要求政治服从军事委员会。 | `spa_mola_prominent`，军队忠诚上升 |
| E0303 | `spa.scw_franco_crosses_the_strait` | SPA | 国家事件 | 内战初期 | 佛朗哥和非洲军团在德意运输支援下渡过海峡。 | 非洲军团强化，佛朗哥权威上升 |
| E0304 | `spa.scw_army_of_africa` | SPA | 国家事件 | 佛朗哥线 | 非洲军团成为国民军尖刀，也带来严酷镇压和殖民军争议。 | 战力上升，`spa_white_terror_expanded` 风险 |
| E0305 | `spa.scw_radio_seville` | SPA | 国家事件 | 塞维利亚控制 | 奎波·德·利亚诺的电台宣传制造恐惧与胜利幻觉。 | 战争支持/恐怖 flag |
| E0306 | `spa.scw_supreme_command_question` | SPA | 权力斗争事件 | 军政府成立 | 谁领导起义军：佛朗哥、莫拉、合议委员会，或暂缓决定。 | 设置 `spa_franco_supreme_command` 等 |
| E0307 | `spa.scw_junta_of_national_defense` | SPA | 国家事件 | 内战初期 | 国防委员会建立统治架构，卡瓦内利亚斯警惕佛朗哥独裁。 | 政治点/稳定，影响佛朗哥权威 |
| E0308 | `spa.scw_church_crusade` | SPA | 国家事件 | 教会支持 | 教会将战争定义为反无神论十字军。 | `spa_church_crusade`，教会影响上升 |
| E0309 | `spa.scw_jose_antonio_absent` | SPA | 国家事件 | 长枪党影响存在 | 何塞·安东尼奥仍在共和国监狱中，长枪党失去活着的领袖。 | 长枪党分裂风险，烈士叙事准备 |
| E0310 | `spa.scw_falange_martyrdom` | SPA | 国家事件 | 何塞·安东尼奥遇害消息传来 | 长枪党将领袖之死塑造成“缺席者”的神话。 | `spa_falange_myth_of_the_absent`，长枪党权力上升 |
| E0311 | `spa.scw_hedilla_succession_crisis` | SPA | 权力斗争事件 | E0310 后 | 曼努埃尔·赫迪利亚试图掌握长枪党，旧衫派、军队和佛朗哥各自下注。 | 可扶持赫迪利亚、削弱他或让危机扩大 |
| E0312 | `spa.scw_old_shirts_and_new_men` | SPA | 国家事件 | 长枪党权力高 | 老长枪党人痛恨保守派涌入，机会主义者则把蓝衫当作晋升通道。 | 长枪党纯洁性/行政效率取舍 |
| E0313 | `spa.scw_carlist_demands` | SPA | 国家事件 | 北方战线 | 卡洛斯派要求王位承诺、宗教保障和民兵自治。 | `spa_carlist_concessions` 或卡洛斯派怨恨上升 |
| E0314 | `spa.scw_requetes_at_the_front` | SPA | 国家事件 | 卡洛斯派影响存在 | 红贝雷帽民兵在北方表现出色，但拒绝被简单党化。 | 北方战力上升，统一难度上升 |
| E0315 | `spa.scw_ceda_and_alfonsists` | SPA | 国家事件 | 战争中期 | CEDA、阿方索派和旧保守精英希望起义后恢复秩序而非社会革命。 | 右派行政支持，长枪党怨恨 |
| E0316 | `spa.scw_moroccan_recruitment` | SPA | 国家事件 | 非洲军团线 | 扩招摩洛哥兵员，用承诺、军饷和宗教宽容换取战力。 | 人力/政治争议 |
| E0317 | `spa.scw_badajoz_aftermath` | SPA | 国家事件 | 巴达霍斯陷落后 | 暴力镇压的军事和政治后果开始外溢。 | `spa_white_terror_expanded` 或纪律路线 |
| E0318 | `spa.scw_order_in_occupied_zones` | SPA | 国家事件 | 控制州增加 | 占领区治理：恐怖、妥协、教会网络或长枪党地方委员会。 | 稳定/抵抗/人力取舍 |
| E0319 | `spa.scw_german_dependency` | SPA | 国家事件 | 德援较多 | 德国顾问影响空战、装甲和外交期待。 | `spa_german_dependency`，外债/技术收益 |
| E0320 | `spa.scw_italian_dependency` | SPA | 国家事件 | 意援较多 | 意大利要求地中海回报，CTV 也要求宣传胜利。 | `spa_italian_dependency`，意大利影响上升 |
| E0321 | `spa.scw_alcazar_symbol` | SPA | 国家事件 | 托莱多相关 | 阿尔卡萨尔解围被塑造成牺牲、忠诚和西班牙永恒性的神话。 | 佛朗哥权威/战争支持上升 |
| E0322 | `spa.scw_mola_dies_in_air_crash` | SPA | 权力斗争事件 | 1937 或莫拉权威高 | 莫拉坠机身亡，北方军事派突然失去核心。 | 佛朗哥权威大幅上升；若莫拉线强则触发军官不满 |
| E0323 | `spa.scw_unification_decree_drafted` | SPA | 权力斗争事件 | 1937，派系压力高 | 佛朗哥准备把长枪党、卡洛斯派和其他右翼强制合并为 FET y de las JONS。 | 开启统一法令分支 |
| E0324 | `spa.scw_unification_decree` | SPA | 权力斗争事件 | E0323 后 | 统一法令颁布，蓝衫、红贝雷和旧保守派被塞进同一个国家运动。 | `spa_unification_decree`，长枪党/卡洛斯派怨恨变化 |
| E0325 | `spa.scw_hedilla_arrested` | SPA | 权力斗争事件 | 统一法令后且佛朗哥权威高 | 赫迪利亚拒绝服从被捕，真正的长枪党被考迪罗收编。 | `spa_hedilla_purged`，佛朗哥路线锁定倾向 |
| E0326 | `spa.scw_hedilla_compromise` | SPA | 权力斗争事件 | 统一法令后且长枪党权力中高 | 赫迪利亚保住名义职位，接受佛朗哥为军事领袖但保留党务网络。 | `spa_hedilla_compromise`，架空路线可用 |
| E0327 | `spa.scw_falange_seizes_secretariat` | SPA | 权力斗争事件 | 长枪党权力很高 | 长枪党在国民运动秘书处、宣传部和地方委员会中取得决定性优势。 | `spa_falange_secretariat_control`，长枪党胜利倾向 |
| E0328 | `spa.scw_franco_caudillo` | SPA | 权力斗争事件 | 佛朗哥权威足够 | 佛朗哥成为国家元首、总司令和“考迪罗”。 | `spa_franco_caudillo`，佛朗哥路线倾向 |
| E0329 | `spa.scw_caudillo_or_revolution` | SPA | 权力斗争事件 | E0324 后，内战中后期 | 国民军必须回答：胜利后是军人独裁、长枪党革命，还是两者互相利用。 | 决定战后主路线候选 |
| E0330 | `hidden.scw_nationalist_internal_pressure_tick` | HIDDEN | 隐藏事件 | 周期触发 | 汇总佛朗哥权威、长枪党、卡洛斯派、教会影响、外援依赖。 | 触发统一/反弹/架空事件 |
| E0331 | `spa.scw_cabanellas_warns_the_junta` | SPA | 人物互动事件 | E0306 后 | 卡瓦内利亚斯在军政府会议上警告众人：若把权力交给佛朗哥，他不会再归还。 | 佛朗哥权威/军队疑虑变化 |
| E0332 | `spa.scw_franco_and_mola_map_room` | SPA | 人物互动事件 | 莫拉仍活且佛朗哥权威上升 | 佛朗哥和莫拉在地图前讨论北方、马德里和最高统帅权，礼貌语言下全是权力计算。 | 佛朗哥/莫拉分数变化 |
| E0333 | `spa.scw_quiepo_broadcasts_without_permission` | SPA | 人物互动事件 | 塞维利亚控制 | 奎波的电台恐怖宣传提升士气，也让布尔戈斯担心地方将军拥有自己的战争。 | 宣传收益，中央权威下降 |
| E0334 | `spa.scw_yague_after_badajoz` | SPA | 人物互动事件 | E0317 后 | 亚圭面对记者、军官和佛朗哥特使时为巴达霍斯的残酷辩护，胜利的价格第一次被公开看见。 | 白色恐怖/军队纪律变化 |
| E0335 | `spa.scw_hedilla_meets_serrano_suner` | SPA | 人物互动事件 | E0311 后且苏涅尔可用 | 赫迪利亚要求尊重真正长枪党，塞拉诺·苏涅尔则暗示革命必须先学会服从国家。 | 赫迪利亚/苏涅尔派系变化 |
| E0336 | `spa.scw_varela_and_fal_conde` | SPA | 人物互动事件 | 卡洛斯派怨恨高 | 巴雷拉与法尔·孔德讨论红贝雷的未来：是并入国家军队，还是为王位保留刀锋。 | 卡洛斯派整合/军队忠诚 |
| E0337 | `spa.scw_kindelan_questions_the_caudillo` | SPA | 人物互动事件 | 佛朗哥成为考迪罗后 | 金德兰和王党军官私下质疑佛朗哥是否会恢复君主制，还是只借王党清路。 | 王党不满/佛朗哥权威变化 |
| E0338 | `spa.scw_jose_antonio_letters_from_prison` | SPA | 国家事件 | 何塞·安东尼奥仍存活或消息未明 | 阿利坎特监狱中的信件让蓝衫相信领袖仍能归来，也让佛朗哥阵营担心活着的象征。 | 长枪党权力/佛朗哥警惕 |
| E0339 | `spa.scw_franco_receives_news_of_jose_antonio` | SPA | 人物互动事件 | E0310 前后 | 何塞·安东尼奥之死传来，佛朗哥、苏涅尔和长枪党代表都明白：死人比活人更容易被利用。 | 解锁烈士神话/长枪党继承危机 |

## CNT/POUM 事件

| 编号 | 事件 id | 归属 | 类型 | 触发 | 大纲 | 关键后果 |
|---|---|---|---|---|---|---|
| E0401 | `cnt.scw_barcelona_barricades` | CNT | 国家事件 | 内战初期 | CNT 在巴塞罗那街头击败叛军。 | `cnt_barcelona_armed` |
| E0402 | `cnt.scw_collectivized_factories` | CNT | 国家事件 | CNT 影响高 | 工厂集体化。 | `cnt_collectivization_recognized` 或共和国反弹 |
| E0403 | `cnt.scw_rural_communes` | CNT | 国家事件 | CNT 影响高 | 农村公社扩大。 | 革命压力上升 |
| E0404 | `cnt.scw_militias_refuse_orders` | CNT | 国家事件 | 共和国整编 | CNT 民兵拒绝正规化。 | `cnt_refused_integration` |
| E0405 | `spr.scw_poum_accused` | SPR | 国家事件 | 苏联影响高 | POUM 被指控为叛徒。 | `poum_protected` 或 `poum_outlawed` |
| E0406 | `spr.scw_andres_nin_disappears` | SPR | 国家事件 | POUM 被镇压 | 安德烈斯·宁失踪。 | CNT 怨恨上升 |
| E0407 | `spr.scw_poum_trial` | SPR | 国家事件 | POUM 线 | 公开审判或秘密清洗。 | 国际舆论/苏联关系 |
| E0408 | `spr.scw_barcelona_telephone_exchange` | SPR | 国家事件 | CNT 怨恨高 | 电话局危机。 | 触发五月事件风险 |
| E0409 | `spr.scw_may_days` | SPR | 国家事件 | 风险达标 | 巴塞罗那五月事件爆发。 | `barcelona_may_days_triggered` |
| E0410 | `cnt.scw_break_with_the_republic` | CNT | 国家事件 | 五月事件恶化 | CNT 是否公开决裂。 | `cnt_uprising_started` 或 `cnt_uprising_prevented` |
| E0411 | `cnt.scw_durruti_death` | CNT | 国家事件 | 内战中期 | 杜鲁蒂之死。 | 战争支持/革命神话 |
| E0412 | `hidden.scw_cnt_uprising_check` | HIDDEN | 隐藏事件 | 周期触发 | 判断 CNT 是否起义。 | 启动/阻止 CNT 局势 |

P6 第一轮实现状态：`cnt.scw_collectivized_factories`、`cnt.scw_militias_refuse_orders`、`spr.scw_poum_accused`、`spr.scw_poum_trial`、`spr.scw_barcelona_telephone_exchange`、`spr.scw_may_days`、`hidden.scw_cnt_uprising_check` 已落地。`cnt.scw_rural_communes`、`spr.scw_andres_nin_disappears`、`cnt.scw_break_with_the_republic`、`cnt.scw_durruti_death` 保留为长期扩展或 CNT 完整路线内容。五月事件不再由固定日期触发，而由 CNT 怨恨、苏联影响、POUM 处理、人民军整编和共和国革命压力共同触发。

## 国际干涉事件

| 编号 | 事件 id | 归属 | 类型 | 触发 | 大纲 | 关键后果 |
|---|---|---|---|---|---|---|
| E0501 | `ger.scw_condor_legion` | GER | 国家事件 | 内战爆发 | 德国是否派遣秃鹰军团。 | `ger_condor_legion_sent`，援助 `SPA` |
| E0502 | `ger.scw_air_war_laboratory` | GER | 国家事件 | 派遣秃鹰军团后 | 西班牙成为空军实验场。 | 空军经验，国际舆论风险 |
| E0503 | `ger.scw_condor_legion_returns` | GER | 国家事件 | 内战结束 | 秃鹰军团归国总结经验。 | 德国经验/学说收益 |
| E0504 | `ita.scw_send_ctv` | ITA | 国家事件 | 内战爆发 | 意大利派遣志愿军团。 | `ita_ctv_sent`，援助 `SPA` |
| E0505 | `ita.scw_mediterranean_prestige` | ITA | 国家事件 | 意援后 | 墨索里尼把西班牙当威望工程。 | 威望/代价 |
| E0506 | `ita.scw_guadalajara_humiliation` | ITA | 国家事件 | 瓜达拉哈拉后 | 意军失利引发尴尬。 | 政治点/稳定影响 |
| E0507 | `sov.scw_advisors_to_spain` | SOV | 国家事件 | 共和国求援 | 苏联顾问前往西班牙。 | 援助 `SPR`，政治条件 |
| E0508 | `sov.scw_gold_in_moscow` | SOV | 国家事件 | `spr_gold_to_moscow` | 西班牙黄金抵达莫斯科。 | `sov_gold_received` |
| E0509 | `sov.scw_political_conditions` | SOV | 国家事件 | 苏援持续 | 苏联要求共和国处理 POUM/CNT。 | 触发共和国压力 |
| E0510 | `mex.scw_aid_the_republic` | MEX | 国家事件 | 内战爆发 | 墨西哥公开支持共和国。 | `mex_republican_aid` |
| E0511 | `eng.scw_non_intervention_committee` | ENG | 国家事件 | 内战爆发后 | 英国推动不干涉委员会。 | `eng_non_intervention_pressure` |
| E0512 | `fra.scw_border_question` | FRA | 国家事件 | 内战爆发后 | 法国是否开放边境。 | `fra_border_opened` 或边境关闭 |
| E0513 | `por.scw_nationalist_supply_routes` | POR | 国家事件 | 内战爆发后 | 葡萄牙为国民军提供通道。 | `por_nationalist_support` |
| E0514 | `news.scw_foreign_volunteers_questioned` | NEWS | 新闻事件 | 外国干涉高 | 国际社会质疑外国志愿军。 | 新闻确认 |
| E0515 | `por.scw_salazar_and_the_rebels` | POR | 国家事件 | 国民军存在 | 萨拉查把国民军胜利视为伊比利亚反共安全带。 | 葡萄牙支持上升，长期互不侵犯铺垫 |
| E0516 | `eng.scw_watch_gibraltar` | ENG | 国家事件 | 国民军推进南部 | 英国关注直布罗陀安全和地中海航道。 | 影响战后英国态度 |

## 战役和新闻事件

| 编号 | 事件 id | 归属 | 类型 | 触发 | 大纲 | 关键后果 |
|---|---|---|---|---|---|---|
| E0601 | `news.scw_madrid_under_siege` | NEWS | 新闻事件 | 马德里受威胁 | 国民军逼近马德里。 | 新闻 |
| E0602 | `news.scw_madrid_falls` | NEWS | 新闻事件 | 国民军控制马德里 | 马德里陷落。 | 共和国稳定重创 |
| E0603 | `news.scw_alcazar_relief` | NEWS | 新闻事件 | 托莱多相关 | 阿尔卡萨尔围城被宣传为国民军神话。 | 国民军战争支持 |
| E0604 | `news.scw_badajoz_falls` | NEWS | 新闻事件 | 巴达霍斯陷落 | 西部通道被打通。 | 触发 E0317 |
| E0605 | `news.scw_malaga_falls` | NEWS | 新闻事件 | 马拉加陷落 | 共和国南线崩溃。 | 共和国难民压力 |
| E0606 | `news.scw_jarama_battle` | NEWS | 新闻事件 | 哈拉马战役 | 马德里生命线争夺。 | 双方消耗 |
| E0607 | `news.scw_guadalajara_battle` | NEWS | 新闻事件 | 瓜达拉哈拉 | 意大利志愿军受挫。 | 触发 E0506 |
| E0608 | `news.scw_northern_campaign` | NEWS | 新闻事件 | 北方战役开始 | 国民军转向北方工业区。 | 巴斯克自治、矿区/钢铁建筑控制权和北方铁路压力 |
| E0609 | `news.scw_guernica_bombed` | NEWS | 新闻事件 | 格尔尼卡轰炸 | 空袭震动世界。 | 国际舆论 |
| E0610 | `news.scw_bilbao_falls` | NEWS | 新闻事件 | 毕尔巴鄂陷落 | 巴斯克工业区丢失。 | 共和国工业损失 |
| E0611 | `news.scw_santander_falls` | NEWS | 新闻事件 | 桑坦德陷落 | 北方防线继续崩溃。 | 北方压力 |
| E0612 | `news.scw_asturias_last_stand` | NEWS | 新闻事件 | 阿斯图里亚斯陷落前 | 北方矿工最后抵抗。 | 战争支持/失败代价 |
| E0613 | `news.scw_teruel_battle` | NEWS | 新闻事件 | 特鲁埃尔 | 冬季血战。 | 双方消耗 |
| E0614 | `news.scw_aragon_offensive` | NEWS | 新闻事件 | 阿拉贡攻势 | 国民军切开共和国领土。 | 共和国危机 |
| E0615 | `news.scw_mediterranean_cut` | NEWS | 新闻事件 | 国民军到达地中海 | 共和国一分为二。 | 重大危机 |
| E0616 | `news.scw_ebro_battle` | NEWS | 新闻事件 | 埃布罗攻势 | 共和国最后大攻势。 | 双方消耗/结局前奏 |
| E0617 | `spr.scw_international_brigades_last_stand` | SPR | 国家事件 | 埃布罗后 | 国际纵队最后一战和撤离压力。 | 战争支持/国际压力 |
| E0618 | `news.scw_barcelona_falls` | NEWS | 新闻事件 | 巴塞罗那陷落 | 加泰罗尼亚崩溃。 | 共和国终局 |
| E0619 | `news.scw_republican_breakthrough` | NEWS | 新闻事件 | 共和国重大胜利 | 共和国反攻打破悲观预期。 | 国民军压力 |
| E0620 | `hidden.scw_battlefield_news_router` | HIDDEN | 隐藏事件 | 周期触发 | 根据州控制触发战役新闻。 | 防止重复触发 |

## 结局事件：通用与共和国/CNT

| 编号 | 事件 id | 归属 | 类型 | 触发 | 大纲 | 关键后果 |
|---|---|---|---|---|---|---|
| E0701 | `news.scw_nationalist_victory` | NEWS | 新闻事件 | `SPA` 胜利 | 国民军胜利，西班牙进入右翼胜利者的漫长清算。 | 新闻 |
| E0702 | `spa.scw_victory` | SPA | 结局事件 | `SPA` 胜利 | 胜利游行背后，军队、长枪党、教会、卡洛斯派和外援债主开始争夺新国家。 | 根据派系分数进入 E0900/E1000/E1100 系列 |
| E0703 | `spa.scw_repression_after_victory` | SPA | 结局事件 | E0702 后 | 清洗、监狱、流亡和军事法庭成为新政权的第一种语言。 | 稳定/国际关系/抵抗变化 |
| E0704 | `news.scw_republican_victory` | NEWS | 新闻事件 | `SPR` 胜利 | 共和国击败叛乱。 | 新闻 |
| E0705 | `spr.scw_victory` | SPR | 结局事件 | `SPR` 胜利 | 共和国胜利后处理战争遗产。 | 长期扩展路线，本轮只保留清理钩子 |
| E0706 | `spr.scw_trial_of_the_generals` | SPR | 结局事件 | 共和国胜 | 审判叛军将领。 | 稳定/军队忠诚 |
| E0707 | `spr.scw_restore_constitution` | SPR | 结局事件 | 民主合法性高 | 恢复宪政秩序。 | 民主路线 |
| E0708 | `spr.scw_soviet_shadow` | SPR | 结局事件 | 苏联影响高 | 苏联影响笼罩胜利共和国。 | 亲苏路线 |
| E0709 | `spr.scw_cnt_question_after_victory` | SPR | 结局事件 | CNT 未被消灭 | 战后如何处理 CNT。 | 革命/民主分歧 |
| E0710 | `news.scw_cnt_victory` | NEWS | 新闻事件 | CNT 胜利 | 伊比利亚革命震动欧洲。 | 新闻 |
| E0711 | `cnt.scw_victory` | CNT | 结局事件 | CNT 胜利 | CNT 建立革命政权。 | 长期扩展路线，本轮只保留清理钩子 |
| E0712 | `cnt.scw_iberian_commune` | CNT | 结局事件 | CNT 胜利后 | 伊比利亚公社成立。 | 国家精神 |
| E0713 | `cnt.scw_international_isolation` | CNT | 结局事件 | CNT 胜利后 | 革命西班牙遭国际孤立。 | 外交/经济压力 |
| E0714 | `hidden.scw_postwar_cleanup` | HIDDEN | 隐藏事件 | 内战结束 | 清理临时 ideas、单位、flags、局势。 | 稳定战后状态 |

## SPA 战后路线 A：佛朗哥胜利

历史基调：这是最接近真实历史的路线。佛朗哥把长枪党、卡洛斯派、教会、王党、军队和技术官僚都变成可替换的工具。早期带有法西斯化外观和国民天主教动员，二战后转向谨慎中立、反共合法性、外交解冻与技术官僚经济。

| 编号 | 事件 id | 归属 | 类型 | 触发 | 大纲 | 关键后果 |
|---|---|---|---|---|---|---|
| E0901 | `spa.franco_victory_the_caesar_of_burgos` | SPA | 结局事件 | E0702 且 `spa_franco_caudillo` | 布尔戈斯的胜利者不承诺王冠、议会或党内革命，只承诺自己。 | 锁定 `spa_route_franco` |
| E0902 | `spa.franco_law_of_political_responsibilities` | SPA | 国家事件 | 佛朗哥路线 | 政治责任法追溯惩罚共和国支持者，将战时敌人变成法律敌人。 | 镇压强化，抵抗上升 |
| E0903 | `spa.franco_prisons_and_labor_battalions` | SPA | 国家事件 | E0902 后 | 监狱、劳役营和战俘营承担惩罚与重建双重功能。 | 基建/稳定收益，人权声誉恶化 |
| E0904 | `spa.franco_national_catholic_state` | SPA | 国家事件 | `spa_church_crusade` | 教会重返学校、婚姻和公共道德中心，胜利被解释为神意。 | `spa_national_catholicism`，教会影响上升 |
| E0905 | `spa.franco_tame_the_falange` | SPA | 国家事件 | 佛朗哥路线 | 蓝衫、罗马礼和革命口号保留，但长枪党被降格为国民运动的仪式部门。 | 长枪党权力下降，稳定上升 |
| E0906 | `spa.franco_neutralize_the_carlists` | SPA | 国家事件 | 卡洛斯派怨恨高 | 红贝雷被表彰、分化、升官或流放，王位问题继续悬置。 | 卡洛斯派怨恨下降，王党问题延后 |
| E0907 | `spa.franco_burgos_to_madrid` | SPA | 国家事件 | 马德里控制且内战结束 | 政府从战时中心迁入马德里，用胜利游行重写首都记忆。 | 首都/稳定修正 |
| E0908 | `spa.franco_autarky_and_hunger` | SPA | 国家事件 | 战后早期 | 自给自足、配给、黑市和饥饿构成胜利后的日常。 | 进口受限、粮食/燃料/纺织品短缺、配给和黑市压力 |
| E0909 | `spa.franco_axis_temptation` | SPA | 国家事件 | 二战爆发或德意影响高 | 佛朗哥在意识形态、债务、军队疲惫和粮食短缺之间权衡参战。 | 可选亲轴非交战、严格中立、有限合作 |
| E0910 | `spa.franco_hendaye_meeting` | SPA | 国家事件 | 亲轴非交战且德国强势 | 亨达耶会晤：西班牙索要直布罗陀、法属摩洛哥、粮食和燃油，德国嫌价太高。 | 大概率维持非交战，设置外交结果 |
| E0911 | `spa.franco_blue_division` | SPA | 国家事件 | 德苏战争且亲轴/反共强 | 派遣蓝色师团赴东线，以反共热情偿还轴心债务而不正式参战。 | 对德关系上升，国内激进派满足 |
| E0912 | `spa.franco_operation_felix_pressure` | GER | 国家事件 | 德国控制法国且西班牙亲轴 | 德国要求借道攻击直布罗陀，西班牙担忧英国海上封锁。 | 德西谈判，可能触发西班牙危机 |
| E0913 | `spa.franco_allied_blockade_fears` | SPA | 国家事件 | 亲轴过高 | 英美用石油、粮食和海运压力提醒马德里中立的价格。 | 降低亲轴，经济压力 |
| E0914 | `spa.franco_postwar_isolation` | SPA | 国家事件 | 轴心失败后且亲轴记录存在 | 新联合国和战胜国把西班牙视为法西斯残余。 | 外交孤立，贸易惩罚 |
| E0915 | `spa.franco_monarchy_without_a_king` | SPA | 国家事件 | 战后中期 | 西班牙被宣布为王国，但王位空悬，佛朗哥保留指定继承人的权力。 | `spa_kingdom_without_king`，王党安抚 |
| E0916 | `spa.franco_concordat_and_bases` | SPA | 国家事件 | 冷战开始 | 反共价值让梵蒂冈和美国重新考虑西班牙。 | 可触发协定、基地、外交解冻 |
| E0917 | `usa.franco_pact_of_madrid` | NEWS | 新闻事件 | E0916 选择亲美 | 马德里协定让美国基地换来援助，西班牙从孤立走向冷战阵营边缘。 | 新闻，西美关系上升 |
| E0918 | `spa.franco_technocrats_enter` | SPA | 国家事件 | 1950s 或经济危机 | 天主教技术官僚和经济专家要求放弃早期自给自足。 | 开启稳定计划 |
| E0919 | `spa.franco_stabilization_plan` | SPA | 国家事件 | E0918 后 | 经济稳定计划削减管制、吸引外资、压低旧长枪党经济口号。 | 外资/进口机器改善建造队列和建筑投入，长枪党不满 |
| E0920 | `spa.franco_spanish_miracle` | SPA | 结局事件 | 经济改革成功 | 旅游、汇款、外资和工业化制造“西班牙奇迹”，但政治仍被锁死。 | 强经济国家精神，改革压力上升 |
| E0921 | `spa.franco_successor_question` | SPA | 国家事件 | 佛朗哥年迈或长期路线 | 继承问题无法永远回避：卡洛斯派、阿方索派、军队和国民运动都等待答案。 | 选择胡安·卡洛斯、卡洛斯派、摄政继续 |
| E0922 | `spa.franco_order_after_the_caudillo` | SPA | 结局事件 | E0921 后 | 佛朗哥主义最终必须离开佛朗哥本人，制度能否活下去成为新危机。 | 战后尾声/后续模组钩子 |
| E0923 | `spa.franco_cabinet_of_families` | SPA | 权力斗争事件 | E0901 后 | 新内阁不是政党政府，而是军人、长枪党、王党、教会、技术官僚这些“政治家族”的平衡表。 | 设置 `spa_franco_families_balanced`，解锁家族斗争 |
| E0924 | `spa.franco_army_demands_its_reward` | SPA | 权力斗争事件 | 战后早期且军队忠诚高 | 将军们要求军区、预算、荣誉和殖民职位，提醒考迪罗胜利首先由军队赢得。 | 军队忠诚上升，财政压力，长枪党不满 |
| E0925 | `spa.franco_falange_seeks_a_second_revolution` | SPA | 权力斗争事件 | E0905 后且长枪党权力仍高 | 被驯服的蓝衫要求把胜利转化为社会革命，而不是把他们变成阅兵背景。 | 可安抚、边缘化或利用长枪党 |
| E0926 | `spa.franco_ministry_of_governance` | SPA | 国家事件 | E0923 后 | 内政部、民防、警察和省长系统被重组为考迪罗直接控制的国家脊柱。 | 中央集权上升，地方抵抗下降 |
| E0927 | `spa.franco_fundamental_laws_begin` | SPA | 国家事件 | 战后建制 | 政权拒绝宪法，却用一组基本法把独裁包装成有机国家。 | `spa_fundamental_laws`，合法性上升 |
| E0928 | `spa.franco_cortes_espanolas` | SPA | 国家事件 | E0927 后 | 西班牙议会被恢复为咨询机关，由家庭、市镇、工团和任命代表构成。 | 稳定上升，民主吸引力下降 |
| E0929 | `spa.franco_single_movement_not_single_party` | SPA | 国家事件 | E0927 后 | 国民运动被定义为国家共同体，而非普通政党；所有政治只能在它内部呼吸。 | 反对派压制，长枪党权力下降 |
| E0930 | `spa.franco_black_market_and_ration_cards` | SPA | 国家事件 | E0908 后 | 配给卡、黑市和地方关系网吞噬战后经济，胜利者也必须排队。 | 稳定下降，可选择严打或默许 |
| E0931 | `spa.franco_ini_foundation` | SPA | 国家事件 | 自给经济推进 | 国家工业院式机构承担钢铁、能源、造船和军工投资，替私人资本完成其不愿承担的任务。 | 钢铁/能源/造船/军工建筑投资进入建造队列，财政和民生商品承压 |
| E0932 | `spa.franco_vertical_unions_without_revolution` | SPA | 国家事件 | 长枪党被驯服后 | 垂直工会保留长枪党语言，却主要用于控制劳工、调解工资和禁止罢工。 | 工资调解、罢工风险下降，工人满意度和地下抵抗变化 |
| E0933 | `spa.franco_landowners_and_colonization` | SPA | 国家事件 | 农村稳定低 | 政权用灌溉、殖民村和小规模安置安抚农民，同时避免触怒胜利阵营的地主。 | 农村稳定，小幅农业收益 |
| E0934 | `spa.franco_currency_and_war_debts` | SPA | 国家事件 | 战后早期 | 比塞塔、战争债务、外汇短缺和进口管制让新国家从胜利第一天就现金紧张。 | 财政危机或紧缩选择 |
| E0935 | `spa.franco_opus_dei_technocratic_intrigue` | SPA | 权力斗争事件 | E0918 后 | 技术官僚不靠蓝衫口号，而靠数字、银行和外贸报告挑战旧长枪党经济官僚。 | 技术官僚上升，长枪党不满 |
| E0936 | `spa.franco_developmental_state` | SPA | 国家事件 | E0919 后 | 发展计划把独裁、外资、旅游和工业区绑定在一起，政治静止换取经济速度。 | 工业区建造队列、城市 POP 就业、住房/服务需求上升 |
| E0937 | `spa.franco_serrano_suner_rises` | SPA | 权力斗争事件 | 佛朗哥路线早期 | 塞拉诺·苏涅尔把亲轴外交、国民运动整合和法西斯化仪式带进考迪罗身边。 | 亲轴/长枪党外观上升，保守派不安 |
| E0938 | `spa.franco_varela_and_the_generals` | SPA | 权力斗争事件 | 长枪党影响高且军队忠诚低 | 巴雷拉等将军警告佛朗哥：蓝衫可以游行，但不能指挥军队。 | 军队忠诚上升，长枪党受限 |
| E0939 | `spa.franco_jordana_restores_caution` | SPA | 权力斗争事件 | 亲轴压力过高 | 戈麦斯-霍尔达纳推动谨慎外交，试图把西班牙从柏林和罗马的承诺中拖出来。 | 降低亲轴，改善英美关系 |
| E0940 | `spa.franco_carrero_blanco_files` | SPA | 国家事件 | 战后中期 | 卡雷罗·布兰科用备忘录、海军式纪律和行政耐心把考迪罗的意志变成日常统治。 | 行政稳定，个人独裁制度化 |
| E0941 | `spa.franco_martin_artajo_and_the_catholic_window` | SPA | 国家事件 | 战后孤立 | 马丁-阿尔塔霍和天主教外交圈试图证明西班牙是反共天主教国家，而非轴心残余。 | 外交解冻，教会影响上升 |

## SPA 战后路线 B：长枪党胜利

历史基调：这是“真正的蓝衫胜利”。长枪党没有被佛朗哥彻底驯服，而是以赫迪利亚或集体书记处为核心，借战争动员和统一党机器把军队、教会、王党都纳入党国。路线更激进、更意识形态化、更亲轴，也更容易引发经济混乱和保守派反扑。

| 编号 | 事件 id | 归属 | 类型 | 触发 | 大纲 | 关键后果 |
|---|---|---|---|---|---|---|
| E1001 | `spa.falange_victory_blue_dawn` | SPA | 结局事件 | E0702 且长枪党权力最高 | 胜利游行不再属于军帽和主教，而属于蓝衫、轭箭和“何塞·安东尼奥的革命”。 | 锁定 `spa_route_falange` |
| E1002 | `spa.falange_hedilla_or_junta` | SPA | 权力斗争事件 | 长枪党路线 | 新国家必须决定由赫迪利亚领导，还是由长枪党书记处集体领导。 | `spa_hedilla_jefe` 或 `spa_falange_junta` |
| E1003 | `spa.falange_purge_reactionaries` | SPA | 国家事件 | E1001 后 | 长枪党清洗“胜利阵营里的旧西班牙”：王党投机者、保守军官和地方寡头。 | 行政重组，军队忠诚下降 |
| E1004 | `spa.falange_militarize_the_party` | SPA | 国家事件 | 军队忠诚低 | 将前线军官吸收入党，把民兵礼仪正规化，避免军队成为独立政权。 | 军队忠诚回升，党国化 |
| E1005 | `spa.falange_twenty_six_points` | SPA | 国家事件 | 长枪党路线 | 二十六点被宣布为新国家纲领：统一、西班牙使命、反自由主义、反马克思主义。 | 意识形态国家精神 |
| E1006 | `spa.falange_national_syndicalist_revolution` | SPA | 国家事件 | E1005 后 | 国家工团主义不再是口号，行业工团接管劳资调解和经济代表权。 | 开启工团经济分支 |
| E1007 | `spa.falange_vertical_unions` | SPA | 国家事件 | E1006 后 | 垂直工会把工人和雇主关进同一组织，罢工和停工都被定义为反国家。 | 工资表、工会合法性、罢工风险、工人满意度和抵抗变化 |
| E1008 | `spa.falange_nationalize_credit` | SPA | 国家事件 | 工团经济推进 | 依照长枪党纲领，国家控制信用以打击“高利贷”和金融寡头。 | 建设速度/资本外逃取舍 |
| E1009 | `spa.falange_agrarian_settlement` | SPA | 国家事件 | 农村省份稳定低 | 在不承认左翼土改的前提下，党国推动殖民村、合作社和小农安置。 | 农村稳定，地主不满 |
| E1010 | `spa.falange_youth_front` | SPA | 国家事件 | 长枪党路线 | 青年阵线接管教育、体能训练和政治仪式，培养没有共和国记忆的一代。 | 长期稳定，教育成本 |
| E1011 | `spa.falange_seccion_femenina` | SPA | 国家事件 | 长枪党路线 | 女性部门被赋予社会服务、家庭道德和动员职责。 | 人力/稳定，社会保守化 |
| E1012 | `spa.falange_church_subordinated` | SPA | 国家事件 | 教会影响中低 | 国家尊重天主教身份，但拒绝让主教高于党国。 | 教会影响下降，长枪党权力上升 |
| E1013 | `spa.falange_concordat_crisis` | SPA | 国家事件 | E1012 且教会影响高 | 梵蒂冈和西班牙主教警告蓝衫国家不要把十字军变成异教政治宗教。 | 妥协或对抗 |
| E1014 | `spa.falange_crush_carlism` | SPA | 国家事件 | 卡洛斯派怨恨高 | 卡洛斯派拒绝蓝衫独占胜利，长枪党选择镇压、流放或吸收。 | 北方抵抗/统一强化 |
| E1015 | `spa.falange_iberian_mission` | SPA | 国家事件 | 长枪党路线 | 泛西班牙主义、直布罗陀、摩洛哥和葡萄牙问题进入宣传核心。 | 宣称/外交紧张 |
| E1016 | `spa.falange_axis_alignment` | SPA | 国家事件 | 二战爆发 | 蓝衫政府比佛朗哥更愿意加入轴心秩序，但国家仍缺粮、缺油、缺海军。 | 加入轴心/非交战/志愿军扩大 |
| E1017 | `spa.falange_take_gibraltar_question` | SPA | 国家事件 | 亲轴且英国受压 | 长枪党要求用参战换取直布罗陀和北非补偿。 | 战争风险，英国敌意 |
| E1018 | `spa.falange_volunteers_against_bolshevism` | SPA | 国家事件 | 德苏战争 | 反布尔什维克志愿军被塑造成欧洲十字军中的西班牙先锋。 | 对德关系，老兵政治资本 |
| E1019 | `spa.falange_economic_overreach` | SPA | 国家事件 | 工团制度推进过快 | 党干部、工团官僚和粮食短缺让国家工团主义陷入效率危机。 | 选择放缓、强推、技术官僚修正 |
| E1020 | `spa.falange_blue_terror` | SPA | 国家事件 | 反对派抵抗高 | 蓝衫安全机关把战后清算扩大到“消极者”和“资产阶级破坏者”。 | 恐怖加成，国际恶名 |
| E1021 | `spa.falange_after_axis_defeat` | SPA | 国家事件 | 轴心失败 | 激进蓝衫政权面对战胜国敌意，必须伪装、抵抗或继续孤立。 | 战后孤立强度 |
| E1022 | `spa.falange_revolution_besieged` | SPA | 结局事件 | 长枪党路线稳定后 | 西班牙成为欧洲最后的蓝衫革命国家，既不是传统军政府，也不是完全照搬的德意法西斯。 | 最终国家精神/后续钩子 |
| E1023 | `spa.falange_council_of_the_revolution` | SPA | 权力斗争事件 | E1001 后 | 长枪党全国委员会要求成为最高政治机关，军方则拒绝让前线胜利听命于党务秘书。 | 设置革命委员会或军党妥协 |
| E1024 | `spa.falange_hedilla_faces_the_generals` | SPA | 权力斗争事件 | `spa_hedilla_jefe` | 赫迪利亚必须在清洗将领、收买将领、或让军队保留自治之间选择。 | 军队忠诚/党权力大幅变化 |
| E1025 | `spa.falange_old_shirts_against_careerists` | SPA | 权力斗争事件 | 长枪党路线 | 老衫派指控新加入者把革命变成官职交易，党内纯洁性危机爆发。 | 可清党、妥协或扩编官僚 |
| E1026 | `spa.falange_constitution_of_the_new_state` | SPA | 国家事件 | E1005 后 | 新国家宪章把党、国家、工团和领袖合为一体，否定自由议会和阶级政党。 | `spa_falangist_new_state` |
| E1027 | `spa.falange_party_above_ministries` | SPA | 国家事件 | E1026 后 | 各部大臣必须接受党监察，国家机器被迫承认长枪党路线高于行政惯例。 | 行政党化，官僚阻力 |
| E1028 | `spa.falange_provincial_jefes` | SPA | 国家事件 | E1027 后 | 省级党魁接管地方任命、粮食动员和政治审查，旧地方寡头失去保护伞。 | 地方控制上升，抵抗风险 |
| E1029 | `spa.falange_revolutionary_justice` | SPA | 国家事件 | E1020 后 | 革命法庭审判共和国残余、保守破坏者和“投机资产阶级”。 | 恐怖/没收/国际恶名 |
| E1030 | `spa.falange_syndicalist_planning_board` | SPA | 国家事件 | E1006 后 | 国家工团经济委员会开始制定产量、工资、投资和配给指标。 | 计划经济效率或官僚膨胀 |
| E1031 | `spa.falange_expropriate_the_absent_enemy` | SPA | 国家事件 | E1029 后 | 逃亡共和国派、被清洗资本家和敌对地方精英的财产被划入党国经济。 | 建筑 ownership/财政资产变化，资本家 POP 激进化和地方稳定下降 |
| E1032 | `spa.falange_worker_producer_militias` | SPA | 国家事件 | 工团制度推进 | 工人被组织为“生产民兵”，荣誉、配给和惩罚共同驱动工厂纪律。 | 工人出勤、工资/配给优先权、疲惫和事故/抵抗风险变化 |
| E1033 | `spa.falange_battle_for_bread` | SPA | 国家事件 | 战后粮食短缺 | 蓝衫国家可以夺粮、配给、进口或向农村妥协，但不能承认革命无法喂饱人民。 | 粮食危机选择 |
| E1034 | `spa.falange_credit_nationalization_crisis` | SPA | 权力斗争事件 | E1008 后 | 银行家、军需商和党经济官僚围绕信用国有化爆发暗战。 | 金融控制/资本外逃取舍 |
| E1035 | `spa.falange_autarkic_mobilization` | SPA | 国家事件 | E1030 后 | 自给自足被包装为民族意志，钢铁、煤炭、军工和交通优先于消费。 | 钢铁/煤炭/军工订单优先，民生商品供给和 POP 满意度承压 |
| E1036 | `spa.falange_technicians_under_blue_supervision` | SPA | 国家事件 | 经济危机 | 工程师和会计师被召回，但他们必须在蓝衫监察员面前证明自己服务革命。 | 经济效率上升，意识形态冲突 |
| E1037 | `spa.falange_corporate_empire_of_the_state` | SPA | 结局事件 | 工团经济稳定 | 一个由党、工团、国企和政治法庭支撑的新国家已经形成，代价是社会被永久动员。 | 长枪党国家最终经济精神 |
| E1038 | `spa.falange_hedilla_jefe_nacional` | SPA | 权力斗争事件 | E1002 选择赫迪利亚 | 赫迪利亚以何塞·安东尼奥继承人的名义成为全国领袖，但旧衫派、苏涅尔和军人都怀疑他能否统治国家。 | 设置 `spa_hedilla_jefe_nacional` |
| E1039 | `spa.falange_serrano_suner_architect` | SPA | 权力斗争事件 | 长枪党路线 | 塞拉诺·苏涅尔起草国家法制和外交路线，试图成为蓝衫革命的黎塞留。 | 法制效率上升，赫迪利亚警惕 |
| E1040 | `spa.falange_fernandez_cuesta_party_machine` | SPA | 国家事件 | E1026 后 | 费尔南德斯-奎斯塔接管党务机器，把理想、纪律和任命表绑在一起。 | 党组织力上升，地方自主下降 |
| E1041 | `spa.falange_giron_ministry_of_labor` | SPA | 国家事件 | E1007 后 | 何塞·安东尼奥·吉隆把劳工部变成垂直工会、福利和社会民粹主义的核心。 | 工人稳定上升，财政压力 |
| E1042 | `spa.falange_arrese_doctrine_of_the_state` | SPA | 国家事件 | E1026 后 | 阿雷塞要求新国家不只是反共军政府，而必须有长枪党教义、仪式和成文原则。 | 意识形态纯度上升，保守派不满 |
| E1043 | `spa.falange_ridruejo_propaganda_front` | SPA | 国家事件 | 长枪党路线 | 里德鲁埃霍用诗歌、广播、青年刊物和烈士叙事把胜利写成革命神话。 | 宣传强度上升，长期可触发幻灭 |
| E1044 | `spa.falange_pilar_social_service` | SPA | 国家事件 | 长枪党路线 | 皮拉尔·普里莫·德里维拉将女性部门、社会救济和家庭道德塑造成新国家的温柔面孔。 | 稳定/人力，社会保守化 |
| E1045 | `spa.falange_yague_the_blue_general` | SPA | 权力斗争事件 | 军队忠诚低 | 亚圭作为亲长枪党军人被推到前台，用非洲军团威望证明蓝衫与军队可以共存。 | 军队党化上升，传统军官不满 |
| E1046 | `spa.falange_aznar_and_davila_old_shirt_pressure` | SPA | 权力斗争事件 | 旧衫派不满 | 阿斯纳尔和桑乔·达维拉代表的旧衫派要求更激进的清洗、更少妥协和更多革命职位。 | 激进派压力上升，可清洗或安抚 |
| E1047 | `spa.falange_government_of_blue_victory` | SPA | 结局事件 | E1038 或 E1023 后 | 赫迪利亚、苏涅尔、费尔南德斯-奎斯塔、吉隆、阿雷塞、里德鲁埃霍、皮拉尔和亚圭组成蓝衫胜利政府。 | 确立长枪党政府班底 |

## SPA 战后路线 C：长枪党架空佛朗哥与制度化国家工团主义

历史基调：这是更特殊的架空路线。佛朗哥仍是考迪罗、总司令和胜利象征，但党国机器逐步把他变成仲裁者和图腾。长枪党不公开推翻佛朗哥，而是通过秘书处、垂直工会、青年组织、宣传、地方任命和经济机关，将“国家工团主义”制度化为真正的国家结构。它比路线 B 更隐蔽、更官僚、更可持续，也更像 TNO 式“制度吃掉个人”的路线。

| 编号 | 事件 id | 归属 | 类型 | 触发 | 大纲 | 关键后果 |
|---|---|---|---|---|---|---|
| E1101 | `spa.syndicalist_shadow_the_caudillo_and_the_secretariat` | SPA | 结局事件 | E0702 且 `spa_hedilla_compromise` 或 `spa_falange_secretariat_control` | 佛朗哥站在阳台上接受欢呼，真正的任命名单却从国民运动秘书处流出。 | 锁定 `spa_route_syndicalist_shadow` |
| E1102 | `spa.syndicalist_dual_power_settlement` | SPA | 权力斗争事件 | E1101 后 | 军队承认佛朗哥，党控制社会；双方达成不写进宪章的双重权力协议。 | 佛朗哥权威保留，工团制度化上升 |
| E1103 | `spa.syndicalist_control_the_movement_council` | SPA | 国家事件 | 架空路线 | 国民运动全国委员会被改造成路线审查机关，而非佛朗哥的橡皮图章。 | `spa_movement_council_empowered` |
| E1104 | `spa.syndicalist_caudillo_as_symbol` | SPA | 国家事件 | 佛朗哥权威高 | 宣传把佛朗哥塑造成胜利和统一象征，同时避免他直接干预党务细节。 | 稳定上升，个人权威转化为制度合法性 |
| E1105 | `spa.syndicalist_appointments_from_below` | SPA | 国家事件 | 秘书处控制 | 省长、市长和工团代表由党网络筛选，再送交佛朗哥批准。 | 地方控制上升，军队不满 |
| E1106 | `spa.syndicalist_vertical_syndicates_charter` | SPA | 国家事件 | E1102 后 | 垂直工团宪章规定每个行业的代表、仲裁、福利和生产目标。 | `spa_vertical_syndicates_charter` |
| E1107 | `spa.syndicalist_labor_magistrates` | SPA | 国家事件 | E1106 后 | 劳资纠纷交由国家工团法庭裁决，阶级斗争被定义为对国家统一的犯罪。 | 稳定/生产组织 |
| E1108 | `spa.syndicalist_employers_inside_the_state` | SPA | 国家事件 | 工团制度化中 | 企业主不被消灭，而是被迫成为国家计划的合作者和可惩罚的管理人。 | 工业效率/资本不满取舍 |
| E1109 | `spa.syndicalist_worker_welfare_without_freedom` | SPA | 国家事件 | E1107 后 | 工团国家提供住房、保险、食堂和荣誉，却禁止独立工会和罢工。 | 稳定上升，自由度下降 |
| E1110 | `spa.syndicalist_institute_of_industry` | SPA | 国家事件 | 经济重建 | 国家工业院式机构被提前赋予更强计划权，集中发展钢铁、造船、化工和军工。 | 钢铁/造船/化工/军工建筑投资、政府订单和财政压力上升 |
| E1111 | `spa.syndicalist_credit_and_corporations` | SPA | 国家事件 | 国家工团主义推进 | 信贷、商会和大企业被绑定进国家生产委员会。 | 建设与通胀取舍 |
| E1112 | `spa.syndicalist_rural_brotherhoods` | SPA | 国家事件 | 农村稳定低 | 农村兄弟会把地主、佃农、教士和党代表编入同一地方组织。 | 农村稳定，改革有限 |
| E1113 | `spa.syndicalist_church_compact` | SPA | 国家事件 | 教会影响高 | 党承认西班牙天主教身份，教会承认工团国家对社会组织的最高权威。 | 教会妥协，国民天主教弱化为仪式支柱 |
| E1114 | `spa.syndicalist_old_shirts_bureaucrats` | SPA | 国家事件 | 制度化高 | 曾经街头斗殴的老衫派变成表格、印章和任命权的守门人。 | 行政效率上升，革命热情下降 |
| E1115 | `spa.syndicalist_army_kept_in_barracks` | SPA | 国家事件 | 军队忠诚低 | 通过军费、荣誉、殖民职位和反共任务安抚军队，阻止将领干预党国。 | 军队忠诚恢复，预算压力 |
| E1116 | `spa.syndicalist_security_against_both_red_and_black` | SPA | 国家事件 | 反对派活动 | 安全部门同时监控左翼地下组织、卡洛斯派传统主义者和不服从的蓝衫激进派。 | 抵抗下降，派系紧张 |
| E1117 | `spa.syndicalist_foreign_policy_of_useful_ambiguity` | SPA | 国家事件 | 二战爆发 | 对外展示反共和法西斯同情，对内避免把国家命运完全交给柏林或罗马。 | 可走谨慎亲轴、武装中立、反共志愿军 |
| E1118 | `spa.syndicalist_blue_division_as_pressure_valve` | SPA | 国家事件 | 德苏战争 | 把最激进的蓝衫送往东线，用志愿军释放国内革命压力。 | 激进派满意，老兵后续风险 |
| E1119 | `spa.syndicalist_postwar_rebranding` | SPA | 国家事件 | 轴心失败后 | 政权把自己包装成天主教、反共、社团主义，而不是失败的法西斯复制品。 | 降低孤立，需压制旧口号 |
| E1120 | `spa.syndicalist_technocrats_absorbed` | SPA | 国家事件 | 经济瓶颈 | 技术官僚被允许进入计划机关，但必须穿上国民运动的制度外衣。 | 经济效率上升，意识形态纯度下降 |
| E1121 | `spa.syndicalist_institutional_state` | SPA | 结局事件 | `spa_syndicalist_institutionalization` 高 | 个人独裁尚在，党国却已学会不依赖个人呼吸。国家工团主义成为制度，而非演讲。 | 强国家精神，解锁长期路线 |
| E1122 | `spa.syndicalist_when_franco_fades` | SPA | 结局事件 | 佛朗哥年迈或死亡 | 当考迪罗退场，秘书处、工团、军队和教会争夺“谁继承制度解释权”。 | 后续继承危机钩子 |
| E1123 | `spa.syndicalist_secretariat_vs_the_palace` | SPA | 权力斗争事件 | E1103 后 | 秘书处开始绕过佛朗哥私人幕僚，宫廷、家族和老军官发现自己正在被制度排挤。 | 秘书处权力上升，佛朗哥权威下降 |
| E1124 | `spa.syndicalist_carrero_or_the_blue_bureaucrats` | SPA | 权力斗争事件 | 架空路线中期 | 海军军官、保守行政派和蓝衫官僚争夺“谁替考迪罗过滤国家”。 | 选择保守缓冲或蓝衫官僚优势 |
| E1125 | `spa.syndicalist_generals_draw_a_line` | SPA | 权力斗争事件 | 制度化过快且军队忠诚低 | 将军们警告秘书处：党可以管理社会，但不能任命军队的灵魂。 | 军党危机，可能降速制度化 |
| E1126 | `spa.syndicalist_law_of_national_organization` | SPA | 国家事件 | E1102 后 | 国家组织法把考迪罗、国民运动、工团、军队和教会的权限写成可执行秩序。 | `spa_national_organization_law` |
| E1127 | `spa.syndicalist_cortes_of_corporations` | SPA | 国家事件 | E1126 后 | 咨询议会由家庭、市镇、工团、职业团体和国民运动代表组成，投票让位于归类。 | 社团议会，合法性上升 |
| E1128 | `spa.syndicalist_registry_of_social_bodies` | SPA | 国家事件 | E1127 后 | 所有合法协会、工会、商会、学生组织和慈善机构必须登记为国家共同体的一部分。 | 社会控制上升，地下反对派转入秘密 |
| E1129 | `spa.syndicalist_party_school_of_administrators` | SPA | 国家事件 | E1114 后 | 党校不再只培养演说者，而是培养会计、监察员、劳动裁判和地方行政官。 | 行政效率上升，制度化上升 |
| E1130 | `spa.syndicalist_budgetary_corporatism` | SPA | 国家事件 | E1110 后 | 预算不再只按部委划分，而按行业、区域和国家优先目标分配。 | 计划效率/地方不满 |
| E1131 | `spa.syndicalist_three_year_reconstruction_plan` | SPA | 国家事件 | 战后重建 | 三年重建计划优先修复铁路、港口、电网和军工，同时把劳工福利写入生产目标。 | 基建/工业上升，消费不足 |
| E1132 | `spa.syndicalist_wage_tables_and_price_boards` | SPA | 国家事件 | E1107 后 | 工资表和价格委员会试图替代市场谈判，防止通胀、罢工和雇主锁厂。 | 通胀控制，黑市风险 |
| E1133 | `spa.syndicalist_cooperatives_under_command` | SPA | 国家事件 | 农村兄弟会后 | 合作社被鼓励，但必须服从地方工团和生产指标。 | 农业组织上升，农民自主下降 |
| E1134 | `spa.syndicalist_black_market_purge` | SPA | 国家事件 | 黑市或粮食短缺 | 秘书处把黑市定义为反国家经济，但严打会暴露配给体系本身的失败。 | 稳定/粮食/恐怖取舍 |
| E1135 | `spa.syndicalist_bankers_accept_the_yoke` | SPA | 权力斗争事件 | E1111 后 | 银行界接受轭箭不是出于信仰，而是因为贷款许可、外汇和警察都在国家手里。 | 信用控制上升，投资效率变化 |
| E1136 | `spa.syndicalist_planned_opening` | SPA | 国家事件 | 战后孤立缓和或经济瓶颈 | 经济机关尝试有限开放：引进机器、技术和外汇，但由工团国家决定谁能接触世界。 | 外资/技术收益，意识形态风险 |
| E1137 | `spa.syndicalist_institution_over_charisma` | SPA | 结局事件 | E1121 后 | 机关、工团、法庭、党校和预算程序开始比任何演讲更有力量。西班牙被制度重写。 | 最终制度化国家精神 |
| E1138 | `spa.syndicalist_hedilla_in_the_secretariat` | SPA | 权力斗争事件 | `spa_hedilla_compromise` | 赫迪利亚不再公开挑战佛朗哥，而是在秘书处内保留“正统长枪党”的否决权。 | 长枪党合法性上升，佛朗哥权威微降 |
| E1139 | `spa.syndicalist_fernandez_cuesta_keeps_the_registers` | SPA | 国家事件 | E1103 后 | 费尔南德斯-奎斯塔掌握党员、干部、地方任命和纪律档案，纸面权力开始超过演讲权力。 | 制度化上升 |
| E1140 | `spa.syndicalist_giron_builds_the_social_state` | SPA | 国家事件 | E1106 后 | 吉隆用劳工福利、住房、保险和工团荣誉让国家工团主义获得社会触感。 | 工人稳定上升，财政压力 |
| E1141 | `spa.syndicalist_arrese_writes_the_organic_formula` | SPA | 国家事件 | E1126 后 | 阿雷塞把考迪罗、国民运动和垂直工团写成一套有机国家公式。 | 建制合法性上升，教义化 |
| E1142 | `spa.syndicalist_serrano_suner_contained` | SPA | 权力斗争事件 | 塞拉诺·苏涅尔影响高 | 秘书处利用苏涅尔的法律才能，却阻止他把自己变成第二个权力中心。 | 法制收益，个人派系下降 |
| E1143 | `spa.syndicalist_carrero_as_conservative_filter` | SPA | 权力斗争事件 | E1124 选择保守缓冲 | 卡雷罗·布兰科成为军队、宫廷和秘书处之间的过滤器，确保制度推进不触发军人反扑。 | 军队忠诚上升，制度化放缓 |
| E1144 | `spa.syndicalist_pilar_and_the_social_surface` | SPA | 国家事件 | 架空路线 | 皮拉尔的女性部门和社会服务网络为冰冷的工团国家提供家庭、慈善和道德外观。 | 稳定上升，社会保守化 |
| E1145 | `spa.syndicalist_ridruejo_private_doubts` | SPA | 国家事件 | 制度化高 | 里德鲁埃霍开始怀疑革命是否已经变成档案柜里的服从机器。 | 宣传收益下降，可触发异议人物线 |
| E1146 | `spa.syndicalist_government_of_the_hidden_machine` | SPA | 结局事件 | E1121 后 | 佛朗哥签字，卡雷罗过滤，赫迪利亚背书，费尔南德斯-奎斯塔登记，吉隆分配福利，阿雷塞写下原则。机器终于有了面孔。 | 确立架空路线政府班底 |

## 国民军战后外交与债务事件

| 编号 | 事件 id | 归属 | 类型 | 触发 | 大纲 | 关键后果 |
|---|---|---|---|---|---|---|
| E1201 | `ger.scw_claims_after_nationalist_victory` | GER | 国家事件 | 德援且国民军胜 | 德国要求矿产、基地便利、外交靠拢或偿债。 | 西德关系/贸易 |
| E1202 | `ita.scw_claims_after_nationalist_victory` | ITA | 国家事件 | 意援且国民军胜 | 意大利要求地中海回报、宣传胜利和经济补偿。 | 西意关系 |
| E1203 | `spa.postwar_tungsten_and_debt` | SPA | 国家事件 | 德援较多 | 钨矿、贸易协定和战争债务让西班牙难以完全中立。 | 对德贸易/英美压力 |
| E1204 | `spa.postwar_portuguese_pact` | SPA | 国家事件 | 国民军胜且葡萄牙支持 | 伊比利亚两大右翼政权建立互不侵犯和反共合作。 | 对葡关系，边境稳定 |
| E1205 | `eng.postwar_spain_and_gibraltar` | ENG | 国家事件 | 国民军胜 | 英国评估直布罗陀、海峡和西班牙亲轴风险。 | 英西关系 |
| E1206 | `spa.postwar_morocco_question` | SPA | 国家事件 | 国民军胜 | 非洲军团的胜利记忆让摩洛哥保护地和殖民军地位变得敏感。 | 殖民政策/人力 |
| E1207 | `news.spain_isolated_after_axis_defeat` | NEWS | 新闻事件 | 西班牙亲轴且轴心失败 | 战后国际社会孤立西班牙右翼政权。 | 新闻 |
| E1208 | `news.spain_returns_to_diplomacy` | NEWS | 新闻事件 | 西班牙与西方和解 | 反共冷战让马德里重新获得外交空间。 | 新闻 |
| E1209 | `ger.spain_berlin_after_france` | GER | 国家事件 | 1941、法国被德国击败、国民军西班牙存活且未与德国交战 | 法国崩溃后，柏林重新审视伊比利亚：西班牙的港口、铁路和直布罗陀方向比内战债务更有价值。 | 解锁对西班牙大交易 |
| E1210 | `ger.spain_offer_axis_reconstruction_pact` | GER | 国家事件 | E1209 后 | 德国向新西班牙提出轴心重建协定：贷款、机器、军工设备和顾问，换取参战、基地和战略通道。 | 发送德国提案给 `SPA` |
| E1211 | `spa.axis_reconstruction_pact_arrives` | SPA | 权力斗争事件 | 德国发送 E1210 | 马德里收到柏林提案；苏涅尔、赫迪利亚、吉隆、卡雷罗、巴雷拉和财政官员围绕“援助换战争”爆发争论。 | 选择接受、讨价还价或拒绝 |
| E1212 | `spa.axis_pact_the_price_of_recovery` | SPA | 人物互动事件 | E1211 选择谈判 | 德国代表列出数字：煤、钢、机床、贷款、军火厂；西班牙代表读到附页：基地、参战、情报站和港口优先权。 | 明确援助包和主权代价 |
| E1213 | `spa.axis_pact_serrano_suner_sells_the_future` | SPA | 人物互动事件 | 佛朗哥或架空路线且苏涅尔影响高 | 塞拉诺·苏涅尔把协定包装成“欧洲新秩序中的西班牙复兴”，卡雷罗和军人质疑这是复兴还是抵押。 | 亲轴上升，军队/保守派疑虑 |
| E1214 | `spa.axis_pact_hedilla_demands_revolutionary_aid` | SPA | 人物互动事件 | 长枪党路线 | 赫迪利亚和吉隆要求德国援助首先进入垂直工会、工人住房和国营军工，而不是旧银行和将军仓库。 | 长枪党经济优先，德国不满或妥协 |
| E1215 | `spa.axis_pact_acceptance` | SPA | 国家事件 | 接受德国重建协定 | 西班牙正式加入轴心战争体系，开放基地、港口和铁路，德国援助开始装船。 | 加入轴心/战争，设置 `spa_axis_reconstruction_pact` |
| E1216 | `news.spain_joins_axis_war` | NEWS | 新闻事件 | E1215 | 新西班牙加入轴心战争，柏林宣布伊比利亚将成为欧洲新秩序的西南堡垒。 | 新闻，英国/美国敌意上升 |
| E1217 | `eng.spain_axis_bases_alert` | ENG | 国家事件 | E1215 | 英国参谋部评估德国飞机、潜艇和侦察站进入西班牙基地后，直布罗陀和大西洋航线面临的新威胁。 | 英国制裁/袭击/防御准备 |
| E1218 | `spa.german_aid_first_shipments` | SPA | 国家事件 | E1215 后 30-60 天 | 第一批德国机器、煤炭、卡车、药品和军工顾问抵达毕尔巴鄂、巴塞罗那或加的斯。 | 工业修复、基建修复、德国影响上升 |
| E1219 | `spa.german_credits_for_reconstruction` | SPA | 国家事件 | E1218 后 | 德国信贷注入财政，马德里可以支付进口、恢复铁路、重开兵工、弹药、机床和车辆工厂，但还款将以矿产、港口优先权和政治服从计价。 | 财政现金流、进口订单和建造队列缓解，长期债务/矿产贸易承诺上升 |
| E1220 | `spa.retooling_the_armories` | SPA | 国家事件 | 德援到账且战争中 | 德国顾问帮助重整托莱多、特鲁维亚、毕尔巴鄂等军工体系，西班牙军队开始接收标准化武器和弹药线。 | 修复军工建筑等级、切换军工 PM、增加装备/弹药库存和德国影响 |
| E1221 | `spa.recovery_of_the_workshops` | SPA | 国家事件 | 德援到账 | 机床和煤炭让停摆的车间重新发声，城市工人第一次把轴心协定与工资袋联系起来。 | 机床/煤炭投入缓解，机械/民生建筑复工，就业和工资改善 |
| E1222 | `spa.bread_coal_and_ration_cards` | SPA | 国家事件 | 德援到账且配给危机 | 援助缓解煤炭、运输和药品短缺，配给卡仍存在，但黑市价格开始回落。 | 粮食/稳定改善，黑市下降 |
| E1223 | `spa.german_advisors_in_the_ministries` | SPA | 权力斗争事件 | E1219 后 | 德国顾问不再只在工厂和机场，他们出现在财政部、军工委员会和港口调度室。 | 经济效率上升，主权/派系不满 |
| E1224 | `spa.recovery_with_a_hook_in_its_mouth` | SPA | 结局事件 | 德援多次到账 | 西班牙经济开始复苏：烟囱重新冒烟、军工订单回流、铁路恢复运转。但复苏的账单写在柏林。 | 强复苏国家精神，德国依赖锁定 |
| E1225 | `spa.axis_pact_rejects_the_bargain` | SPA | 国家事件 | E1211 选择拒绝 | 马德里拒绝用基地和战争换取援助，德国将西班牙视为忘恩负义的债务国。 | 德西关系下降，经济危机持续 |
| E1226 | `ger.spain_punish_refusal` | GER | 国家事件 | E1225 | 柏林考虑减少贸易、催收内战债务或扶持西班牙内部亲轴派。 | 对西施压，可能触发阴谋 |
| E1227 | `news.axis_victory_in_the_east` | NEWS | 新闻事件 | 1945、德国占领莫斯科/列宁格勒/斯大林格勒或苏联投降 | 苏联在三大城市陷落后被迫议和，东欧全部割让给德国势力范围。 | 新闻，轴心胜利阶段开启 |
| E1228 | `ger.eastern_settlement_and_the_next_front` | GER | 国家事件 | E1227 后 | 柏林把东方胜利转化为新的战略：迫使英国承认欧洲新秩序，地中海成为下一张谈判牌。 | 解锁地中海总攻 |
| E1229 | `tur.axis_entry_after_the_soviet_collapse` | TUR | 国家事件 | E1227 后且土耳其亲轴 | 苏联崩溃后，土耳其加入轴心战争，寻求黑海、高加索与中东新秩序中的位置。 | 土耳其加入轴心，开启中东战线 |
| E1230 | `spa.axis_call_for_the_final_mediterranean_campaign` | SPA | 国家事件 | E1215 且 E1227 后 | 德国要求西班牙兑现协定：开放基地、派出军队、协助夺取直布罗陀并压迫英国地中海生命线。 | 西班牙进入最后地中海战役 |
| E1231 | `spa.operation_felix_at_last` | SPA | 国家事件 | E1230 后 | 被推迟多年的直布罗陀计划终于执行，西班牙炮兵、德国工兵和空军顾问共同压向岩山。 | 直布罗陀战役，英西关系彻底破裂 |
| E1232 | `news.gibraltar_falls_to_spain` | NEWS | 新闻事件 | 轴心控制直布罗陀 | 直布罗陀陷落，英国在地中海西门的锁被西班牙和德国撬开。 | 新闻，英国战争支持下降 |
| E1233 | `spa.the_rock_returns` | SPA | 结局事件 | E1232 后 | 马德里宣布直布罗陀回归西班牙，胜利游行把数百年的屈辱压缩成一天的钟声。 | 获得/核心化直布罗陀，民族主义高涨 |
| E1234 | `news.axis_armies_enter_cairo` | NEWS | 新闻事件 | 轴心控制开罗 | 德意土西多路压力下，开罗陷落，英国帝国的中东支柱断裂。 | 新闻，英国求和倾向上升 |
| E1235 | `news.sinai_and_the_canal_cut` | NEWS | 新闻事件 | 轴心控制西奈/苏伊士 | 西奈半岛和苏伊士通道落入轴心手中，印度洋与地中海的帝国脐带被切断。 | 英国危机，轴心谈判优势 |
| E1236 | `eng.request_the_glorious_peace` | ENG | 国家事件 | 直布罗陀、开罗、苏伊士失守且轴心控制欧洲大陆 | 英国内阁承认继续战争只会失去帝国剩余部分，开始请求一场体面的“光荣和平”。 | 触发和平谈判 |
| E1237 | `news.the_glorious_peace` | NEWS | 新闻事件 | E1236 后 | 英国与轴心签署“光荣和平”，承认欧洲大陆新秩序并退出主要战场。 | 世界格局重写 |
| E1238 | `ger.divide_the_mediterranean_spoils` | GER | 国家事件 | E1237 后 | 柏林召集马德里、罗马和安卡拉分配地中海战利品，奖赏必须足够大，也不能让盟友脱离德国轨道。 | 触发西班牙战利品谈判 |
| E1239 | `spa.claims_in_morocco_and_algeria` | SPA | 国家事件 | E1238 后 | 西班牙代表要求获得法属摩洛哥及阿尔及利亚西部部分地区，作为参战、基地和直布罗陀战役的回报。 | 谈判摩洛哥/阿尔及利亚战利品 |
| E1240 | `ita.objects_to_spanish_africa` | ITA | 国家事件 | E1239 后 | 意大利不满西班牙在北非获得过多收益，担心罗马的地中海帝国被马德里稀释。 | 意西紧张，德国仲裁 |
| E1241 | `spa.treaty_of_rabat` | SPA | 结局事件 | E1239 谈判成功 | 拉巴特条约确认西班牙获得摩洛哥全境或保护权，并取得阿尔及利亚西部一片战利品区。 | 获得摩洛哥和阿尔及利亚部分地区，殖民管理开启 |
| E1242 | `news.spanish_africa_enlarged` | NEWS | 新闻事件 | E1241 后 | 新西班牙的非洲版图扩大，马德里宣称“帝国使命”终于从内战废墟中复活。 | 新闻，西班牙威望上升 |
| E1243 | `spa.governorate_of_new_africa` | SPA | 国家事件 | E1241 后 | 新非洲总督区成立，军官、长枪党行政员、殖民公司和教会传教网络争夺新领地的第一把钥匙。 | 殖民行政、资源开发、抵抗风险 |
| E1244 | `spa.moroccan_loyalists_and_broken_promises` | SPA | 人物互动事件 | E1243 后 | 内战中为国民军作战的摩洛哥老兵询问他们被承诺的荣誉、自治和报酬，如今是否只换来更大的殖民政府。 | 摩洛哥兵员忠诚/殖民抵抗变化 |
| E1245 | `spa.algerian_borderland_problem` | SPA | 国家事件 | E1241 后且获得阿尔及利亚西部战利品 | 奥兰、特莱姆森和西部高地没有像地图上的墨线那样安静移交：卡雷罗·布兰科要求把港口、铁路和矿区交给军管，塞拉诺·苏涅尔试图用条约文本安抚柏林和罗马，法裔定居者要求保留财产与市政特权，阿尔及利亚民族主义者则把西班牙旗视作换了颜色的殖民旗。马德里必须决定这片边疆是先被开采、先被驻军压住，还是先被包装成帝国使命。 | 选择资源优先、军管优先或安抚定居者；资源收益/基础设施开发变化，驻军消耗与抵抗风险上升，意大利不满或德国仲裁压力变化 |
| E1246 | `spa.victory_dividend_reaches_madrid` | SPA | 国家事件 | E1241 后且经济复苏链完成 | 直布罗陀、德国订单和非洲资源让马德里的财政数字第一次显得像胜利者的数字。 | 经济复苏加强，德国依赖仍保留 |
| E1247 | `spa.empire_or_dependency` | SPA | 权力斗争事件 | E1246 后 | 帝国还是依赖国 | `spa_imperial_ambition` 或 `spa_german_dependency` 强化 |

## 隐藏修正事件索引

| 编号 | 事件 id | 归属 | 类型 | 触发 | 大纲 | 关键后果 |
|---|---|---|---|---|---|---|
| E0801 | `hidden.scw_prevent_cnt_if_republic_dead` | HIDDEN | 隐藏事件 | CNT 起义检查 | 如果共和国已灭亡，阻止 CNT 起义。 | 防 bug |
| E0802 | `hidden.scw_end_if_spa_dead` | HIDDEN | 隐藏事件 | 周期触发 | 如果 `SPA` 被灭，结束国民军线。 | 防卡局势 |
| E0803 | `hidden.scw_end_if_spr_dead` | HIDDEN | 隐藏事件 | 周期触发 | 如果 `SPR` 被灭，结束共和国线。 | 防卡局势 |
| E0804 | `hidden.scw_assign_spa_capital` | HIDDEN | 隐藏事件 | `SPA` 生成后 | 给 `SPA` 设置合理首都。 | 可玩性 |
| E0805 | `hidden.scw_assign_cnt_capital` | HIDDEN | 隐藏事件 | `CNT` 生成后 | 给 `CNT` 设置合理首都。 | 可玩性 |
| E0806 | `hidden.scw_ai_profile_refresh` | HIDDEN | 隐藏事件 | 新国家生成后 | 刷新 AI 行为目标。 | AI 正常作战 |
| E0807 | `hidden.scw_foreign_aid_cooldown` | HIDDEN | 隐藏事件 | 周期触发 | 管理外援冷却。 | 防刷援助 |
| E0808 | `hidden.scw_remove_foreign_volunteers` | HIDDEN | 隐藏事件 | 内战结束 | 外国志愿军撤回。 | 清理单位 |
| E0809 | `hidden.scw_restore_unified_spain_cores` | HIDDEN | 隐藏事件 | 内战结束 | 统一核心和控制权。 | 战后状态 |
| E0810 | `hidden.scw_clear_temporary_ideas` | HIDDEN | 隐藏事件 | 内战结束 | 清理临时国家精神。 | 防残留 |
| E0811 | `hidden.scw_route_nationalist_victory` | HIDDEN | 隐藏事件 | E0702 | 根据派系变量分流到佛朗哥、长枪党或架空路线。 | 设置 `spa_route_franco` / `spa_route_falange` / `spa_route_syndicalist_shadow` |
| E0812 | `hidden.scw_lock_conflicting_nationalist_routes` | HIDDEN | 隐藏事件 | 战后路线确定 | 锁定其他两条国民军路线。 | 防止路线混叠 |
| E0813 | `hidden.scw_update_syndicalist_institutionalization` | HIDDEN | 隐藏事件 | 架空路线周期 | 汇总秘书处、工团、军队、教会妥协进度。 | 触发 E1121 |
| E0814 | `hidden.scw_postwar_family_balance_tick` | HIDDEN | 隐藏事件 | 国民军战后周期 | 汇总军队、长枪党、教会、王党、技术官僚的战后权力平衡。 | 触发内阁/建国/经济危机 |
| E0815 | `hidden.scw_postwar_economic_crisis_tick` | HIDDEN | 隐藏事件 | 国民军战后周期 | 汇总粮食、外汇、配给、黑市、债务和工业恢复。 | 触发经济路线事件 |

## 国家归属索引

### SPR

- E0101 到 E0117：战前危机和共和国人物互动。
- E0201 到 E0227：共和国战争路线和战中人物互动，其中第一轮只实现最小支撑事件，完整共和国可玩路线暂缓。
- E0405 到 E0409：POUM/CNT 处理中的共和国事件，其中第一轮只作为压力来源和五月事件风险链。
- E0617：国际纵队最后一战。
- E0705 到 E0709：共和国胜利结局，长期扩展，本轮只保留清理和防卡钩子。

### SPA

- E0118 到 E0120：国民军战前阴谋人物事件。
- E0301 到 E0339：国民军战争、权力斗争和人物互动。
- E0702 到 E0703：国民军胜利通用结局。
- E0901 到 E0941：佛朗哥胜利路线。
- E1001 到 E1047：长枪党胜利路线。
- E1101 到 E1146：长枪党架空佛朗哥与制度化国家工团主义路线。
- E1203 到 E1206、E1211 到 E1225、E1230 到 E1233、E1239、E1241、E1243 到 E1247：国民军战后外交、轴心重建协定、地中海胜利、北非战利品和经济复苏事件。

### CNT

- E0401 到 E0404：CNT 社会革命，第一轮只做压力来源和背景事件。
- E0410 到 E0411：CNT 决裂和人物事件，第一轮只做五月事件风险所需最小链条。
- E0711 到 E0713：CNT 胜利结局，长期扩展。

### GER

- E0501 到 E0503：德国干涉。
- E0912：德国对直布罗陀计划施压。
- E1201：德国战后索偿。
- E1209、E1210、E1226：德国击败法国后对西班牙的大交易和拒绝后的施压。
- E1228、E1238：德国东方胜利后的地中海战略和战利品分配。

### ITA

- E0504 到 E0506：意大利干涉。
- E1202：意大利战后索偿。
- E1240：意大利反对西班牙获得过多非洲战利品。

### SOV

- E0507 到 E0509：苏联援助和政治条件，第一轮只做共和国背景和 POUM/CNT 压力所需最小链条。

### FRA

- E0512：法国边境问题，第一轮只做共和国补给背景所需最小事件。

### TUR

- E1229：苏联崩溃后土耳其加入轴心战争。

### ENG

- E0511：不干涉委员会。
- E0516：直布罗陀观察。
- E1205：战后西班牙与直布罗陀。
- E1217：西班牙开放轴心基地后的英国警报。
- E1236：英国请求“光荣和平”。

### POR

- E0513、E0515：葡萄牙通道和萨拉查支持。

### MEX

- E0510：墨西哥援助共和国，第一轮可暂缓或只做新闻/背景钩子。

### NEWS

- E0002：内战爆发。
- E0514：外国志愿军问题。
- E0601 到 E0619：战役新闻。
- E0701、E0704、E0710：结局新闻。
- E0917、E1207、E1208、E1216、E1227、E1232、E1234、E1235、E1237、E1242：国民军战后外交、轴心战争、光荣和平和西班牙非洲扩张新闻。

### HIDDEN

- E0005 到 E0006：初始化和旧内容清理。
- E0112：战前评分。
- E0220：共和国压力 tick。
- E0118 到 E0120：国民军战前阴谋隐藏事件。
- E0330：国民军压力 tick。
- E0412：CNT 起义检查。
- E0620：战役新闻路由。
- E0714：战后清理。
- E0801 到 E0815：隐藏修正、防 bug、国民军路线分流、战后权力和经济 tick。

## 国民军路线分流规则建议

- 佛朗哥胜利：`spa_franco_authority` 最高，或 `spa_hedilla_purged`，或军队忠诚高且长枪党权力低。
- 长枪党胜利：`spa_falange_power` 最高，`spa_franco_authority` 中低，且军队被党化或保守派已被清洗。
- 长枪党架空佛朗哥：`spa_franco_caudillo` 与 `spa_hedilla_compromise` 同时存在，或 `spa_falange_secretariat_control` 存在但没有公开推翻佛朗哥。
- 卡洛斯派不是本轮主线，但 `spa_carlist_anger` 应作为三条路线的危机来源。佛朗哥路线安抚，长枪党路线镇压，架空路线吸收。
- 教会不是单独胜利路线，但应强烈影响国民天主教和国家工团主义的边界。佛朗哥路线依赖教会，长枪党路线压低教会，架空路线与教会缔结制度妥协。
- 战后权力斗争不应在胜利事件后结束：佛朗哥路线是“政治家族平衡”，长枪党路线是“党压倒国家”，架空路线是“制度逐步排挤个人宫廷”。
- 建立新国家至少要有三层：最高权威来源、地方任命系统、代表/咨询机构。三条路线都必须各自回答这三件事。
- 经济线至少要有三类危机：粮食和配给、外汇和债务、工业恢复。佛朗哥路线后期可转技术官僚开放，长枪党路线偏自给动员，架空路线偏社团计划加有限开放。

## 首批实施建议

第一批只做这些，保证“战前完整 + 国民军内战/战后完整”先成立。共和国、CNT、共和国胜利、CNT 胜利和完整苏联/法国/墨西哥线只保留必要支撑与清理钩子。

- E0001 到 E0006。
- E0101、E0105、E0107、E0109、E0110、E0111、E0112、E0113、E0114、E0115、E0116、E0118、E0119。
- E0201、E0204、E0207、E0208、E0214、E0215、E0220、E0222、E0223、E0224。
- E0301、E0303、E0306、E0308、E0310、E0311、E0323、E0324、E0325、E0326、E0327、E0328、E0329、E0330、E0331、E0332、E0335、E0339。
- E0401、E0405、E0408、E0409、E0412。
- E0501、E0502、E0503、E0504、E0505、E0506、E0507、E0511、E0512、E0513、E0515、E0516。
- E0601、E0602、E0607、E0609、E0616、E0618、E0620。
- E0701、E0702、E0703、E0704、E0714。
- E0801 到 E0815。
- E0901、E0905、E0908、E0909、E0911、E0914、E0915、E0918、E0921、E0923、E0926、E0927、E0930、E0931、E0935、E0937、E0938、E0940。
- E1001、E1002、E1005、E1006、E1007、E1012、E1016、E1021、E1023、E1026、E1028、E1030、E1033、E1034、E1038、E1041、E1045、E1047。
- E1101、E1102、E1103、E1104、E1106、E1113、E1117、E1119、E1121、E1123、E1126、E1127、E1131、E1132、E1135、E1138、E1139、E1140、E1146。
- E1209、E1210、E1211、E1212、E1215、E1218、E1219、E1220、E1221、E1222、E1223、E1224、E1225。
- E1227、E1228、E1229、E1230、E1231、E1232、E1233、E1234、E1235、E1236、E1237、E1238、E1239、E1241、E1242、E1243、E1244、E1245、E1246、E1247。

首批目标事件数：约 120 到 160 个。国民军路线会先有完整分流、具名人物、人物互动、政府班底、权力斗争、建国骨架、经济危机、轴心援助复苏链和轴心胜利战利品链；共和国和 CNT 只做到能支撑战前、国民军叙事、新闻、AI 和清理。
