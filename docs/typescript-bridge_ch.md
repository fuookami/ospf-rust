# TypeScript 桥接

[English](typescript-bridge.md) | 简体中文

## 职责

Rust 是计算、建模、模型 codec 和求解执行的唯一实现。独立的 `ospf-typescript` 仓库提供 npm 主包 `ospf-wasm`；SDK 不复制 Rust 算法。当前为 **0.1 初始实现，不是整库导出完成**。

| 组件 | 职责 |
| --- | --- |
| `ospf-rust-bridge` | 协议主版本 1、Rust 生成的 TS DTO、既有线性/二次及 canonical CP codec、MetaModel 编译、指纹和精确 CP 报告复验 |
| `ospf-wasm` | wasm-bindgen 对象绑定，不启用 native solver 或 Tokio |
| `ospf-rust-native` | HTTP/N-API 共用的有界、单任务、后台原生执行器 |
| `ospf-rust-host` | loopback HTTP、Bearer token、显式 CORS、任务轮询/取消/结果/释放 |
| `ospf-rust-node` | napi-rs 原生绑定及自动生成的底层声明 |

## 能力矩阵

| 模块 | 当前 TS 能力 | 执行路径 | 尚未开放 |
| --- | --- | --- | --- |
| base | 错误映射、共享协议类型 | TS SDK | Rust 专用容器、迭代器、函数 trait |
| multiarray | f64 动态行优先数组、TypedArray 批量复制、线性读取、形状副本、二维矩阵乘、一维点积、二维转置、方阵迹（复用原 einsum）、全局/单轴/多轴求和与前缀和（复用原 FastSum，轴被消去）、一维外积、任意轴缩并（复用原通用 einsum 执行器，与矩阵乘/点积对照验证）、形状重排（沿用原截断/补零语义）| WASM（浏览器/Worker/Node）| view、block、广义 einsum 表达式、DataFrame |
| math | Decimal/Rational 精确文本构造、加减乘、取负、精确比较与零判定；Rational 另有精确除法（零除数拒绝）| WASM（浏览器/Worker/Node）| Decimal 除法（一般不精确，未开放）、其余代数、几何、符号函数与 parser |
| quantities | 运行时 f64、m/km/cm/dm/mm、m³/L、m/mm 与 in/ft/yd、其平方单位（含 dm^2）、s/min/h/kg/g/t/lb、量纲校验、转换、相加、同量纲相减、无量纲标量缩放、量纲乘除（原单位代数合并，如 `L^2`、`L·T^-1`）、同量纲比较（跨量纲拒绝）、量纲符号读取、生成契约的物理量 DTO | WASM（浏览器/Worker/Node）| 其他单位（体积已开放 m³ 与 L）、编译期物理量；合并后的单位可能不在固定词表内，`symbol()` 此时为空串 |
| core | 连续/二元/有限精度整数变量、Linear/Quadratic 与关系快照、真实 Token/LinearExpressionSymbol/QuadraticExpressionSymbol、线性/二次目标约束、MetaModel、线性/二次模型交换 | WASM 建模；原生端共享 codec | 其他符号特化与 checkpoint；非零目标常数与 codec 无法保留的元数据明确拒绝 |
| core CP | 精确 i64 域/表达式、当前 core AST 全部约束族工厂、固定/变量时长与正负 presence 区间、独立 canonical snapshot、赋值验证、assumptions 快照派生 | WASM（浏览器/Worker/Node），复用 core AST 与 codec | 其他 core 操作、持久会话/原生 assumptions 参数/conflict shrinking；工厂覆盖不代表原生降维全部支持 |
| core analysis | CP/线性候选活动性、精确整数证据、默认完整 MIP 降维预检与原始报告文本、候选漏斗（`candidateFunnel`，按优先级排序并分组；无对偶证据时只出现 Tier C/Unclassified，绝不伪造分层）| WASM，无需求解器 | 后端专属 CP support report、冲突提取、再求解及其他分析流程；敏感性已开放（`linearSensitivity`，需可行 incumbent，SDK 侧 `LocalConstraintSensitivityReport` reader 由原 core `validate` 复核）；IIS 已开放：原生端 `linearIis` 入口（仅 Gurobi 后端）＋ SDK `LinearIisReport` 只读 reader（WASM 内校验自洽性，不计算 IIS） |
| solver | Gurobi/SCIP 的原生 LP/MIP/QP/MIQP/QCP、CP 严格有限降维、节点/解上限、精确 hint、整模型 conflict seed、线程数／随机种子／相对 MIP 间隙调优（仅直接后端入口；非零间隙可能以 `GapLimit` 终止且无最优性证明）、能力、进度、取消、精确 CP 报告；`summarizeReport` 暴露 `hasOptimalityProof`／`relativeGap`／`absoluteGap`／`nodes`／`solveTimeMs` 供调用方自行判断结论强度 | Node N-API / HTTP；无 WASM 求解器 | 其余求解器参数、CP 持久会话/assumption conflict shrinking；SCIP 二次入口需 scip-quadratic feature。LP 解池已开放：原生端 `linearSolutionPool` 入口（SCIP/Gurobi 原生实现）＋ SDK `LinearSolutionPoolReport` 只读 reader（不认证最优性） |
| framework | 原生 Serial/ParallelCombinatorialLinearSolver、类型化后端顺序/终态策略及 First/Best 模式、完整 attempt 轨迹及最终选择报告、固定连续追索 Benders、同源投影列生成及无损子报告 | HTTP / N-API 执行真实 Rust 编排；WASM 读取聚合报告 | 多后端扫描已开放（`linearSweep`，复用 framework `ExperimentRunner`，结果按 case ID 稳定排序）；通用 pipeline 注册、其他列生成策略、其余 Benders 与领域策略注册；认证定价对偶已开放（`CertifiedPricingDuals.verify`，由原 `VerifiedPricingDuals` 复核最优 LP 证书） |
| BPP1D | 默认规则的物理量问题/指纹、严格 CSV 双表导入（闭合表头、空库存即无限、产物与 JSON 构造等价）、有界候选生成、精确组合用量校验、物理展开/坐标复验/物料/KPI、候选 MILP 及有限池列激活、独立无损报告、原域渲染适配器（复核放置后按展示单位输出，非精度/最优性证据）| WASM 纯计算（含 CSV 解析）；HTTP / N-API 执行 Rust 领域编排，含 `tailWindow` 尾箱优化（只重排末 N 箱，改动排样即收回全局最优性主张）| 离散装载、任意领域扩展策略 |
| CSP1D | 物理量 DTO/无损 codec、Roll/Sheet、Simple/N-Same、方案/需求/库存/设备复验、整数及完整流程报告读取 | WASM 计算/报告；HTTP/N-API 显式原生 MILP/列生成，真实 SCIP/Gurobi 10 验收 | 动态长度及扩展规则；旧 Rust 默认仍是启发式。产品可声明长度与单位重量，重量口径需求按原领域几何换算。配规（Costar）已可声明并无损编解码、可在显式方案中引用，但原 `CostarFiller` 在领域内无调用方，默认生成器不产出配规切片 |
| geometry2d | 点/向量、矩形/圆、朝向、实际/包围面积、包围盒、板材包含、刀缝与批量重叠对；精确圆盘相交（圆心距 vs 半径之和，相切不算，非圆返回 null）、圆心读取、一维容纳数量 `countFitting`（沿用原内核：非正单元或容器更小时返回 0）| WASM，调用原共享 Meter/f64 内核 | 其余量值工具（`overlapsAny`／`widthDelta` 与既有批量重叠、边界相重复相通，不重复导出）、高精度特化；圆×矩形仍按包围盒判定 |
| BPP2D | 带单位源目录、无损 codec、完整原生报告与整数产出、有界压缩装箱／余料／物料／渲染 | WASM 源/报告/装箱；HTTP/N-API 显式原生列生成，真实 SCIP/Gurobi 10 验收 | 非默认质量／承重／规则；尾板优化（舱位二维离散装载的矩形互斥边已在共享 BPP 行开放）|
| CSP2D | 生成源／报告契约、无损 codec、`CuttingContext`、`Csp2dReport` 及独立整数产出；`Csp2dColumnGeneration` 原生列生成 | WASM 源／报告校验；HTTP/N-API 显式原生执行，真实 SCIP/Gurobi 10 验收 | 其他领域规则；部分旧 shelf 断头台树仍不满足严格物理门禁 |
| BPP | `DiscreteLoadingContext` 稀疏分配／容量／占位／冲突／布局注册、可选舱位一维区间或二维矩形自动补出同空间严格重叠的互斥边（二维走原 `Bpp2dDiscreteLoading` 的精确矩形判交；贴边与跨空间不算冲突；两者互斥）、可选货物／占位／布局成本、有界连续附加重量、完整向量／方案及报告复验、普通及连续资源 Benders 原生 application、完整源 MILP 收尾；分区列池／认证 LP 定价／原子加列／资源补全基础对象、原生根分区 application、无时钟完整重放及显式预留完整源收尾 | WASM 建模／生命周期／复验；HTTP/N-API 普通线性与 compiled-linear 子模型及原 application 执行真实 SCIP/Gurobi | 任意业务扩展、branch-and-price；不含三维几何 |
| BPP3D | 带单位源（默认长方体与轴感知圆柱，圆柱可选离散半径候选、可选包属性子集、可选目标策略 none／带权最小化半径平方）／无损指纹、初始层及完整源层分配的严格物理校验（圆柱按真实圆形足迹）；源绑定列池（含从源重建的连续半径组件）、单阶段 codec 及有界版本化单轮执行报告 codec（另透出原流程 info 里的连续半径条目） | WASM 源、多箱多层方案及完整执行报告复验；native 正式分派与 HTTP/N-API 执行，SDK `ExecutionReport`／`Bpp3dOneRoundExecution`；真实 SCIP/Gurobi 10 覆盖分组单箱及跨箱复用；连续半径组件（原型＋业务权重函数）经真 SCIP 在 final 模型注册真实半径／PWL 变量 | 连续半径的业务权重函数以外的目标策略（当前仅默认 tie-breaker）、包属性的其余业务规则（仅开放 CSV 路径实际物化的三项）、分组模型扩展、CSV 内嵌 trace；CSV 一轮入口已按声明数量传递需求，全流程入口对分组需求显式拒绝 |
| Gantt/Network/Gantt-Network | Gantt 活动调度已全链路开放；Network 已接入**多商品最小费用流**（`networkFlow`，原 `network-scheduling` 的 flow 领域），广义流也已接入（`networkGeneralizedFlow`，产率与单位转换真的生效）；网络建设也已接入（`networkDesign`，共享建设选择 + 名义流，建设费与运行费分开交付）；网络结构也已接入（`networkStructure`，根树 + 设施归属，不注册商品流）；运输分配也已接入（`networkAllocation`，共享配额 + 枢纽建设，运输量按请求-服务对交付）；韧性规划也已接入（`networkResilience`，显式故障场景 + 连通要求，场景结果逐个交付、不越出显式场景）；VRP 也已接入（`vrp`，带时间窗车辆路径的分支定价；路线按停靠点顺序交付，预算只能收紧算法）。至此 `network-scheduling` 全部子域接通；二次组合编排也已接入（`quadraticOrchestration`，多后端串行/并行 + 选结果策略） | WASM `ActivityContext`（源校验／候选展开／方案复验／报告 reader）＋ SDK `ActivityContext`／`GanttActivitySolver`；native 分派 `gantt-activity` 经 HTTP／N-API 执行原生单 MILP，报告由 bridge 编码并独立复验 incumbent | 物料流／基线／执行事实／附加业务行与任意策略；Network 迁移未完成 |

