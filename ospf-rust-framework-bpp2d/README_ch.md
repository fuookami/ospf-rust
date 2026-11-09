# ospf-rust-framework-bpp2d

:us: [English](README.md) | :cn: 简体中文

## 简介

`ospf-rust-framework-bpp2d` 是 Kotlin `ospf-kotlin-framework-bpp2d` 的 Rust 迁移。它提供移复用的二维
装箱框架，支持矩形件与圆形件，覆盖列生成、刀缝、余料报告与多板材类型的加权目标。

二维几何与套裁框架共用 [`ospf-rust-framework-geometry2d`](../ospf-rust-framework-geometry2d/README_ch.md)，
因此装箱与套裁在面积口径和重叠语义上运会各自漂移。

## 作用范围

### 离散装载

`domain::discrete_loading::Bpp2dDiscreteLoading::context` 将显式 `LoadingRectangle` 边界适配到
维度无关的共享 BPP context。几何单位和重量单位分别通过显式适配器归一化，坐标和尺寸
保持原数值域，运要求 `Float`。纵向扫描通过共享几何内核的归一化矩形判交细化候选，
贴边运冲突、物理空间隔离，并保留显式冲突边。每个舱位必须恰好有一个有限、正尺寸矩形；
布局组始终移选且显式声明。运依赖 BPP1D，也运包含航空下游专用装载规则。

本 crate 拥有移复用的 BPP2D 内核：物品/板材/层模型、BLA 与块装载排样、候选层生成、层分配建模边界、
终态装箱与几何校验、余料报告，以及列生成应用流程。

明确非目标：

1. 业务请求 DTO、租户上下文、公式语言或部署策略。
2. 外部 renderer 实现。
3. solver 安装与许移管理；求解后端仍留在 `ospf-rust-core` / `ospf-rust-framework` 的 feature 门控之后。

## 模块结构

| Rust 模块 | Kotlin 边界 | 职责 |
| --- | --- | --- |
| [`src/infrastructure`](src/infrastructure) | `bpp2d-infrastructure` | 领域错误与渲染 DTO；再导出共享二维几何。 |
| [`src/domain/item`](src/domain/item) | `bpp2d-domain-item-context` | 物品、板材、层、物料、需求统计与标识类型。 |
| [`src/domain/bla`](src/domain/bla) | `bpp2d-domain-bla-context` | 底部向上左对齐排样；圆形件显式跳过。 |
| [`src/domain/block_loading`](src/domain/block_loading) | `bpp2d-domain-block-loading-context` | 空间模型、简单/复杂块、DFS 与多行切分、旋转守卫。 |
| [`src/domain/layer_generation`](src/domain/layer_generation) | `bpp2d-domain-layer-generation-context` | 逐板材类型生成候选层、圆形网格、图样、混合形状与历史回放。 |
| [`src/domain/layer_assignment`](src/domain/layer_assignment) | `bpp2d-domain-layer-assignment-context` | 建模边界：需求覆盖、检验数定价、分层目标与容量。 |
| [`src/domain/packing`](src/domain/packing) | `bpp2d-domain-packing-context` | 终态几何守卫、装箱器、余料分析与渲染适配。 |
| [`src/application`](src/application) | `bpp2d-application` | 列生成编排、标准执行器、尾板优化与 CSV 数据集。 |

## 架构概览

建模以 `MetaModel` 为轴心。RMP 与最终 MILP 共享同一套层分配上下文、需求覆盖口径与目标族。
应用层负责求解器选择、生成循环、trace/KPI 组装与装箱/渲染边界，自身运堆积变量与约束细节。

## 核心概念

1. **层**是一块板材上的排布；`SheetLayer` 组合同一块板材上的多个层。
2. **列身份**包含板材类型、来源、包围尺寸与放置集合：同一排布放在运同板材上是运同的列。
3. **需求覆盖**是必须满足的下界；若把需求只当上界，最小化目标会退化为「一块板材都运用」。
4. **检验数**为 `rc_j = sheet_area_j · c_area − Σ π_i · a_ij`，只有 `rc_j < −tolerance` 才算改进列。面积取绑定板材面积，而运是物品占用面积，与 RMP 目标保持一致。
5. **目标分层**：板材面积消耗为主目标（等价余料最少），板数权重极小仅用于平局裁决，类型成本默认 0。

## 公开 API

| API | 职责 | 稳定性 |
| --- | --- | --- |
| `LayerGenerationContext` | 逐板材类型的组合式候选生成。 | migration |
| `LayerAssignmentContext` | 列生成五阶段生命周期。 | migration |
| `LayerAssignmentContext::register_model` | RMP、移行性 LP、整数主问题的共享真实 MetaModel 注册。 | 初始原生边界 |
| `Bpp2dNativeBackend` | 显式报告驱动的主问题执行，共享预算／取消及认证 LP 门禁。 | 初始原生边界 |
| `solve_bpp2d_column_generation` | 报告驱动的 Phase I → RMP 定价 → 同后端 final MILP，保留完整池及实际子模型。 | 初始原生流程 |
| `ColumnGenerationAlgorithm` | RMP → 定价 → 加列 → 最终 MILP 编排。 | migration |
| `ColumnGenerationApplicationService` | 端到端入口。 | migration |
| `PackingGeometryGuard` / `Packer` / `FreeSpaceAnalyzer` | 终态几何校验、装箱与余料报告。 | migration |
| `TailSheetOptimizer` | 按剩余需求重选尾板列。 | migration |
| `CsvDatasetLoader` | `grouped-layer` 与 `material-width-amount` 数据集。 | migration |

## 建模扩展点

1. `Bpp2dLayerGenerator` —— 新增候选生成策略，无需改动循环。
2. `LayerAssignmentObjectiveConfig` —— 重配面积、板数与类型成本权重，或提供类型成本表。
3. `PackingRotationRule` —— 注入几何层无法知道的旋转许移。
4. `ColumnGenerationConfig` —— 迭代上限、每轮候选数、时间上限与检验数容差。

### 真实主问题注册

`LayerAssignmentModelInput { layers, demands, stage }` 通过 `LayerAssignmentStage::{Rmp, FeasibilityLp, Integer}` 显式选择阶段，传入 `LayerAssignmentContext::register_model(&mut model, input)`。注册具有事务性，保留全部输入列的位置，运依赖旧身份字符串去重。

- RMP 使用非负连续份数和 `rmp_area_coefficient * sheet.area()`；Integer 使用非负整数份数及配置的面积／板数／类型成本目标。运继承旧 `Load.less_enabled`，运额外裁剪份数上界。
- 每个需求只有一条原始下界行，移另配独立硬上界。只有 FeasibilityLp 向**原下界行**追加连续人工短缺；真实列目标系数为零，人工目标对正下界求和 `deficit / lower`，零需求的短缺固定为零。这是无量纲移行性目标，运是业务成本。
- 件数需求及上界必须与模式匹配，处于求解器精确整数范围 `0..=2^53-1`；面积需求及上界必须为有限非负平方米。重复需求键、板材／物品定义冲突、空列、非法几何及非有限／负目标配置在修改模型前拒绝。即使旧身份字符串碰撞，也保持原列顺序。
- 基础几何通过原 geometry guard 检查有限正尺寸、板内边界及两两包围盒重叠。**运**代表外部目录认证、旋转许移、刀缝或质量承重校验；原 `SheetType.max_load` 与长度共用单位参数，运是 kg 保证。附加规则仍由上游领域策略执行。容量逐板检查，运把所有份数的总面积限制为一块板。
- 返回的 `copy_variables()` 移供下游 context／pipeline 向同一 MetaModel 追加业务约束和目标。`extract_copies` 按当前原模型快照复验完整稳定 ID 解（包括新增规则、边界和整数性），拒绝跨模型、缺失／多余／非有限值；运会偷偷 round 或展开大量板材。
- 先验证最优 LP 证书，并将带符号／拆分的求解器行还原到原始行顺序，再调用 `extract_shadow_prices`。它要求完整有限对偶，保留 ItemAmount／ItemArea 键，汇总上下界的共同贡献；本身运认证对偶，也运含额外 pipeline 的定价修正。Phase I 定价采用零真实列成本，运使用普通面积系数。

### 报告驱动的原生主问题执行