Gantt 活动调度的桥接源协议（`ospf_rust_bridge::GanttActivityProblem`）只接受可序列化数据：RFC 3339 区间与开始域、固定 `QuantityUnit` 费用／容量词汇、活动与模式、分阶段资源占用、FS/SS/FF/SF 端点依赖（含最小／最大间隔与存在性语义），以及资源日历（不可用区间与时变容量）。`encode_gantt_activity_problem` 复验后返回源绑定指纹，`decode_gantt_activity_problem` 强制 canonical 文本与指纹一致，`prepare_gantt_activity_source` 用原领域 `ActivityPreparation` 展开完整有限候选并返回计数摘要。上限为 512 活动、每活动 16 模式、每模式 4096 开始（总计 20000）、256 资源、32 分阶段占用、每资源 256 日历区间、4096 依赖、366 天区间与 8 MiB 文本。基线、执行事实、附加业务行、物料流与任意费用策略仍未开放；求解已由 native `solver: "gantt-activity"`（`modelKind: ganttActivity` + 独占 `ganttActivityOptions`）接通，报告由 bridge 编码并经原域复验；该源本身不是求解报告，不声称可行或最优。

`replay_gantt_activity_plan` 接受显式方案（任务、模式、RFC 3339 开始），要求每个选择都是已声明的完整候选、同一任务与模式不得重复，然后调用原领域 `ActivityPreparation::validate` 与 `schedule` 复验容量、日历、依赖、必做缺项与库存轨迹，返回按候选稳定顺序排列的绝对时间分配、未选择的可取消任务与原域费用；费用单位取自源声明，资源占用单位取自对应源资源声明。违反任一原域规则即拒绝，不产生任何最优性声明。

原生执行入口 `solver: "gantt-activity"` 要求 `modelKind: ganttActivity` 与独占的 `ganttActivityOptions { backend }`，复用原 `ActivitySchedulingApplicationService` 单 MILP 流程并共享时间预算、取消与进度；报告 `GanttActivityExecutionReport` 保留原问题状态、终止原因、派生目标与 `solverModelOnly`，且只有通过 `ActivityPreparation::validate` 独立复验的 incumbent 才携带分配，否则只保留失败原因（不伪造方案）。WASM 侧 `GanttActivityContext` 提供源校验、`sourceSummaryJson` 候选展开、`replayJson` 方案复验与 `readExecutionReport` 报告复验；SDK `ospf-wasm/gantt` 提供对应类与 `GanttActivitySolver`（能力门禁 `ganttActivity`，`modelKind: ganttActivity`，独占 `ganttActivityOptions`）。