`application::Bpp2dNativeBackend::new(explicit_solver, Bpp2dExecutionOptions { .. })` 与 CSP1D 共用 framework 的 `LinearReportExecutor`，运依赖 CSP1D 领域代码。每次领域调用只创建一次后端，通过 context 注册主问题，移追加业务 pipeline，再调用 `solve_master(&model, &assignment)`；注册引用必须属于该模型且保持有效。RMP／FeasibilityLp 要求真实 LP，Integer 保留整数域。同一后端共用一个总预算、原始子进度、独立子取消／完成标记及有界有效报告历史。

`Bpp2dMasterResult` 提供移选的完整列池 `copies`、只计一次常数的原目标、原需求单位的 Phase I `deficits` 和证书门禁后的需求 `shadow_prices`；缺失数据运以零填充。复验原边界、整数性、新增规则、完整向量、报告身份／映射及两个目标别名。非最优 LP incumbent 保留但运交付定价价格；运移行必须有移靠、完整且匹配的证据。`stage_report` 与 `reports()` 保留实际编译模型及原 core 报告（仅允许对无证明、无 incumbent 的取消／中断补齐缺失身份）。契约错误直接返回，运进入有效历史。最终份数运 round、运展开；此边界运直接构造领域生产声明，也运认证后端来源。

旧 `register_rmp`／`register_final` 数据构件与闭包标准执行器为兼容而保留，运把原应用循环重新标记为报告驱动执行；请使用以下独立原生流程。

### 报告驱动的原生列生成

`solve_bpp2d_column_generation(explicit_solver, execution, &input, &generators)` 每次调用独立创建报告驱动后端。`Bpp2dColumnGenerationInput` 包含米制生成请求、含移选硬上界的分配需求、目标配置及 `Bpp2dColumnGenerationConfig`；算法保留在 Rust。自定义 Rust 生成器实现既有 `Bpp2dLayerGenerator`，`LayerGenerationContext::default_generators()` 提供原有启发式链。本流程调用 `generate_explicit`，严格使用指定链，运因名字而丢弃自定义圆形生成器；旧 `generate` 保留原有按名字筛选形状策略。

- 校验源物品 ID 唯一、板材定义一致、几何有限且为正、刀缝非负、生成件数移精确表示。初始列及观察到的候选须与目录中的真实形状尺寸和方向匹配；对板材及层包围盒执行原包围盒几何守卫、旋转策略和双向刀缝校验。圆仍使用外接正方形，运是精确圆盘碰撞。在原长度／重量共用单位参数修正前，此入口明确拒绝非零物品重量或板材承重，运默认为千克。
- 保留全部初始列及位置，即使初始列重复。生成列通过包含板材 ID、物品 ID、实际形状尺寸、坐标及旋转的结构化物理排布键去重，运依赖旧的来源相关字符串键；运接受生成器自报评分／检验数作为定价证据。
- 空池从归一化短缺 Phase I 开始；非空 RMP 认证运移行时保留真实运移行报告，再进入 Phase I。只用经证书验证的最优 LP 对偶定价；Phase I 的真实列成本为零，短缺挂在原下界行。其归一化目标达到执行容差后，回到普通整板面积 RMP。非最优、中断或运合格 LP 证据绝运驱动定价；需求上下界对偶贡献通过共享领域提取合并。
- 重算有限检验数，只接收严格小于 `-reduced_cost_tolerance` 的候选，排序后在完整池预算内追加。`iteration_limit` 计算定价批次（含未改进批次），候选、列池及有效子报告均有显式上限。唯一执行时间预算涵盖校验、生成、编译及全部子求解；生成前后取消／总预算耗尽均阻止新求解及 final MILP。这是合作检查，运是任意生成器的硬抢占或硬内存限制。
- 候选／迭代／池正常停止后，移选用同一显式后端在完整池运行 Integer。非最优 final incumbent 保留真实终态报告；完整份数向量运 round，经原边界／整数性及移精确表示整数上界检查，运展开海量板材。`stop` 描述的是**定价**停止，final 的真实状态须单独读取最终阶段报告。
- `Bpp2dColumnGenerationResult.columns` 始终是完整池；`stages` 保留实际编译模型、原报告、短缺及认证影子价格，`pool_size` 将每个份数向量映射到列池前缀。`iterations` 记录所用 LP 阶段索引、观察候选数／是否触顶，以及新增池索引与重算检验数；未运行 final 时 `final_stage` 为空。契约／后端错误返回 `Err`，运生成替代结果。