BPP2D 原领域 context 已有 RMP／归一化短缺 Phase I／整数主问题的事务性共享 MetaModel 注册，支持硬需求上下界、受检数值转换及原快照结果复验。`Bpp2dNativeBackend` 与 CSP1D 共用 framework `LinearReportExecutor`，校验实际模型／报告身份、完整 primal／整数性／目标及 LP 证书，共享总预算、原始进度和独立子取消。`solve_bpp2d_column_generation` 串接 Phase I 恢复、认证后的普通整板面积定价及同后端 final MILP；观察到的排布通过源目录／旋转／刀缝校验，保留完整池、实际子模型／报告及池前缀映射，检查迭代／池／报告／总时间／取消预算。原生成器仍是启发式，`candidate_complete` 始终为 false，不把受限最优／不可行提升为源问题全局结论。默认规则循环暂不支持非零质量／承重或附加规则定价修正。旧闭包执行器保持兼容，不被标记为真实报告路径。详见[原生列生成边界](../ospf-rust-framework-bpp2d/README_ch.md#报告驱动的原生列生成)。

`encode_bpp2d_problem`／`decode_bpp2d_problem` 提供 schema 1.0 canonical 源封装与领域分离指纹；完整执行 codec 将全部物理列及每个实际子模型／报告绑定到源问题。验证器重建原 context，严格比较数学／类型／边界／目标／执行元数据，不替换实际报告身份；复验 LP／不可行证明、Phase I→RMP→定价→final 生命周期及新增列实算检验数。生产量仅从真实 final 整数 incumbent 派生，checked u128 汇总转十进制文本，件数不经 JS number；圆仍按外接方形进行几何判定，产出面积使用原形状实际面积。数学一致性不是来源认证。

SDK `ospf-wasm/bpp2d` 开放 `PackingContext`、`Bpp2dReport` 和独立可释放的 `ProductionResult`、`PackingResult`，浏览器／Worker／Node 各 realm 显式初始化。输入件数使用 0..2^53-1 的 bigint，生成件数另限 4096；源最多 64 物品、16 板材、128 需求、256 初始列，完整池最多 512 列、总计 4096 放置，并保守限制板材／单件包围面积比不超过 4096。文本限 8 MiB，归一为米／平方米；初始重复位置保留，生成物理重复拒绝。原子报告 u64 元数据保留文本。`OSPF_BPP2D_FIXTURE_DIR` 可提供 ignored 的真实 SCIP/Gurobi 证据给 SDK／Worker／隔离包验收，不是发布依赖。HTTP／N-API 共用原生执行器；装箱／渲染委托原 Rust 纯计算，非默认领域规则仍未接通；SDK reader 本身不执行生成器或求解器。

SDK `ospf-wasm/bpp3d` 开放长方体与轴感知圆柱的 `PackingContext`：Rust 生成 DTO，数量由 bigint 经规范十进制文本传递，源文本具有领域分离指纹，并物化为原 CSV application 输入。每个初始层调用原严格物理回放和几何守卫校验完整单件坐标、允许朝向、重叠、边界和单层重量；源校验本身不声称满足完整需求或认证来源。`packLayers` 可按显式源层／箱分配复验多箱、多层、叠加深度、总载重和精确分组需求，按源箱及源层顺序返回米制绝对坐标；同一层可复用于不同箱，同箱重复分配会被拒绝。该方案不是求解器报告，不证明最优或认证来源；最多 4096 个分配和 4096 个展开后的物品，方案文本限 8 MiB。对象需显式释放，返回 DTO 为独立副本。最多 64 物品／64 个独立候选箱／256 层、每层 256 放置／总计 4096 放置，正需求总量不超过 100000，文本限 8 MiB。重复箱型可代表不同候选箱，但归一化尺寸、容量和主箱标记必须一致。只接受六种原朝向的长方体与轴感知圆柱（对齐轴 + 半径，轴向长度取自对应尺寸，圆形足迹由原几何守卫校验），未知规则、回调及不带实际坐标的汇总明确拒绝。长度归一为米；质量与载重先检查质量量纲并归一为千克数值，原 BPP3D 领域却将其放在 `Meter` 参数化字段中，因此内部类型并不提供编译期质量单位保证。完整单轮执行报告 codec、native／HTTP／N-API 执行及 WASM／SDK 报告 reader 已接入（详见本节后段）；离散／连续半径候选与扩展规则仍未接入；生成器链已可按显式类型配置（见本节后段）。

Rust bridge 已提供源绑定单轮列池及单个 RMP／final 阶段报告 codec。列池保留生成块网格与放置 trace，整数以十进制文本表示；阶段 reader 重建默认分组需求模型，比较实际子模型的完整数学内容，复验原报告并从真实 incumbent 提取 final 分配。它不认证生成器或求解器来源，分配只有严格物理回放通过后才是装箱方案。上限为 512 列、16384 个 trace 条目和每份文本 8 MiB；完整单轮执行报告生命周期（三阶段顺序、前缀／共享池一致性、最优 LP 对偶证书、严格物理回放与失败证据）及 WASM／HTTP／N-API／SDK 报告入口已开放。

`encode_bpp3d_execution_report`／`decode_bpp3d_execution_report` 在此基础上补全有界、版本化的单轮执行封装，保留实际初始 RMP、刷新 RMP、final 子模型与原报告以及两份列池。解码会按源绑定列池重建每个子模型，要求初始 RMP 只在迭代 0 精确使用源初始前缀、刷新 RMP 处于迭代 1、final 列池与刷新池逐字相同，用精确对偶证书复验每一次最优 LP 声明，要求无报告的阶段只能是失败执行的最后阶段，最后按记录的展开预算重跑严格分组物理回放后才交出源绑定装箱。缺 final incumbent、final 认证不可行、analysis 失败或展开预算不足都保留真实证据与原失败文本：摘要中的 `completed`／`failure` 表达结局，`candidateComplete` 恒为 false，因为预注册启发式生成器从不重放。整份报告限 8 MiB、三个阶段、展开 100000 个放置。native 引擎以共享总预算、进度、取消和报告上限把该流程分派给 HTTP 与 N-API；WASM reader 与 SDK `ExecutionReport` 只复验不重解。

执行的生成链是显式的：`bpp3dOptions.generators` 接受 1..=7 个不重复的已注册策略（`block`、`blLocal`、`blGlobal`、`circlePacking`、`pattern`、`pile`、`historical`），缺省为去除历史层重放的原标准链——历史层生成器若重放源初始层身份会产生不带物理 trace 的重复列，严格 final 必须拒绝（`FinalMilp: selected assignment has no physical trace; coverage fallback forbidden`）。不加载任意回调，任何链下 `candidateComplete` 恒为 false。

CSP2D 的 `ProduceContext` 共用 RMP／归一化短缺 Phase I／整数主问题的真实 MetaModel 注册；
`Csp2dNativeBackend` 复用 framework `LinearReportExecutor` 执行，真实 SCIP／Gurobi 10 验证原报告、LP 证书、
件数／面积独立需求口径、绑定库存对偶、硬需求上界、附加规则／常数及不可行结果。只有证书验证通过的最优 LP
提供价格，非最优 incumbent 不冒充定价证书。旧 service／启发式回退不变。每次图案使用消耗一个物料批次，
卷材段亦如此；消耗／余料沿用原 region 面积，可能是部分板材，不虚构整板面积。
`solve_csp2d_column_generation` 已串接 Phase I 恢复、认证 LP 定价及 final 整数执行，显式调用原生成器；观察列
先校验源物料／工艺／产品尺寸／旋转和物理几何，定价保留件数／面积、需求上界及库存贡献。恢复依据实际原单位短缺，
不把归一化目标很小当作零短缺。全部子求解共用预算，保留完整池、实际子模型／报告和定价轨迹；
`candidate_complete` 始终为 false，不把受限最优／不可行升级为源问题全局结论。真实 SCIP／Gurobi 10 验收原 shelf、
恢复、多物料库存定价、硬上界、卷材加权目标及显式正刀缝有效断头台列。求解仍**仅原生端**，通过共享 `NativeEngine`
的 HTTP／N-API 任务及 SDK `Csp2dColumnGeneration` 开放。请求必须同时指定 `solver: "csp2d"`、`modelKind: "csp2d"`
和 `csp2dOptions`，拒绝混用其他策略。SDK 检查领域／算法及所选 SCIP/Gurobi 的 LP/MIP 能力，不隐式回退 HTTP 或 WASM。
目录一致性不是外部来源认证；部分旧 shelf 断头台树缺少匹配成品叶子，
严格门禁会拒绝而非静默修复。重量、设备、最小余料阈值、副产物规则明确拒绝，附加 pipeline 需匹配定价修正。
详见[原生列生成边界](../ospf-rust-framework-csp2d/README_ch.md#报告驱动的原生列生成)。

共享 bridge 已生成 CSP2D 源 DTO，`encode_csp2d_problem`／`decode_csp2d_problem` 提供 schema 1.0 canonical
原文及领域分离 SHA-256 指纹。SDK `ospf-wasm/csp2d` 开放 `CuttingContext` 构造、`fromArtifact`、`toArtifact`、
`toJson`、`validateColumns` 与独立释放；浏览器／Worker／Node 使用相同 WASM 原领域校验，不启动生成器、求解器或时钟。
此校验不证明任意用量满足需求，更不证明全局最优或求解器来源。件数使用 bigint，范围 0..2^53-1，超范围拒绝不舍入；
库存必须为正，每物料的正生成提示合计最多 4096。源限 64 产品／16 物料／128 需求／256 初始列，校验池限 512 列及
4096 放置，源／列文本限 8 MiB；源封装外层允许 16 MiB + 1 KiB，以容纳 JSON 再转义，内部源仍限 8 MiB。
切树采用平坦后序，末节点为根，子索引必须不同且指向先前节点，禁止共享和游离节点；
每树限 4096 节点、深度 128，整池限 65536 节点。长度／面积归一为米／平方米，保持原 f64 严格物理判定。

`encode_csp2d_execution_report`／`decode_csp2d_execution_report` 已将完整物理列池及每个实际子模型／原报告绑定到源。
解码器重建 `ProduceContext` 并严格比较数学、变量类型、边界、目标与执行元数据，不替换实际子身份或改写原报告。
复验 LP／不可行证书，重放 Phase I→RMP→定价→final，按源需求上下界及库存行重算已接纳列检验数；恢复检查原单位
短缺，不把很小的归一化目标当作零。这是数学一致性，不认证求解器来源，也不独立穷举证明全局最优。

SDK `CuttingContext.readExecutionReport` 与独立 `Csp2dReport.fromJson` 在浏览器／Worker／Node 使用同一 WASM reader，
保留完整原文、实际子报告、全池、定价批次与 Rust 派生摘要。`production()` 仅从复验过的 final 整数 incumbent 返回
独立可释放的 `ProductionResult`；无整数解时不虚构产出。件数／批次通过 checked u128 汇总再转文本／bigint，并精确
重查需求上下界及库存；面积保持有限 f64。消耗沿用 region，净余料含刀缝带，卷段高总量不引入共享卷长规则。
`candidateComplete` 始终 false，受限池不可行不等于源不可行，晚到取消不改写已完成子报告。

报告文本限 8 MiB。配置允许 0..64 定价批次、每批 1..512 观察候选、1..512 列及 1..1024 报告，检验数容差 [0, 0.5)，
默认依次为 8／128／512／196 和 1e-6；`finalIntegerEnabled` 默认 true，目前仅预注册原 shelf，旧断头台树失败不修复或
替换。`OSPF_CSP2D_FIXTURE_DIR` 为 SDK／Worker／隔离包验收提供 ignored 的真实 SCIP/Gurobi 证据，不是发布依赖。
真实 SCIP/Gurobi 共享执行器、认证的 in-process HTTP 路由及隔离 N-API 包验收覆盖求解、进度、预算、禁用 final、
晚到取消与释放。独立浏览器 HTTP 验收须显式配置 `OSPF_E2E_URL` 和 `OSPF_TOKEN`，无宿主时记为跳过，不视为通过。
其他领域规则仍未完成。

不存在的能力不导出成功占位函数。后续领域绑定继续复用 context/aggregation/pipeline，完整多轮求解在原生侧执行；TS 回调不能自动成为跨进程 Rust trait。

## 共享 BPP 稀疏装载建模

`ospf-wasm/bpp` 提供原 `DiscreteLoadingContext` 的默认规则门面：独立货物、显式稀疏合法分配、真实件数／重量容量、
整体占位、非传递冲突边和显式布局组。原 context 负责 MetaModel 注册及方案复验，不在 TS 拼接算法或领域约束。
固定货物在禁用舱位仍可合法保留；固定占位和允许的空预留不被误当作装载货物。此入口没有坐标或三维碰撞判断。

源协议独立 schema 1.0、canonical 文本及领域分离 SHA-256。源摘要通过新增的注册前 `with_namespace` 进入实际模型
名称，因此数学相同但货物身份不同的源也不能互换报告。默认 Rust context 仍使用原隔离命名空间；新 API 不改变默认
行为。桥接线性 codec 保留 `[0,0]`／`[1,1]` 固定二元边界，不放宽分数、无界或反向二元区间。

`maxCount` 使用完整正 u64 文本／bigint，可超过 2^53；原容量限制以实际稀疏邻接数收紧模型系数，不把大整数转成
浮点 big M。重量显式使用 kg／g，委托 quantities 转换并拒绝非有限值、负值及换算溢出／下溢。源限 512 件货物、
128 舱位、4096 分配、2048 冲突、64 布局组、合计 512 布局和 4096 布局舱位成员，文本及执行模型各限 8 MiB。
反向冲突、重复分配／成员及无效引用明确拒绝，不静默去重。构造冲突索引前检查端点目录，防止非法外部顶点放大计算。

`toLinearTriadModel()` 在省略成本时导出原注册的**零目标可行性 MILP**，可通过既有 `SCIPSolver`／`GurobiSolver` 和显式 HTTP 或
Node 客户端求解。该直接模型入口不是分区／Benders 或最大装载优化器，可选货物不保证被选中。`solutionValues` 返回独立
Float64Array；`verifySolution`／`extractSolution` 同时复验完整模型和领域。`readReport`／`DiscreteLoadingReport.fromJson`
检查实际导出模型映射／指纹、完整 incumbent、整数性、目标和原领域约束，保留原报告文本与 u64 元数据。只有实际
incumbent 产生独立 `DiscreteLoadingResult`；取消／超时／不可行无解时不虚构零方案。在认证原生提交边界，预取消报告
缺少的模型身份按既有 BPP1D 规则补齐，已有身份、终态或证明不覆盖。数学一致性不等于外部 solver 来源认证。

真实 SCIP/Gurobi 验收包括固定货物／禁用舱位、无合法分配不可行、重量不足、显式布局、空预留、取消及单位换算；
in-process HTTP 与隔离 N-API 分开验证。`OSPF_BPP_LOADING_FIXTURE_DIR` 仅指向 ignored 证据，供浏览器／Worker／
SDK reader 验收，不成为跨仓源码或正式发布依赖。分区生命周期／资源补全及原生根分区 application 另见专节；
任意业务扩展、branch-and-price 与任意 trait 回调仍待接入（BPP3D 原生执行已接通，见 `Bpp3dOneRoundExecution`），不能把任意 TS 回调自动变成原 Rust 扩展。

### 原共享 BPP 普通 Application

`DiscreteLoadingContext(problem, objective?)` 可接受生成的 `LoadingObjective`：`itemCosts`、`positionCosts` 和
`layoutCosts: [{ groupId, layoutId, cost }]`，均为有限最小化成本，负值表示奖励。占位成本包含空预留；未知身份及
重复布局项拒绝。原事务性 `register_with` 注册成本，源摘要包含成本；省略成本保持旧零目标源编码。

`DiscreteLoadingApplicationService(client, { backend })` 通过 `solver: bpp-loading`、`modelKind: bppLoading`
及独占 `bppLoadingOptions` 调用原普通 MILP Rust service，客户端同时检查 `bppLoading`／`bppLoadingMilp` 和
所选后端 LP／MIP 能力。HTTP／N-API 共用工厂／求解总预算、进度、合作取消与释放，默认 60 秒，不隐式回退求解器或传输；
有界 codec 工作不属于可抢占硬截止。

`toApplicationModel()` 保留实际 snapshot 编译，不冒充现有 mechanism 导出模型。原默认约束使用显式稳定行 ID，
相同源命名空间的独立注册可重建相同 application 指纹，默认 context 隔离不变。报告不改写身份或行映射。
`DiscreteLoadingApplicationReport.fromJson`／`readApplicationReport` 复验完整模型身份、incumbent、最小化目标和
原领域提取；`solverReportJson` 保留精确元数据。工厂前取消／时限保留无证明、未绑定模型的原 core 报告和
`registrationPresent: false`，不伪造方案。branch-and-price 及任意领域扩展仍未绑定。

### 有界连续附加重量

`DiscreteLoadingContext(problem, objective?, resources?)` 的第三参数为 Rust 生成的 `LoadingResource[]`，
提供时限 1..128 项，每舱位至多一个且身份唯一。字段为 `id`、`positionId`、带 kg/g 的
`minimumWeight`／`maximumWeight`、非负无量纲 `minimumPerLoadedWeight` 以及有限 `costPerWeight`。
成本按 `problem.weightUnit` 的每单位计价，kg/g 源改变时不自动换算成本。负成本允许，但变量始终有界。
连续变量及最小装载比例在 Rust registration adapter 注册，原 `link_additional_weight` 负责真实容量与整体占位；
附加行使用稳定 ID，独立 snapshot 注册仍可重建。源摘要包含资源定义，数学相同但身份不同也不能移植报告。

有资源时，`solutionValues(plan, weights)`／`verifySolution(plan, weights)` 必须显式提供
`ReadonlyMap<string, PhysicalQuantity>` 的完整资源重量，缺失、未知资源、错误单位、比例／边界／容量／占位违规都拒绝，
不猜测零值或最优资源。`extractSolution` 与两个报告 reader 同样复验完整连续向量。
`result.resources()` 单独返回资源 `weights`、`positionTotalWeights` 与 `totalWeight`；旧
`loadedWeight`／`positionWeights` 始终仅计货物。所有重量归一为模型单位，返回独立 DTO 副本。
省略资源时旧源及结果编码保持不变。资源建模本身不运行分解；共享 BPP Benders 见下节。

### 原共享 BPP 连续资源 Benders

`DiscreteLoadingBendersApplicationService(client, { backend, recourseResources, iterationLimit?,
recoverMissingCertificates?, fallbackToMilp? })` 显式调用原 `DiscreteLoadingBendersApplicationService`。
`recourseResources` 必须为 1..128 个唯一已声明资源身份，映射为本次注册的真实连续变量，非零目标资源也可进入追索；
不接受任意 token、JS callback 或空追索伪门面。迭代限 1..10000，默认 128；默认总预算 60 秒及容差 1e-7。
原证书恢复默认开启，已有无效证据不替换；只有显式 `fallbackToMilp: true` 允许非取消可恢复失败后的全新等价工厂 MILP。
HTTP／N-API 共用 `bpp-loading`／`bppLoading` 任务及嵌套 `bppLoadingOptions.benders`，SDK 要求
`bppLoadingBenders` 和所选后端 LP／MIP 能力，不回退旧普通入口或 HTTP。普通 facade 仍只接受 `{ backend }`。

原服务复用完整注册、固定连续追索、证书／割验证、总预算、进度和合作取消。初始构造时间计入预算；
分解编码持有初始 `LinearModelSnapshot`，不能拿全新回退会话 token 解释旧分解值。实际 fallback 原报告单独保存，
更好分解方案可被保留；取消／时限／收敛不启动回退。真实 SCIP／Gurobi 验收包含最优性割、可行性割、
负追索成本、g 单位、迭代限制、取消和全新工厂回退，不以编译通过代替求解通过。

`DiscreteLoadingBendersReport.fromJson`／`context.readBendersReport` 验证源、配置、实际完整模型、
资源方案、分解 incumbent／bound 一致性、原子报告 schema、回退选择及证明边界；读取**不独立重放全部切割、认证 bound
或外部 solver 来源**。原分解合并源报告即使 converged 也保持无证明的 `FEASIBLE`，无 incumbent 为 `UNKNOWN`；
不可行 master 不升级为完整源不可行证明。只有实际完整源 MILP 回退报告可保留自身证明。
`decompositionSummary()`、`decompositionReportJson()`、`fallbackReportJson()`、`decompositionError()` 和
`fallbackUsed()` 保留不同证据，不把子报告重标记。外层限 64 MiB + 4 KiB，application 沿用 16 MiB + 1 KiB、
固定追索及原子 fallback 文本各限 8 MiB，精确 u64 子元数据保持嵌套文本。结果副本独立释放。
branch-and-price、任意扩展策略及三维几何仍待绑定；分区生命周期／资源补全基础对象另已开放。

### 原生 CP 执行

SCIP/Gurobi 仅在对应 native feature 编译后声明 `cpExactLowering`。SDK `solveConstraintProgramming(snapshot.toArtifact(), options)` 先检查能力，再显式提交 `modelKind: 'cp'`；旧宿主不声明能力时在提交前拒绝，不回退为线性。HTTP/N-API 复用现有有界执行器、进度、取消句柄、任务存储和原始报告文本。浏览器不执行 native solver。`timeLimitMs` 限制后端求解，不是覆盖 codec/降维全过程的可中断截止时间。

SCIP 复用 `ScipConstraintProgrammingSolver`，Gurobi 复用 `MipBackedConstraintProgrammingSolver<GurobiSolver>`；均使用 core 默认严格有限降维预算，遇到未支持结构、预算超限或不能无损转入 f64 MIP 的模型明确拒绝，不舍入/丢约束。完整 i64 快照支持不等于所有 i64 模型均可求解。数值/预算错误保留 core 的 `NUMERICAL` 分类，未支持结构保留 `UNSUPPORTED`。WASM 已支持 assumptions 快照派生后按普通 CP 任务提交；持久会话、原生 assumptions 参数、assumption conflict shrinking 和自定义降维预算尚未开放。

非零 CP 目标常数在实际降维中通过固定为 1 的辅助列进入后端，使目标、界、gap 和进度包含常数；源报告不暴露该列。所有 CP 目标（包括零常数）的常数绝对值与各系数／变量域贡献绝对值上界之和不得超过 `2^53`；这一保守门禁可能拒绝相互抵消的表达式。既有 Rust 普通降维接口仍单独返回常数。新宿主声明 `cpObjectiveConstant`；缺少该能力时，SDK 使用已初始化的 Rust WASM codec 拒绝非零常数，并在提交零常数快照前执行当前默认预算下的完整 MIP 预检。此同步预检也可能拒绝其他未支持或超预算模型，且不可中断。旧 SDK 不执行此门禁。

新的原生运行时为已编译的 SCIP 和 Gurobi 声明 `intermediateProgress`。直接提交的求解任务可以发布百分比不确定的中间快照，默认按 100 毫秒节流；可用时包含当前解目标、界和 gap，最终报告仍为权威结果。未声明该能力的旧宿主不保证中间快照。轮询只能观察最新快照，因此可能错过短任务和部分迭代。

`solveConstraintProgramming` 的参数类型为 `ConstraintProgrammingSolveOptions`，复用 Rust 生成的 `ConstraintProgrammingOptions` 契约；nodeLimit/solutionLimit 接受 u32，零保留 core 立即返回 NODE_LIMIT/SOLUTION_LIMIT 的语义。`solutionHint` 为稳定 ID 到 bigint 的 ReadonlyMap，编码为 `IntegerSolutionHint` 十进制文本项，不经 JSON number；Rust 限制最多 100000 项，拒绝重复 ID、非 canonical i64 文本，core 校验 ID、域及完整 hint 的源可行性。部分 hint 经 core 投影至源 MIP 变量，不自动填充辅助变量，也不保证后端采用提示。零上限不搜索；完整可行 hint 可作为 incumbent 返回，不能据此宣称实际后端求解通过。

`requestConflict: true` 调用既有 `solve_constraint_programming_with_conflict`；当前只请求整个提交模型的 seed，不提供 assumption-aware 删除收缩，不宣称 IIS/不可约集。证据经过 core 不可行证明 gate，已有权威证据保留；无有效证明时报告保留 UNAVAILABLE/问题码。`infeasibilityEvidenceJson` 返回原始证据文本，`diagnosticIssueCodes`/`diagnosticExtension` 读取统一诊断；这些对象同样遵循独立报告和释放规则。

Circuit、Automaton、Reservoir 可在 WASM 建模与源赋值验证，但当前原生降维不支持。Cumulative 降维按 core 默认关闭，SDK 不静默开启。HTTP/N-API 验收均要求这四类明确返回 `UNSUPPORTED`；Element、表、重化、变量时长/负 presence 区间则具有真实 SCIP/Gurobi 求解验收。

原生报告保持 `SolveReport<i64>`。`ConstraintProgrammingReport.fromJson(reportJson, snapshot)` 校验统一报告 schema/不变量、源指纹、完整稳定 ID/canonical 赋值、原 CP 可行性、解池赋值及精确目标。值/目标/typed 界读取为 bigint，values 返回 BigInt64Array 副本，完整文本保留精度；快照释放后报告仍有效。不可行、限时、取消沿用既有统一报告语义。源复验不等于重新证明最优性或后端证明；精确 CP 值不要通过 f64 `summarizeReport` 读取。core 预取消报告缺少指纹时，执行边界绑定已校验的提交快照身份。

## 首批可移植性审计

审计范围是当前绑定实际可达的调用路径，不是对所有 Rust public API 的 WASM 承诺。

| 依赖 | 当前绑定路径 | 限制及后续要求 |
| --- | --- | --- |
| 时钟 | 数值、数组、单位、共享二维几何、线性/二次编译、CP 快照、活动性分析和指纹不读取宿主时钟 | `SolveHandle`、远程停止确认和计时仍仅原生使用；新增 WASM 入口前需注入浏览器时钟或拒绝调用 |
| 随机数 | WASM 纯计算路径不使用随机源；直接 `scip`／`gurobi` 入口可按显式 `seed` 传入随机种子 | 随机搜索、启发式和生成器尚未开放；不能假定原生熵源在浏览器可用 |
| 线程 | WASM 不创建线程；每个 Worker 初始化自己的实例 | 原生执行器使用后台线程；不承诺共享 WASM 内存或跨实例对象引用 |
| 文件系统 | WASM 导出不调用文件系统；模型与报告以文本交换 | 既有远程本地对象存储仍是 Rust 原生 API，未作为 SDK 能力导出；Node 加载器仅负责读取包内 WASM |
| 异步运行时 | `remote-codec` 提取复用 codec，不启用 Tokio 客户端 | Tokio 多线程、远程调度、对象存储调用及 native solver 不进入 WASM 执行路径 |
| 精度与资源 | f64 建模、精确文本数值、TypedArray 复制、显式释放 | 不把 f64 模型声明为任意精度模型；不提供零复制长期借用或自动持久化 |

验证同时包含真实 wasm32 编译、Node wasm-bindgen 测试，以及浏览器/Worker/Node 的 SDK 加载；只有编译通过不足以声明可移植。

## 构建

现有基础库需要 nightly。此桥接使用 `nightly-2026-08-01`；较新 nightly 的 allocator API 已与现有 base 不兼容，不使用 `RUSTC_BOOTSTRAP` 绕过。

```powershell
rustup toolchain install nightly-2026-08-01
rustup target add --toolchain nightly-2026-08-01 wasm32-unknown-unknown
cargo +nightly-2026-08-01 install wasm-bindgen-cli --version 0.2.129 --locked
./scripts/build-wasm-bindings.ps1
./scripts/build-node-bindings.ps1 -Features scip-bundled,scip-quadratic
```

两个构建脚本默认将 npm 包放入 `target/npm`。WASM 脚本遵循 Cargo 的目标目录设置（包括 `CARGO_TARGET_DIR`）；Node 脚本仍输出到仓库内的 `target/npm`。WASM 生成声明引用 TypeScript 的 disposable 标准库，要求 TypeScript 5.2 及以上。正式 SDK 使用精确版本依赖；本地允许用这些 tarball 联调，不把相邻源码路径写进依赖。

Node 包首批仅支持 Windows x64。Gurobi 需要安装匹配的 runtime 和许可证；不要重新分发其 DLL。SCIP bundled 包仍须检查依赖运行库及其许可证，不能仅凭 `.node` 编译成功宣布可分发。

实际加载的 SCIP 运行时必须与构建绑定时使用的头文件版本完全一致，包括补丁版本。已加载但版本不一致时，创建 SCIP 问题前返回 `ENVIRONMENT` 错误；DLL 缺失或不兼容时可能在加载阶段更早失败。能力查询只表示编译了哪些功能，不保证运行时可用。

## HTTP

```powershell
$env:OSPF_TOKEN = '<至少 32 个可打印 ASCII 字符的随机凭据>'
$env:OSPF_ORIGINS = 'http://localhost:5173'
cargo +nightly-2026-08-01 run -p ospf-rust-host --features scip-bundled,scip-quadratic
```

默认监听 `127.0.0.1:8787`，可通过 `OSPF_BIND` 改为其他 loopback 地址。非 loopback 地址会被拒绝。所有业务路由要求 `Authorization: Bearer ...`；浏览器预检由 CORS 层处理。

| 方法 | 路径 | 行为 |
| --- | --- | --- |
| GET | `/ospf/v1/capabilities` | 协议版本及编译能力，不伪称运行时已就绪 |
| POST | `/ospf/v1/jobs` | 提交 `SolveRequest`，返回 202 与任务视图 |
| GET | `/ospf/v1/jobs/{id}` | 状态和最新进度 |
| POST | `/ospf/v1/jobs/{id}/cancel` | 请求真正的后端取消，不提前标记完成 |
| GET | `/ospf/v1/jobs/{id}/result` | 终态视图，运行中返回 Busy |
| DELETE | `/ospf/v1/jobs/{id}` | 释放终态任务；运行中不允许删除 |

默认一个活动求解，最多保留 64 个任务，不排队、不持久化、不自动恢复；消费报告后显式释放。HTTP 请求体最多 9 MiB，内层模型文本最多 8 MiB。公网使用受控 HTTPS 反向代理；本服务不是多租户调度平台。

宿主的 `http_network` 测试通过真实 loopback TCP 验证认证、CORS、SCIP 求解、进度、取消和关闭。服务端与 Rust 测试客户端位于同一进程，不能替代独立宿主可执行文件及 SDK／浏览器 HTTP 验收。SDK 另通过真实 socket 和 fake HTTP 服务测试断链与取消竞争。

`SolveRequest.modelKind` 是协议 1 的可选增量字段：省略时沿用 `linear`；`quadratic` 选择独立的二次 codec 与原生求解路径，未知类型被拒绝。字段由 Rust 生成声明，SDK 在提交前检查 `quadratic` 能力，不把旧版线性宿主当作二次宿主，也不静默降阶。

`cp` 选择 canonical CP codec 与原生严格有限降维。SDK 在提交前要求 `cpExactLowering`；协议主版本仍为 1，不推断旧宿主支持 CP。

可选 `SolveRequest.cpOptions` 只适用于显式 `modelKind: 'cp'`；SDK 在发送前额外要求 `cpSolveOptions` 能力，无配置则省略字段，保持既有 CP 默认路径。旧宿主不会静默丢弃 hint/limit/conflict 请求；线性/二次请求携带该字段在原生端被拒绝。DTO 字段由 Rust 生成，未知配置字段拒绝。

## 数据边界

### 原生串行 framework 编排

`ospf-wasm/framework` 的 `SerialCombinatorialLinearSolver(client, configuration)` 调用宿主内置 `serial-linear` 策略，直接复用 Rust `SerialCombinatorialLinearSolver`，不在 TS 重写 fallback 算法。Rust 生成 `SerialLinearOptions`、`NativeBackend` 和 `SerialFallbackPolicy`。后端顺序必须显式、非空且不重复，所有配置后端必须已编译；不会静默跳过未编译后端或切换 HTTP/Node 执行路径。配置被独立复制，不接受 JS 回调。`serialOptions` 仅允许 solver 为 serial-linear 且模型为 linear/省略，不能与 CP 配置混用；旧宿主缺少 `serial-linear`/`serialLinear` 能力时提交前拒绝。

默认 stopOnCertifiedTerminal：首个 incumbent 或有可靠证明的数学终态结束 fallback；continueOnTerminal 在有可靠证明的不可行/无界后仍尝试后续后端，但首个 incumbent 仍停止。取消、中断及超时始终结束链；无 incumbent 的超时保留 TIME_LIMIT，而不是被重写为 BACKEND_FAILURE。时间上限是每个实际 backend solve 的限制，不是编排端到端 deadline。进度为最近子求解器的原始快照，不代表整条链的总百分比；任务终态以 JobView.status 为准。任务仍共享有界单活动执行器、取消句柄、轮询和 release 机制。

此策略的 JobView.reportJson 是原始 `CombinatorialSolveReport<f64>` 文本，不是单独 SolveReport。用 `CombinatorialReport.fromJson(text, artifact)` 读取；共享 Rust codec 验证 schema、core aggregate/attempt 契约、源线性模型指纹和 incumbent/pool shape。它不独立证明可行性/最优性。`summary()`/`reportJson()` 读取最终统一报告，`attemptIds()`/`attemptJson(id)` 保留每个 attempt 的原始轨迹（含错误及终态摘要，不是每次求解的完整报告）、`selectedAttemptId()`/`selectionReason()` 保留选择依据。精确 u64 元数据保持 Rust 文本，不经 JS JSON.parse 再编码。报告独立持有数据，释放源模型/任务后仍有效，并需单独 dispose。

运行时/许可证失败保留在 attempt 诊断内；当没有任何可用结果时，沿用 core 的 UNKNOWN/BACKEND_FAILURE 聚合报告而不伪造成功。无 incumbent 的预取消或全失败聚合缺指纹时，只在已校验提交模型的原生执行边界补源身份。其他列生成/领域策略、其余 Benders、任意 domain context/pipeline/策略注册尚未接入；固定连续追索见专节，不能据此宣称 framework 全覆盖。

### 原生并行 framework 编排

`ParallelCombinatorialLinearSolver(client, { backends: ['scip', 'gurobi'], mode: 'best' })` 通过宿主 `parallel-linear` 预置策略（能力 `parallelLinear`）执行实际 Rust 并行组合。`ParallelLinearOptions` 和 `ParallelCombinatorialMode` 由 Rust 生成。后端列表显式、有序、非空且不重复，最多两个 worker，所有配置后端均须已编译。省略 mode 沿用 Rust 默认 best：等待全部 worker，按源模型目标方向选择，平局按配置顺序；无 incumbent 时保留首个有可靠证明的数学终态，不伪造后端失败。first 接受首个可选报告，向其余 worker 请求合作式 FrameworkLoser 取消，**仍等待全部 worker 回收才完成任务**，不保证立即返回或获胜者已全局最优。

`parallelOptions` 只接受线性模型，不能与 serialOptions 或 CP 参数共存。HTTP/N-API 共享原有有界单活动任务执行器；并发仅发生在单任务内部，不开放并行任务提交。时间限制独立作用于每次后端求解，不含模型解码或端到端 deadline。进度是最近子求解器快照，worker 事件可交错，不是单调的整体进度或任务完成标记；只有 JobView.status 表示全部 worker 回收后的任务终态。外部取消传播到子句柄，取消 loser 不取消父任务，也不改写已完成后端结果。共用 CombinatorialReport 读取轨迹、精确元数据、选择与源身份；WASM 不启用并行运行时。

线性/二次模型沿用现有 schema 1.0，正负无穷边界以字符串表示；不允许 NaN。MetaModel 整数建模仍限精确 f64 范围；独立 CP builder 使用完整精确 i64。模型文本和统一报告在 HTTP/N-API 中作为字符串传递；TS 不解析并重写原始载荷。摘要由 Rust 校验后读取。指纹来自 core，正常求解终态以 `SolveReport` 表达。

### 精确 CP 与活动性分析

`snapshot.withAssumptions()` 直接调用 core `snapshot_with_assumptions`，支持 literal（含 negated）、equal、lowerBound、upperBound、sparseDomain。`variableId` 显式指向接收快照的稳定 ID，不接受 builder 变量句柄；源快照不变，派生对象独立持有数据，重复身份、非子集域、非法值及未知 ID 由 Rust 拒绝，临时批次在成功/失败路径均释放。输入整数保持 bigint/i64。相互矛盾但单独有效的假设仍形成不可行模型，不会静默忽略。派生指纹复用 core canonical 编码；原生报告必须使用派生快照复验，不能使用原快照。这不是增量 session 或 assumption-aware conflict 诊断。

`snapshot.analyzeMipLowering()` 在 WASM 同步执行与原生端相同的完整默认预算执行降维，并释放临时 MIP；非零目标常数的固定辅助列计入统计与预算。不调用求解器，不查询运行时/许可证，不证明可行性。Rust 生成 `MipLoweringAnalysis` 判别联合类型：lowered（指纹与统计）、unsupported（约束与 formulation）、inexact（原值文本）、budgetExceeded（资源/预算/需求）及 invalid（说明）。失败返回 core 的首个降维问题，不承诺列出所有不支持约束。预检本身不支持取消或自定义预算；成功也不保证原生后端能力、就绪或求解成功。Cumulative 仍按默认关闭。协议主版本未改变，派生快照继续使用已支持的 canonical CP 载荷。

`ConstraintProgrammingModel` 持有真实 core CP builder，与 f64 `MetaModel` 分离。工厂覆盖当前 core AST 的全部约束族：整数关系、布尔 literal/and/or/xor/atMostOne/exactlyOne、implication、reification、表达式 allDifferent、Element、允许/禁止赋值表、noOverlap、Cumulative、Circuit、Automaton 和 Reservoir。运算、AST 语义与验证均在 Rust，修改通过 clone→freeze 校验→提交保持失败原子性。这是约束族覆盖，不是 core 所有 public 操作均已绑定。

`allDifferent`、表列、Circuit 后继与 Automaton 输入接受所属变量或整数表达式。`element({ index, values, target })` 保留 bigint 常量表，整个索引声明域必须落在表内。`implication(literal, constraint)` 与 `reification(literal, constraint, direction)` 捕获独立后件 AST；重化方向为 equivalent/impliedByLiteral/impliesLiteral，literal 接受变量或 `{ variable, negated }`。表在创建 TypedArray 前验证行宽，保留空表/零列元组语义；绑定侧最多 1,048,576 行，与较小的原生降维预算独立。临时表达式批量容器及变量转换句柄由 SDK 在成功/失败路径内释放。

`addInterval` 的 duration 接受 bigint 或本模型的非负整数变量；presence 接受变量或负布尔文字，presence 变量使用 `IntegerDomain.boolean()`。Cumulative 任务使用区间及固定 bigint demand/capacity。Automaton 状态为创建 TypedArray 前检查范围的 i32 number，标签为精确 bigint；调用者已创建并截断的 Int32Array 无法恢复。Reservoir 接受时间/变化表达式及精确液位边界，同时间变化由 core 聚合后再检查。所有新增工厂沿用身份、快照与释放规则。

`IntegerDomain` 保留稀疏域及完整 i64 范围；输入使用 bigint，单值范围校验后以文本传递，批量使用 BigInt64Array。SDK 在创建 TypedArray 前检查 readonly bigint[]，避免越界自动截断；调用者已经创建并截断的 BigInt64Array 无法恢复。全域基数为 bigint 的 2^64。表达式 evaluate 使用 builder 注册顺序；快照赋值必须遵循 `variableIds()` 的 canonical 稳定 ID 顺序，两者可能不同。

`freeze()` 返回独立不可变快照，源 builder 修改/释放后仍有效；模型变量、表达式和约束则在模型释放后失效，每个 WASM 句柄仍需分别 dispose。`toJson/fromJson` 复用 core canonical artifact codec，校验 schema、身份、指纹、canonical 字节与 artifact digest；`toArtifact/fromArtifact` 额外验证外层指纹。内层文本不得经 JS JSON.parse 再编码。`intervalValuesJson` 验证完整可行赋值，省略 absent 区间，以文本保留精确整数。导入/工厂覆盖不代表可无限制求解；原生路径显式使用 `modelKind: 'cp'` 和严格有限降维。

`ConstraintActivityAnalyzer` 对 CP 完整候选赋值或指纹校验后的线性 artifact 做本地活动性分析，不需要求解器；CP 候选可违反约束以形成诊断。`integerLhs/integerRhs` 为整数关系返回精确 bigint，原始报告文本同样保留整数；其他证据返回 undefined。slack、归一化与变量边界指标仍为 core f64 指标，不是全精确保证。报告导入检查已支持 schema 和 core 不变量；报告生命周期独立于源模型/快照。这不是 IIS、冲突提取、敏感性或再求解入口。

WASM 对象由 SDK `dispose()` 释放；TypedArray 返回值是副本，不是长期借用 WASM 内存。变量仅能用于所属且未释放的模型。

对象型线性建模通过 `MetaModel.linearExpression(terms, constant)` 或变量的 `toLinear()` 创建 `Linear`；运算复用 Rust math 的多项式、合并同类项和求值实现，TS 不合并系数。`add/sub/mul/neg` 返回独立对象；`le/ge/eq` 返回持有表达式快照的 `LinearInequality`，通过 `addInequality` 注册。约束常数在 Rust 中移到右端，拒绝溢出；当前中间模型没有独立目标常数，因此非零目标常数明确报 `Unsupported`，不静默丢失。

普通线性与二次导出均检查往返 core 指纹以及实际求解器变量类型；后者不包含在 core 指纹中。codec 无法保留的执行元数据或类型会报 `Unsupported`。独立 compiled-linear codec 在显式子模型执行路径中保留这些类型及元数据，不会自动回退。

带非零约束多项式常数的原始 Rust 二次模型也在此导出边界拒绝：core 指纹不覆盖该常数，移到右端时可能因舍入丢失。WASM 构造的关系在注册前已归一化常数。

表达式和关系均绑定到原模型；底层 Rust 同样检查模型身份。模型释放后所有关联对象失效，但仍须分别释放其内存。释放原表达式不影响已经捕获的关系或已注册的约束。添加变量不会重排已有索引；系数副本补零到当前变量数量，求值必须传入当前完整变量顺序的 Float64Array。

WASM 持久持有真实 Rust MetaModel，不在每次修改时从 DTO 重建变量。`variable.token()` 和 `model.tokens()` 获取真实 token 的独立句柄，`id()` 为本模型/实例内稳定的 bigint，不跨 HTTP 或进程复用。`addLinearSymbol(name, expression)` 和 `addQuadraticSymbol(name, expression)` 分别注册真正的 Rust 线性和二次表达式符号；对应的 `toLinear()` 和 `toQuadratic()` 保留当前符号为后续注册的直接依赖，`dependencyIds()` 返回 BigUint64Array 副本。两者共享名称空间并在 Rust 中保留原始项和求值；释放句柄不注销模型内注册。同名符号和跨模型引用被拒绝，其他符号特化仍未开放。

`setVariableRange(variable, { lowerBound, upperBound })` 事务性替换 Rust token 的范围，不改变 token 身份或先前导出的模型。连续／整数变量省略端点表示无界，二元变量省略端点分别默认为 0、1；整数端点仍限于 f64 精确整数范围。更新成功会清除显式赋值；非法范围或跨模型引用不会改变模型和原赋值。

`setSolution(Float64Array)` 检查完整 shape、有限值、范围及整数性后显式赋值，**不是约束可行性或最优性证明**。非法赋值保持旧值，成功的结构修改清除赋值，`clearSolution()` 可显式清除。`token.result()` / `symbol.evaluateSolution()` 在所需赋值缺失时返回 undefined；模型释放后全部句柄失效。

`Linear.mul(Linear)` 复用 Rust 多项式乘法得到 `Quadratic`，`toQuadratic()` 无损提升。二次式支持加减、标量缩放、完整变量顺序求值、稠密线性系数及稀疏二次系数副本。交换项规范为 row <= column 后交给 Rust 合并；稀疏项是 `coefficient*x[row]*x[column]`，无隐含二倍因子，不是 Hessian。`tryToLinear()` 仅在二次项全部消去后成功；模型身份、释放、有限值与溢出检查保持一致，降阶保留符号依赖。

`Quadratic.le/ge/eq` 捕获关系快照，通过 `addInequality` 真正注册到 Rust MetaModel。`setQuadraticObjective` 将线性部分注册到 MetaModel，二次项在 Rust 四角模型编译边界写入 Q；没有修改原有 core 线性子目标的泛型契约，也不在 TS 拼模型。`toQuadraticTetradModel` 复用既有二次 codec 和 core 指纹，编码时检查往返一致性，拒绝 codec 无法保留的信息。非零目标常数仍报 `Unsupported`。切换为线性目标会清空 Q，新增变量会按新维度补零；线性导出拒绝任何二次约束或非零 Q，不静默丢弃。

`solveQuadratic` 通过 HTTP/N-API 执行原生 QP/MIQP/QCP，复用进度、取消及报告生命周期。SCIP 需要 `scip-quadratic` 加上对应的链接/runtime feature；Gurobi 使用匹配且互斥的版本 feature。浏览器只构建模型，不在 WASM 内执行原生求解器。

## 验证与发布

```powershell
cargo +nightly-2026-08-01 test -p ospf-rust-bridge -p ospf-rust-native -p ospf-rust-host --lib
cargo +nightly-2026-08-01 test -p ospf-rust-framework --features remote-solver --test checkpoint_wire_contract
```

发布顺序为 Rust 底层包 → SDK 集成验收 → npm `ospf-wasm`。生成的声明、JS glue、WASM、原生二进制和报告不得入库。初始构建流程生成候选包，不自动向 npm 发布。真实 Gurobi 验证需要单独配置环境；fake 仅用于合同测试。

### 原生固定连续追索 Benders

`FixedRecourseAlgorithm(client, configuration)` 通过 `fixed-recourse` / `fixedRecourse` 调用实际 Rust framework 算法，显式选择 `scip` 或 `gurobi`，只接受线性源模型。`FixedRecourseOptions`、阶段、停止原因和摘要类型由 Rust 生成。这是一条内置策略，不代表任意 Benders、列生成或领域 pipeline 注册已完成。

`stages` 使用源交换模型的列索引，不传进程内 token ID。非零成本的连续追索变量须显式声明：省略时沿用 Rust 默认规则，整数及非零目标变量留在 master。整数追索及不支持的划分明确失败。分区、证书及割均复用既有 Rust 实现；可在剩余预算内恢复缺失证书，不替换无效的已有证据，也不回退为完整 MILP。初始化有限主问题界可能需要有界追索盒；不支持的情况保留 `unsupported`。

`timeLimitMs` 是算法求解总预算，包含 master、subproblem 与证书恢复，每次后端接收剩余时间。codec/partition 初始化在计时前完成，不是可中断的端到端 deadline。进度仍是最近子求解器的原始快照，不代表整体百分比。HTTP/N-API 共用有界单活动任务和合作式取消。划分错误可能发生在创建任务之后；SDK 错误保留 `jobId`，便于释放失败任务。

`FixedRecourseReport.fromJson(job.reportJson, artifact)` 独立持有 Rust 数据。`summary()` 暴露实际 stop、源列顺序的完整 incumbent、源方向目标/最优界、轮数/割数、原生终态状态/原因及保留的后端错误；最大化时最优界为上界。迭代/时间/取消终态可保留 incumbent，不伪造成功。`masterReportJson()` / `subproblemReportJson()` 以精确文本保留最后真实子报告及其自身证明/身份，证书恢复终态另存于摘要。它们**不是**源模型统一 SolveReport，也不是独立源模型最优性证书。导入复验 schema、源指纹、完整赋值/域、目标、界一致性、计数及 core 子报告不变量，不重放割以证明 bound。任务 release 与报告 dispose 相互独立。

可移植 Rust 报告读取器显式使用 `u64` 元数据计数（统计、来源及迭代轨迹）；原生执行 API 保持默认 `usize`。超过 2^53、直至 u64::MAX 的整数继续以报告文本保留，不经过 JS JSON number；SDK 只解析生成的安全摘要。


### 原生同源投影列生成

`ColumnGenerationAlgorithm(client, { backend, blocks })` 调用原生 `column-generation` / `columnGeneration` 策略，配置、块、停止原因和摘要由 Rust 生成。块显式映射不同的源交换列索引，未映射变量保留为全局变量。每块表示恰选一列的局部配置，**不是**任意装箱领域可无限重复选图案的主问题。空白/重复块、重复/未知变量和任意回调明确拒绝。上限为 256 块、100000 个映射变量、10000 轮 RMP/列；默认 128 轮、10000 列、容差 1e-7。

实际 Rust `ProjectedColumnGeneration` 全程复用 `ColumnProjection`：空池人工变量 Phase I（无大 M）、独立校验 RMP 最优 LP 证书和行映射、保留源边界/整数性/局部行的原生定价、每块每轮最多一列、整批加列、旧对偶失效与重建，最后求受限整数主问题并复核完整源赋值。跨块/全局行仅通过对偶进入局部目标。定价主问题去除凸性已隐含的列上界，避免冗余上界对偶造成重复负检验数；原有普通 master 默认不变。SCIP 显式使用 LP 子问题预设以保留可用对偶。

`timeLimitMs` 是包含 RMP、定价和 final 的总求解预算，各子求解接收剩余时间；源 codec/决策映射初始化在计时之前，编译过程不可抢占。省略时 Rust 默认 60 秒。进度为最近子求解器原始快照，不代表整体百分比。HTTP/N-API 共用取消、任务存储和释放，取得任务 ID 后的错误保留 `jobId`。提交响应丢失可能留下客户端无法识别的任务；宿主没有任务列表或幂等提交接口。无完整源 MILP 回退、隐式后端替换或 TS 回调到 trait 的转换。

`ColumnGenerationReport.fromJson(job.reportJson, artifact)` 复验 schema、源身份、计数、完整原域赋值/目标及 core 子报告契约。`summary()` 区分 Phase I 完成、Phase II 定价收敛、限制/停滞和实际子问题终态；不可行/无界保留在子报告，不伪造成源证明。`masterReportJson()` / `pricingReportJson()` / `finalReportJson()` 以无损文本保留最后真实报告及 u64 元数据，`pricingBlock` 标识最后定价块。final **仅使用已生成列**：定价收敛和受限整数最优都不证明源 MILP 全局最优，受限不可行也不证明源不可行；因此不暴露源 `bestBound` 或伪造源 SolveReport 证明。迭代/列数耗尽后若 Phase I 已完成，仍可求当前池的 final；超时/取消不启动新的 final，但已运行 final 返回的有效 incumbent 继续保留。报告 dispose 与任务 release 独立。

任意 context/pipeline 注册、动态删列策略、branch-and-price 和完整领域流程仍待后续接入，不由 SDK 隐式模拟。

### 共享二维几何

独立子路径 `ospf-wasm/geometry2d` 提供显式输出单位的 `Geometry2d` context，以及从 Rust 生成声明重导出的自然 DTO 名称：`QuantityPoint2`、`QuantityVector2`、`PackingShape2`、`Box2Geometry`、`Placement2`、`Sheet2Shape`/`Sheet2Geometry`、`Shape2Candidate`。坐标和尺寸可混用 m/km/cm/mm，Rust 先归一到原共享内核的 Meter/f64 特化；输出统一到 context 单位，面积使用对应 `m^2`/`km^2`/`cm^2`/`mm^2`，不冒充长度。原 `Quantity` 同步支持这些面积单位和量纲检查。

点/向量加减、先 y 后 x 比较、90 度旋转/候选朝向、实际/包围面积、节距、形状/放置膨胀、盒平移/重定位/边界、点/盒/板材包含均委托原 Rust API。`overlapPairs` 最多接受 512 盒，返回全部重叠对的交错 u32 索引复制 `Uint32Array`（i,j，i<j，按输入顺序），不截断、不提供穷尽装箱或求解证明。DTO 与数组是独立数据，仅 context 需要 dispose；它们可传给其他 context，不是跨模型句柄。

维度不匹配、非法形状、非有限数、非正尺寸、负间距、数值溢出/下溢或坐标精度导致边界塌缩明确报错；文本输入限 8 MiB，itemId 非空时限 1024 UTF-8 字节。保留原 f64 严格比较，不暗加容差，单位转换仍受浮点舍入影响。`rotated` 仅记录实际形状的朝向，不自动再次交换宽高。

**现有语义边界**：原 `Box2Geometry` 对圆使用外接正方形，重叠是包围盒内部重叠、包含是包围盒闭区间，不是精确圆盘碰撞/包含。斜向分离圆盘的外接方形仍可能重叠。原 `inflated_by_placement` 保持左下角，不是中心不变的膨胀；此前原注释已修正，不改变底层行为。这个桥接不等同于 BPP2D/CSP2D 求解、领域 geometry guard 或任意高精度值的 WASM 导出。

### BPP1D 领域桥接

`ospf-rust-bridge` 生成单位/BPP1D DTO 声明，提供 schema 1.0 canonical 问题封装、独立命名空间 SHA-256 身份、有界默认组合生成及物理结果/报告复验。需求、物料、库存、组合件数/次数等精确 u64 通过 canonical 十进制**字符串**跨 JSON；安全计数限界为 u32。SDK 在独立 `ospf-wasm/bpp1d` 子路径消费生成契约并提供 bigint/Map，不依赖相邻 Rust 源文件。单位新增 kg/g，与 m/km/cm/mm/s/min/h 一样由 Rust 运行时量纲检查。

纯计算开放单位归一、组合校验/生成、精确总需求用量复核、有界物理展开、坐标方案复验、物料和 KPI；原生端通过实际 `Bpp1dMilp` / `Bpp1dColumnGeneration` 求解；WASM 仅计算/读取报告，不运行原生求解器。已绑定：严格 CSV 双表导入（`PackingContext.fromCsv`）、renderer（`render`）、原生尾箱优化（`tailWindow`）。尚未绑定：BPP1D 离散装载、自行提供 policy/context/pipeline、任意 TS 回调。报告导入不独立重放穷尽搜索以证明原生全局标记。

边界为 1000 物品种类、128 箱型、10000 候选、100000 搜索节点、100000 展开件数/箱数、8 MiB 文本。候选默认/最大预算 10000/100000，展开默认/最大 100000；原生轮数/每轮列数默认 32、最大 10000。单位归一使用原 `PackingContext` / `RuntimePackingValueAdapter`；间距仅相邻件之间、无尾间距，余料 KPI 仅尾余。用量验证可不展开地支持 u64 上限；不承诺浮点后端能求解所有此类输入，原精确转换门禁保留。

协议主版本仍为 1：请求必须显式 `solver: bpp1d`、`modelKind: bpp1d` 和独占 `bpp1dOptions`，不能混入 CP/其他 framework 选项。客户端检查 `bpp1d`、模式专属 `bpp1dMilp`/`bpp1dColumnGeneration` 及选定后端编译能力。worker 内部重建非 Send 的领域/MetaModel 对象，调用原领域统一 MILP/LP/final 注册，不在桥接手拼矩阵。HTTP 与 N-API 共用单任务、进度、合作取消和释放机制。默认 60 秒或显式 `timeLimitMs` 的总预算覆盖生成/seeds、LP 与 final，子求解接收剩余时间；codec 初始化及有界但不可抢占的生成/编译不是严格硬截止。后端通用预取消/中断报告只有在无 incumbent、无 proof 时才补实际子模型缺失身份/mapping，不覆盖冲突身份。

此处列生成是**有限池列激活**，并非通用同源投影 DW：预算允许时，定价迭代上限后仍可求解完整当前 seeds/候选池 final。候选截断必须保留 `candidateComplete: false`，不能声称全局装箱最优或不可行。全局标记严格保留原生默认规则、完备候选及实际证据范围。领域报告单独绑定源问题，原始 SolveReport 保留真实候选模型身份/证明，物理坐标、精确需求/库存/物料、KPI 和目标重新验证。WASM 使用 u64 解析子报告计数，不经 JS number。任务、context、候选、报告及独立物理结果分别释放。其余领域除以下 CSP1D 范围外尚未导出。

### BPP2D 共享原生执行边界

`NativeEngine` 仅接受 `solver: bpp2d`、`modelKind: bpp2d` 和独占显式 `bpp2dOptions`，创建任务前复验源身份／预算，在 worker 内构造非 Send 领域对象，源解码、生成及所有子求解共享时间预算。所选已编译 SCIP/Gurobi 后端执行原列生成 application，SCIP 使用既有 LP 子问题默认配置；HTTP 与 N-API 共用源绑定报告编码，不回退求解器，不改写取消。默认 8 定价批次、每批观察 128 候选、池 512、196 子报告，支持关闭 final 及显式预注册生成链。禁用 final 或预算提前停止不伪造生产；final 已完成后父取消不改写原报告。原始子进度、线程安全轮询、容量及任务释放复用已有执行器。

SDK 从 `framework` 和 `bpp2d` 导出 `Bpp2dColumnGeneration`，快照 Rust 生成契约配置，提交前要求领域／算法及显式原生 LP/MIP 后端能力。进程内认证 HTTP 路由测试实际运行 SCIP/Gurobi Phase I／定价／final、报告预算、关闭 final、取消及结果获取；隔离 npm 测试运行真实 N-API。独立浏览器／SDK HTTP 验收仍要求显式宿主，缺省跳过而不计为通过。非默认质量／承重／规则扩展仍是独立未完成项。

### BPP2D 有界压缩装箱与渲染

`PackingContext.packLayers(layers)` 将每个输入列作为一张独立板材运行原 `Packer`、`FreeSpaceAnalyzer` 和 `PackingRendererAdapter`，不证明源需求满足。`Bpp2dReport.packing()` 仅从已复验的 final 整数生产派生；无 incumbent 时返回 `undefined`。两者每次返回独立的 `PackingResult`，须单独 `dispose()`；销毁 context、报告或生产副本不影响已有装箱副本。装载顺序按 y、x、物品身份稳定排列，结果保留输入位置，不以坐标量化作为身份。

`patterns()` 保留完整池索引、bigint 份数、物理层、带单位渲染放置、装载顺序、装载率及单板余料。份数不展开；`sheetCount()`、`itemCount()`、`materialCounts()` 用 checked u128 累计后转换为 bigint，可超过 2^53。默认物料键沿用原装箱器的成品物品 ID，不声称接通原材料、供应商、质量或承重规则。渲染长度为米，面积为平方米；纯 DTO／Map 为独立副本。

装箱独立限制：每个选中排布最多 128 放置，所有选中排布的保守余料工作量 `sum((2*n+1)*(n+1)) <= 65536`，输出不超过 8 MiB。超限返回 `PayloadTooLarge`，不截断、不使已读报告或生产结果失效；报告仍允许原 512 列／4096 放置。原余料分析保留其 1e-9 切片容差，对圆扣除外接方形，报告未扣除刀缝损耗，不能将余料面积等同于整板减实际形状面积，也不是可回收库存／切割工艺证明。此处提供 typed 渲染数据而非 SVG／Canvas UI，TS 不重写装箱或几何算法。

### CSP1D 可移植子集

`ospf-wasm/csp1d` 消费 Rust 生成契约，提供 `CuttingContext`、独立 `PatternGenerationResult` 与 `ProductionResult`。schema 1.0 artifact 保留 canonical Rust 文本和领域分离 SHA-256 身份。单位转换、源产品/物料引用重建和需求贡献全部复用原领域公式；`verifyUsages` 复用 context/MetaModel 注册及完整 snapshot 校验，再用 checked u64 独立复验离散需求、库存、设备批次数及溢出，不逐件展开。

当前仅开放 Roll/Sheet、静态产品幅宽和默认物料/设备幅宽/批次数规则。受原 f64 领域限制，每个输入 count 在 TS 为 bigint、wire 为 canonical 文本，范围 [0, 2^53-1]；需求/使用次数/切片份数必须为正。更大输入明确拒绝、不舍入。checked 合计输出可超过 2^53 并保持 bigint/文本，物理宽度仍为 f64。最多 64 产品/需求、16 物料/设备、每产品 8 幅宽、512 方案/用量、每方案 64 组切片/刀、8 MiB 文本。源 ID 为 1..128 ASCII 字母/数字/下划线/点/短横线，方案 ID 同字符集最多 512 字节。

候选委托原 Simple 或 N-Same（默认）；maxPlans 默认 128、最大 512，maxKnives 默认 16、最大 64。保留真实生成统计及截断，**candidateComplete 始终为 false**，不将单产品策略当作任意混合模式空间的穷尽。实际纯计算路径不使用时钟、随机数、线程、文件系统，不因此承诺未调用的原生/benchmark 路径可移植。浏览器、模块 Worker、Node ESM、生产打包和隔离包消费者使用真实 WASM 验收。Weight/Costar/动态长度/yield/waste/length/capacity/switch/自定义策略尚未绑定；通过验证的生产方案不代表最优性证明。

#### 原生子模型身份前置能力

`Csp1dStageReport` 在原始报告和独立目标常数之外，保留**实际提交的编译模型**。业务等价的 MetaModel 重建不保身份：编译行名可包含模型局部 owner。既有 schema 1 远程交换会合并无符号/特殊变量类型并省略行元数据，现保持不变，不将其重新命名后冒充完整执行模型封装。

运行时无关的 `remote-codec` 新增独立 schema 1 `CompiledLinearModelArtifact` 编解码，复用既有稀疏远程 DTO，分别保留准确 token/solver 变量类型（包含有意松弛的 solver 类型）、约束元数据的缺失/存在差异和实际行名。校验 canonical 字节、覆盖完整载荷的领域分离摘要、原始 core 指纹及稳定行列映射；无穷边界复用既有有符号文本编码。进程内变量 arena ID、显示句柄、可变 token 结果不是跨进程句柄，解码分配新局部身份。此封装保留 backend 数学/报告契约，不转移原 MetaModel 所有权；每个模型文本限 8 MiB。

Rust bridge 的阶段报告 codec 以保留的模型复验原始报告、完整向量/行/界/整数性、stable values、目标，以及最优 LP 阶段既有完整 primal/dual 证书。可移植门禁使用 `SolveReport<f64, u64>`，不把元数据降到 wasm32 usize；原生报告默认仍用 usize。目标常数与原始子报告目标分别保留。单独阶段 codec 不证明源问题全局结论，也不认证调用方子模型属于某个源问题。

#### 完整原生 CSP1D 执行

`Csp1dMilp(client, { backend })` 与 `Csp1dColumnGeneration(client, { backend, generation })` 通过显式 HTTP 或 Node 客户端调用原领域 service。请求使用协议 1、`solver: csp1d`、`modelKind: csp1d` 和独占 `csp1dOptions`；客户端检查领域、算法及选定线性/MIP 后端编译能力。不降级启发式、不在浏览器 WASM 运行原生求解器、不隐式回退 HTTP，也不注册任意 TS 回调。

初始候选默认 128；定价沿用选定 Simple/N-Same 策略及刀数限制。`poolLimit` 默认/最大 512，不能小于初始候选预算；`iterationLimit` 默认 8、最大 64；`columnsPerIteration` 默认 64、最大 512。`timeLimitMs` 默认 60 秒，生成和子求解共享总预算，各子求解接收剩余时间。有界生成与模型编译不能硬抢占，codec 初始化也不是端到端硬截止。进度是最近真实子快照，不是整体百分比；取消/完成竞争保留实际子求解终态。

`CuttingContext.readExecutionReport(text)` / `Csp1dReport.fromJson(text, source)` 复验独立 schema 1.0 封装（整体限 8 MiB）、源身份、有界配置、候选池/trace 计数、阶段顺序和保留的实际子报告。最终生产复用下述源绑定整数验证器。Rust 生成的 `summary()` 区分业务状态、执行停止、计数和受限整数结果。`traceJson()` 保持原 Rust trace canonical 文本；`stageJson(i)` 保留实际编译模型和原报告，`stageReportJson(i)` 返回精确 u64 原始报告文本。越界返回 undefined。trace 遥测和局部一致的 LP 封装不构成独立重放的源问题全局证书，也不是来源认证。

每次 `integerReport()`、`production()` 返回独立可释放副本，不可用时为 undefined。任务 release、context dispose 和报告 dispose 互相独立。无初始列、受限不可行、后端错误、超时、取消都不伪造产出。`candidateComplete` 始终 false，定价成功与 final 池内最优都不能证明无限制 CSP 的全局最优/不可行。

Phase I 现将人工短缺变量接入**原需求行**，避免重复 yield balance 行，并保留真实需求对偶身份；惩罚 key 按每条需求自身的 Roll/Sheet 单位构造。真实 SCIP/Gurobi 测试覆盖单列不可行初始池、定价恢复、混合 Roll/Sheet 需求和 final 提取。普通 hard-demand/yield 路径与旧启发式默认行为不变。

#### 源绑定整数生产报告

`encode_csp1d_production_report` 保留一个实际 MILP/final-MILP 子报告及其**完整**候选池，包括零使用列；独立 schema 1.0 封装限 8 MiB。`decode_csp1d_production_report(text, source)` 复验 canonical 源身份、重建默认领域方案及原 context，严格比较每个列名、token/solver 类型、边界、目标系数、完整行系数/RHS 和执行元数据。跨 context 数学对照不比较局部行身份，但原始报告仍须绑定保留的实际子模型指纹/映射，不改写身份。缺失/外来列、重复身份/物理模式、辅助列/自定义规则、整数列松弛均拒绝，不以池顺序猜测列对应。

WASM 和 SDK `ProductionReport` 通过 `CuttingContext.readProductionReport(text)` 开放此验证器。产出只从实际完整 incumbent 向量导出，整数舍入后重新检查完整模型，再通过原源 context 与精确 u64 规则复验；载荷不接受独立生产量声明。无 incumbent 时 `production()` 为 undefined，取消/限时/不可行保留原子报告状态/原因。`restrictedOptimal` 与证明门禁后的 `restrictedInfeasible` 仅描述当前候选池，`candidateComplete` 始终 false。指纹和数学一致性都不是报告来源认证。`solverReportJson()` 保持原始精确 u64 文本，不应经 JS number 解析再编码。每次返回的生产结果独立拥有，即使 context/报告已释放也可读取；摘要类型由 Rust 生成。真实 SCIP/Gurobi 验收将普通 MILP 与列生成的生产量直接对照原领域提取结果。此整数阶段 reader 与下述完整执行报告相互独立。

### Benders 后的完整源 MILP 收尾

显式设置 `sourceMilpFinishing: { reserveTimeMs: 3000 }`，在**同一个** `timeLimitMs` 总预算中预留完整源求解时间，
不是追加时限。预留量为正 u32 毫秒，严格小于总预算（省略总预算时为 60000 ms）。省略该配置保持原行为。
SDK 同时要求宿主声明 `bppLoadingSourceFinishing`，旧宿主在提交前明确拒绝，不静默降级。

```typescript
const solver = new DiscreteLoadingBendersApplicationService(client, {
    backend: 'scip', recourseResources: ['ballast'], iterationLimit: 1,
    sourceMilpFinishing: { reserveTimeMs: 3000 },
});
const job = await solver.solve(source, { timeLimitMs: 10000 });
try {
    const report = DiscreteLoadingBendersReport.fromJson(job.reportJson!, source);
    try {
        console.log(report.summary(), report.sourceFinishingInvoked());
        console.log(report.accelerationApplicationReportJson(), report.certificationReportJson());
        console.log(report.certificationError());
        const plan = report.solution();
        try { console.log(plan?.resources()); } finally { plan?.dispose(); }
    } finally { report.dispose(); }
} finally { await client.release(job.id); }
```

Rust 调用原 `DiscreteLoadingCertificationService`，独立重建等价完整源注册，仅复制已验证向量作为 warm start；
初始构造、加速、回退和中间 codec 工作消耗同一预算，收尾收到剩余时间。加速阶段在注册前耗尽自身预算时，
预留量仍可创建真实普通完整源求解，不伪造旧注册。构造／编译不可硬抢占，输出复验也不属于可抢占后端时限。
已验证完整源最优／终态证明优先原样保留，即使此时取消或预算耗尽，也不重复启动收尾。
旧取消／中断不启动新后端；收尾无解、非最优且更差或可恢复失败不丢弃已有方案。
保留旧方案时复用原 Rust 转换重建审计、gap 和 proof-free 边界，不搬用旧 trace、对偶或解池。

`summary()`、`solution()`、`applicationReportJson()`、`solverReportJson()` 返回最终选择结果；
`accelerationApplicationReportJson()` 保留收尾前的原 application 文本；`certificationReportJson()` 保存实际收尾 core 原报告，
即使其方案未被选择也保留。`certificationError()` 为实际可恢复错误；`sourceFinishingPresent()` 表示有收尾阶段记录，
`sourceFinishingInvoked()` 只在实际后端产生报告或错误时为 true，不能混同。旧报告的加速 getter 返回原 application。
加速报告的证明边界不变，源证明只能来自真实完整源回退／收尾报告。导入复验数学与选择一致性，**不认证外部 solver 来源**。

可选 `certification` 保留旧原子文本，协议主版本仍为 1；原子 core／收尾报告各限 8 MiB，application 各限 16 MiB + 1 KiB。
有收尾证据的外层响应限 128 MiB + 8 KiB；无收尾证据仍为 64 MiB + 4 KiB。HTTP 默认请求体 9 MiB 限制不变，
不把大旧报告重新上传。精确 u64 元数据保持文本，不经 JS number 解析再编码。
该入口不等于branch-and-price、自定义策略或 BPP3D 已绑定。

## 共享 BPP 分区生命周期基础对象

`ospf-wasm/bpp` 开放 `PartitionLoadingContext`、`PartitionLoadingMaster`、
`PartitionLoadingPricing` 和 `PartitionLoadingBatch`。`source.partition(options)` 独立拥有原领域注册，
释放源 context 不影响它。显式分区必须让所有源舱位恰归属一次；已声明资源自动跟随舱位归属，
`refinableResources` 是可省略的资源身份白名单，不接受裸 token 或回调。

`decisions()` 给出实际局部决策目录。`addColumns` 接受完整有限 `ReadonlyMap<string, number>`，
缺失决策不补零，整批验证通过后才修改列池。`columns()` 保留 bigint 货物／件数及源模型重量单位。
`removeColumns` 原子删除，列身份不复用。重建 master 或成功改变列池会使旧注册／定价 epoch 失效；
原子加列失败不改变旧注册。`acceptLpReport` 复用原完整最优 LP 证书、实际模型身份、mapping 和对偶门禁，
报告门禁失败会撤销旧价格；同一报告重验也不能复活旧请求。裸对偶数组不能代替完整报告。
批次拥有请求副本，`addPricedBatch` 在同一当前 epoch 原子提交所有分区候选。实际改变列池后旧 epoch 失效；空操作或全部去重而未改变列池时沿用原 Rust 的 epoch 保持语义。

master／pricing 的 `toModel()` 返回**不透明 compiled-linear 封装**，不是普通线性 codec：LP 松弛可能保留
原 token 类型，而实际求解类型变为连续变量。必须使用显式 `SCIPSolver.solveCompiledLinear`／
`GurobiSolver.solveCompiledLinear`，要求所选后端声明 `compiledLinear`；旧宿主明确拒绝，
不回退普通 linear、HTTP 或其他后端。Rust 原 compiled-model codec 保留实际类型及元数据。
`ModelArtifact.fingerprint`／`JobView.fingerprint` 是传输内容摘要；原 core 报告保留自身实际模型指纹，
二者有意不同，不可重标记为同一身份。原 master／pricing 行在注册时使用稳定身份而非进程内计数器，确保独立重建的实际子模型身份一致。`objectiveConstant()` 与后端 `c*x` 分开保存，不能塞进未修改的子报告。
pricing 的 `readReport` 验证完整实际局部模型（含原布局辅助变量），保留原报告文本及 u64 大计数，
返回局部决策及包含凸性价格常数的检验数。`reducedCost` 只是只读快照计算，不证明请求仍是当前 epoch。

```typescript
const partition = source.partition({
    partitions: [{ id: 'left', positionIds: ['a'] }, { id: 'right', positionIds: ['b'] }],
    refinableResources: ['ballast'],
});
// seedColumns 为已验证的完整分区 Map 向量。 / Validated complete per-partition Map vectors.
partition.addColumns(seedColumns);
const master = partition.master();
const job = await new SCIPSolver(client).solveCompiledLinear(master.toModel());
try {
    partition.acceptLpReport(master, job.reportJson!);
    const pricing = partition.pricing(master, 'left');
    try { console.log(pricing.toModel(), pricing.objectiveConstant()); }
    finally { pricing.dispose(); }
} finally { await client.release(job.id); master.dispose(); partition.dispose(); }
```

`master({ integer: true, refineResources: true })` 仅释放白名单连续资源，其他离散轮廓保持冻结。
`sourceValues`／`extractSolution` 必须使用完整 Float64Array，拒绝 LP／人工变量方案，并复验完整源模型与领域。
`materializeCompletedResources` 原子物化补全轮廓并使旧 master 失效，之后需重建整数 master。
返回物理方案、master／pricing 快照均独立释放；旧快照仍可读不表示可用于新会话。

限制：1..128 分区，池最多 4096 列／262144 决策项，每次直接加列最多 512 输入；
定价批次最多 128 分区／512 候选／262144 决策项。容量保守按已有加全部输入计算，重复列去重前也占预算。
文本 DTO／compiled 封装内部载荷限 8 MiB，池计数受限不保证 `columns()` 序列化必能放入该预算，超限 getter 明确失败。
源外层封装允许 16 MiB + 1 KiB；HTTP 请求体仍限 9 MiB。进度、取消、任务释放复用原生执行器。

这些基础对象本身不运行算法；原 `PartitionColumnGenerationApplicationService` 已由下述原生门面单独接入。
两个入口均不提供完整源 MILP 最优证明、branch-and-price 或 BPP3D。
一次定价及最优受限 final 不能证明完整源最优／不可行。真实 SCIP/Gurobi 验收这些基础对象；
WASM／浏览器／Worker／隔离 npm 包重建实际子模型与资源补全方案。独立 HTTP 未配置时记 skip，
不能冒充通过；认证的进程内 HTTP 验收单独执行。

## 原生根节点分区 Application

`ospf-wasm/bpp` 与 `framework` 导出 `PartitionColumnGenerationApplicationService`，
显式使用 HTTP 或 Node 客户端执行原 Rust service，不在 TS 运行迭代。请求为 `bpp-loading`／`bppLoading`
及独占 `bppLoadingOptions.partition`，与 Benders 互斥。SDK 要求 `bppLoadingPartitionColumnGeneration`
及所选原生 LP/MIP 后端能力；旧宿主明确拒绝，不回退普通 MILP、WASM、HTTP 或其他后端。

```typescript
import { PartitionColumnGenerationApplicationService, PartitionColumnGenerationReport } from 'ospf-wasm/bpp';
const service = new PartitionColumnGenerationApplicationService(client, {
    backend: 'scip',
    partition: { partitions: [{ id: 'left', positionIds: ['a'] }, { id: 'right', positionIds: ['b'] }],
        refinableResources: ['ballast'] },
    iterationLimit: 64, columnLimit: 512, completionColumnLimit: 128, reportLimit: 256,
    sourceMilpFinishing: { reserveTimeMs: 2000 },
});
const job = await service.solve(sourceArtifact, { timeLimitMs: 10_000 });
try {
    const report = PartitionColumnGenerationReport.fromJson(job.reportJson!, sourceArtifact);
    try {
        console.log(report.summary(), report.executionSummary(), report.refinementReportJson(), report.finalReportJson());
        const plan = report.solution();
        try { console.log(plan?.solution(), plan?.resources()); } finally { plan?.dispose(); }
    } finally { report.dispose(); }
} finally { await client.release(job.id); }
```

原流程按分区生成种子、执行 Phase I／根 LP／定价整轮原子提交、可选资源补全及轮廓物化，最后求无人工变量的
受限整数 final。SCIP 使用既有 LP 子问题预设以保留认证对偶；所有子求解接收剩余时间及同一取消句柄。
进度仍为最近子求解器快照，不代表整体百分比。worker 源注册、模型构造及子求解消耗同一总预算，
前置验证和后置编码／重放不是可硬中断的端到端截止。

`PartitionColumnGenerationReport` 使用预算观察与原子报告重放同一 Rust service，**不读取时钟或执行后端**。
逐次核对实际 compiled 子模型、完整 LP 证书、局部及完整源方案、目标、控制流顺序、单调剩余时限／取消观察和最终源选择。
`calls()` 保留有序实际模型、原报告文本及 u64 大计数；`executionSummary()` 派生根收敛、根／补全列数、
原轨迹及实际 refinement／final 调用索引。独立 getter 保留两个终态原报告，`accelerationError()` 保留已有源方案后的
真实可恢复错误。这是数学与控制流一致性验证，不认证外部 solver 来源。

根阶段派生源报告始终无 proof。`COMPLETED` 可能仅表示受限 final 完成，不能代替 `lpConverged`，也不能忽略根迭代预算。
根 LP 收敛、受限最优／不可行均不能提升为完整源 MILP 最优／不可行，更不是 branch-and-price。
原 service 在 final 错误或取消后可保留已验证资源补全方案；加速预算已终止时不新启根阶段后端。方案独立释放。
显式预留完整源收尾见下一节，任意 trait 回调仍未接入；BPP3D 仅绑定其长方体与轴感知圆柱单轮原生执行。

默认／范围：64 根轮数（1..64）、512 根列（分区数..4096）、128 补全新增列（0..128，有可补全资源时至少容纳每分区一列）、
256 子求解（1..1024）。子报告预算耗尽按原错误恢复语义处理：无已验证源方案时任务明确失败，不伪造成功报告；
最多额外记录一次预算保护错误调用。每份子模型／报告限 8 MiB，累计成功子文本限 32 MiB，预算观察最多 16384，
外层封装限 128 MiB + 8 KiB；HTTP 请求仍限 9 MiB。真实 SCIP/Gurobi、进程内认证 HTTP、隔离 npm／N-API、
浏览器与 module Worker 分开验收；独立 HTTP 未显式配置时记 skip，不算通过。

### 分区加速后的可选完整源收尾

在分区 application 配置 `sourceMilpFinishing: { reserveTimeMs: 2000 }` 显式启用。正预留量必须严格小于
`timeLimitMs`（默认总预算 60000 毫秒），从根阶段预算扣除，不追加到任务时限。SDK 另要求
`bppLoadingPartitionSourceFinishing` 能力。根 application 成功返回后，原生端复用
`DiscreteLoadingCertificationService`，通过全新等价完整源模型收尾，只将已验证完整数值向量作为 warm start。
根注册前超时可在预留预算内调用原普通源 service；先前取消／中断不新启收尾。若原根流程发生无法生成有效
application 结果的错误，任务仍明确失败；此选项不是任意加速错误的隐式重试或回退。

原根重放仍独立且无证明。`accelerationApplicationReportJson()` 保留其完整源报告原文；
`applicationReportJson()`／`solverReportJson()` 返回最终选定源结果。`certificationReportJson()` 与
`certificationError()` 保留真实完整源后端证据和精确 u64 元数据。`sourceFinishingPresent()` 表示已处理收尾阶段
（含前置停止），`sourceFinishingInvoked()` 则要求实际报告或可恢复后端错误。`calls()`、根轨迹、refinement／final
索引及 `accelerationError()` 始终只描述加速阶段。独立完整源调用不占根子调用次数／累计文本额度，单份报告仍限
8 MiB，并受同一外层封装及总时限约束。

可移植 reader 先重放根 service，再复用共享认证门禁，核对源身份、完整方案／目标、实际报告选择、保留较好旧方案
及审计／gap／证明重建。只有真实完整源报告能保留自己的源证明；无解／较差的未完成候选和可恢复收尾错误保留已验证
旧方案，矛盾的不可行／最优声明被拒绝。即使最终保留旧方案，真实认证原文也不丢弃。这验证数学与证据一致性，
不认证外部 solver 来源，也不等于完整 branch-and-price。

`SolveRequest.tuningOptions` 只接受三类后端开关：`threads`（1..=64）、`seed`（i32）与 `mipGap`（0.0 ≤ gap < 1.0，0 表示精确最优）；仅直接 `scip`／`gurobi` 入口接受，领域与框架策略各自持有预算和配置，携带调优即被拒绝。非零 `mipGap` 允许求解以 `GapLimit` 结束：此时报告状态为 `Feasible`、终止原因为 `GapLimit` 且不携带最优性证明；只有精确完成才可能出现已验证的最优性证明。该路径已用真 SCIP 观测：400 变量级随机多约束背包在 `mipGap: 0.99` 下以 `GAP_LIMIT` 结束（1 个节点、剩余间隙约 0.65、目标显著差于最优、无证明），同一源在 `mipGap: 0` 下完成并带证明。调优不改变模型、证书或预算。