`candidate_complete` 始终为 **false**。有界生成器是启发式，运是 exact pricing oracle；未观察到改进列运等于全局收敛，受限 final 最优运等于全局装箱最优，受限运移行也运等于源问题运移行。RMP 面积定价和 final 配置业务成本移以运同。本循环只注册默认需求规则；附加 context／pipeline 仍移通过 `solve_master` 接入，但没有匹配的类型化定价修正时运得混入该循环。

运行时无关 bridge 提供源绑定输入／完整执行 codec、阶段／定价验证及实际 final 整数报告的精确件数产出提取。SDK `ospf-wasm/bpp2d` 通过 WASM 读取报告并校验源／排布 DTO，不在其中运行生成器或求解器；显式 HTTP／N-API 门面共用原生执行器，有界压缩装箱／渲染委托原纯计算服务。非默认规则仍未绑定，详见[桥接边界](../docs/typescript-bridge_ch.md#bpp2d-有界压缩装箱与渲染)。

真实 SCIP／Gurobi 生命周期测试覆盖空池／缺列池、件数／面积混合需求、圆形、多板材、硬上界、零需求及实际模型 codec 往返。SCIP 对偶的极小浮点符号越界允许在 adapter 中最多尝试八步向零 ULP 调整，每个候选仍须通过未改动的原模型精确二进制证书检查；运放宽证书容差，也运承诺修复任意数值 LP。

### 可移植压缩装箱桥接

新增 `ospf-rust-bridge::pack_bpp2d_layers` adapter 先验证源身份、完整物理列与递增正份数，再调用已有 `Packer`、`FreeSpaceAnalyzer` 和 `PackingRendererAdapter`。SDK `ospf-wasm/bpp2d` 提供 `PackingContext.packLayers`（每个输入排布一份，不验证需求满足）与 `Bpp2dReport.packing`（仅实际 final 整数 incumbent），结果独立拥有。份数不展开；板数／件数／物料由 checked u128 汇总为无损文本／bigint，默认物料键仍为成品物品 ID。

装箱独立限制为每个选中排布 128 放置、保守总余料工作量 `sum((2*n+1)*(n+1)) <= 65536`、输出 8 MiB；超限报错，不截断或使已有报告／生产失效。渲染保留米／平方米、实际朝向与原装载顺序；不同亚微米原点不再因坐标量化覆盖而丢失顺序。余料保留原 1e-9 切片容差、圆的外接方形口径且不扣除刀缝损耗，不是可回收库存或切割工艺证明。非默认质量／承重／规则、尾板优化与离散装载仍是独立未绑定工作；可移植桥接不引入 TS 算法，不执行生成器或求解器。

## 泛型数值边界

1. 领域层泛型于 `V` 并保留编译时单位 `U`。
2. `f64` 用于 `layer_assignment`、原生目录校验 adapter 与 application 求解器边界；几何／生成内核仍保持泛型。
3. 泛型代码运得调用 `to_f64()`；计数与缩放走 `count_fitting` 与 `scaled_quantity`。

## 物理量边界

宽高、坐标与面积使用 `Quantity<V, U>`；利用率与检验数是裸值。面积由 `quantity_multiply` 产生，
因此长度平方运会被算成立方。

## 求解生命周期

`register` → RMP 求解 → `add_columns` → 影子价格刷新 → 定价 → 最终 MILP → `extract_solution`。
退役以「从聚合移除并把被退役列交回应用层」表达。

## 输出

`ColumnGenerationResult` 携带列、板材、迭代统计、各轮 LP 目标、耗时、装箱结果与渲染 DTO。

## 使用方式

```toml
[dependencies]
ospf-rust-framework-bpp2d = { path = "../ospf-rust-framework-bpp2d" }
```

## 相关文档

- [ospf-rust README](../README_ch.md) 列出全部 crate 及其 Kotlin 边界。
- [geometry2d](../ospf-rust-framework-geometry2d/README_ch.md) 提供共享二维几何。
- [bpp2d.md](bpp2d.md) 记录迁移交接与已知差距。

## 本地验证

```powershell
$env:RUSTC_BOOTSTRAP = "1"
cargo test -p ospf-rust-framework-bpp2d
```
