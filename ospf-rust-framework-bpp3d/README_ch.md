# OSPF Rust Framework BPP3D

:us: [English](README.md) | :cn: 简体中文

## 简介

`ospf-rust-framework-bpp3d` 是 Kotlin `ospf-kotlin-framework-bpp3d` 的 Rust 迁移目标。它提供可复用三维装箱领域能力，包括 BPP3D infrastructure、item 建模、BLA/block-loading/layer-generation context、layer-assignment pipeline、packing、CSV 协议适配、renderer DTO，以及 application 层 column-generation 编排。

## 作用范围

本 crate 拥有可复用 BPP3D framework 内核，把装箱几何、候选生成、层分配、最终装箱、fixture 协议和 renderer DTO 边界保留在共享领域 crate 内。

明确非目标：

1. 业务专用请求 DTO、租户运行时上下文、公式语言或项目部署策略。
2. 外部 renderer 源码。
3. solver 安装、许可证管理或非 BPP3D 后端插件所有权。

## 模块结构

| Rust 模块或目录 | Kotlin 边界 | 职责 |
| --- | --- | --- |
| [`src/infrastructure`](src/infrastructure/README_ch.md) | `bpp3d-infrastructure` | 几何原语、orientation、packing shape、PWL approximation helper 和 renderer DTO。 |
| [`src/domain/item`](src/domain/item/README_ch.md) | `bpp3d-domain-item-context` | item、package、material、bin、layer、pattern、demand coverage、shape metadata 和 continuous-radius model component。 |
| [`src/domain/bla`](src/domain/bla/README_ch.md) | `bpp3d-domain-bla-context` | bottom-up-left-justified placement 与投影候选构造。 |
| [`src/domain/block_loading`](src/domain/block_loading/README_ch.md) | `bpp3d-domain-block-loading-context` | simple/complex block generation、DFS 空间分裂、MLHS search 和 block-loading diagnostics。 |
| [`src/domain/layer_generation`](src/domain/layer_generation/README_ch.md) | `bpp3d-domain-layer-generation-context` | Block、BL、circle-packing、Pattern、Pile 和 Historical 层候选生成。 |
| [`src/domain/layer_assignment`](src/domain/layer_assignment/README_ch.md) | `bpp3d-domain-layer-assignment-context` | RMP/final MILP 变量、动态列、约束、目标、需求/深度/启用限制和 shadow price。 |
| [`src/domain/packing`](src/domain/packing/README_ch.md) | `bpp3d-domain-packing-context` | 最终 packed-bin 转换、geometry guard、placement/block trace replay、material summary 和 render adaptation。 |
| [`src/application`](src/application/README_ch.md) | `bpp3d-application` 与 layer-selection flow | solver backend 选择、CSV 物化、column-generation 生命周期、fixture report、KPI/trace diagnostics 和 renderer output。 |

## 架构概览

建模路径以 `MetaModel` 为轴心：RMP 和 final MILP 共享 `LayerAssignmentContext`、`LayerAssignmentAggregation`、变量组件、需求/深度/启用约束和目标 pipeline。application service 负责 solver backend、CSV 物化、trace/KPI 诊断和 renderer 适配；placement/block trace replay 继续保留在 packing domain。

Kotlin `bpp3d-domain-layer-selection-context` 在 Rust 中映射到 application 编排，因为 Rust 把 layer selection 作为围绕可复用 domain context 的 solver 生命周期处理。


## 圆柱体几何语义

框架支持轴对齐圆柱体，具有以下语义：

### 垂直圆柱体（Axis3.Y）

- 完整支持圆形填充网格候选和保守长方体支撑堆候选。
- 底部重叠/支撑检查使用真实足迹几何。
- 渲染器 DTO 中的 `radius` 和 `diameter` 字段。

### 水平圆柱体（Axis3.X / Axis3.Z）

- **固定/离散半径**：轴感知放置的圆形填充网格路径。
- **支撑堆**：单全长支撑、重复同形多支撑区间、异构支撑区间。
- **悬挂候选**：覆盖圆柱轴的单或重复窄长方体支撑线悬挂。
- **支撑要求**：必须在容器底部或具有覆盖完整圆柱轴和底部支撑线的长方体支撑区间。
- **层约束**：单个 `BinLayer` 不能混合多个圆柱轴。

### 连续半径建模

- **PWL 近似**：`UnivariateLinearPiecewiseFunction` 符号用于通过 `π·r²·h` 的体积近似。
- **保守包络**：`rMax` 包络边界用于生产就绪的连续半径变量。
- **生产就绪路径**：`RealVar` 求解器变量，具有约束边界和目标等式约束。
- **渲染器输出**：求解器选择的半径应用于 `actualVolume` 和 `radius`/`diameter` DTO 字段。
- 单轮 CSV 流程向两次 RMP 状态都传递连续半径组件供扩展使用，但不会自动将其注册到 RMP 模型。
- **PWL 诊断**：渲染器项目信息中的 `pwl_volume`、`pwl_error`、`pwl_segments`、`pwl_within_envelope`。
## 核心概念

1. `BinLayer` 是 layer assignment 中的被选择列。
2. layer generation 由 BLA、block loading、circle packing、Pattern、Pile 和 Historical 来源提供候选层。
3. layer assignment 通过 `MetaModel` 注册 RMP/final MILP 变量和约束/目标 pipeline。
4. packing 把选中的层转换为 packed bin，并在 renderer output 前校验 shape-aware geometry。
5. CSV fixture suite 提供 Kotlin 对比基线和 solver backend diagnostics。

## Public API

| API | 职责 | 稳定性 |
| --- | --- | --- |
| `application::ColumnGenerationApplicationService` | application-facing column-generation 编排。 | migration |
| `application::MetaModelSolverBackend` | RMP/final executor 的可插拔 solver backend 边界。 | migration |
| `application::ColumnGenerationStandardExecutors` | 标准 RMP/final executor 接线。 | migration |
| `application::TraceValidatedMetaModelFinalExecutor` | 显式启用的报告驱动 final 与严格物理回放。 | 实验性，尚待验收 |
| `application::Bpp3dOneRoundEvidence` | 显式单轮流程的实际阶段模型、报告、列池和最终分配。 | 实验性，仅 Rust |
| `application::CsvDatasetLoader` | 启用 `serde` 时的 CSV dataset 加载。 | migration |
| `application::Bpp3dRunReport` | 结构化运行与对比报告。 | migration |
| `domain::layer_assignment::*` | layer-assignment context、aggregation、limit、objective 和动态列。 | migration |
| `domain::layer_generation::*` | 层候选生成 trait、request、diagnostics 和策略实现。 | migration |
| `domain::packing::*` | packing 转换、geometry guard、packed-bin solution 和 render adaptation。 | migration |
| `domain::packing::PackingGeometryContract` | 可注入的最终装箱几何校验。 | migration |
| `infrastructure::*` | 共享 geometry、orientation、shape、PWL 和 renderer DTO 类型。 | stable within migration |

## 建模扩展点

solver 行为通过 `MetaModelSolverBackend` 或 RMP/final executor trait 接入。新的 layer candidate source 应实现 layer-generation trait。业务专用的最终几何校验通过 `PackingGeometryContract` 和 `ColumnGenerationApplicationService::with_geometry_guard` 注入。请求级业务规则应通过 package-rule policy 等 domain policy 注入，不要把任意闭包塞进可序列化模型。

新增约束、目标和结果提取应放在 domain context、aggregation、model component 或 pipeline 中。application 代码负责组合这些扩展点，而不是复制 domain 建模逻辑。

列生成生命周期扩展：

1. `ColumnGenerationRmpExecutor::execute_result` 和 `ColumnGenerationFinalExecutor::execute_result` 将注册/求解失败以 `Result` 传播，并标注 `RestrictedMasterProblem` 或 `FinalMilp` 阶段。
2. `ColumnGenerationRmpModelExtension`、`ColumnGenerationFinalModelExtension` 可在标准 RMP/final context 上追加模型内容；RMP 扩展可返回类型化 `additional_shadow_prices`。
3. `ColumnGenerationApplicationService::create_algorithm_with` 支持替换算法装配；算法还提供可失败的初始列和候选过滤入口。

## 泛型数值边界

面向 domain 的 API 在表达可复用几何、物理量和需求语义时保留泛型数值。solver-facing RMP/final 执行当前在注册和 adapter 边界使用 `MetaModel<f64>`。裸 `f64` 转换应保留在 solver、CSV、reporting 或 renderer 边界。

## 物理量边界

宽、高、深、重量、体积、半径、物料尺寸、包件尺寸、需求覆盖和容量类数量应通过 `Quantity<V, U>` 或 infrastructure geometry wrapper 表达。renderer DTO 和 CSV 协议可以暴露标量字段，但标量转换属于序列化边界。

## 求解生命周期

当前 application flow：

1. 加载或构造 BPP3D request 和候选来源集合。
2. 生成 BLA/block/circle/Pattern/Pile/Historical 层候选。
3. 注册 layer-assignment RMP 变量、约束、目标和 shadow-price metadata。
4. 求解 RMP，提取稳定 demand shadow price，生成或刷新列，并注册新的 layer 变量。
5. 在共享 layer-assignment context 上求解 final MILP。
6. 提取 selected layer，并通过 packing replay placement/block trace。
7. 校验最终几何并输出 solution、KPI、diagnostics 和 renderer DTO。

## 输出

`ColumnGenerationResult` 记录 selected layer、packing analysis、solver status 和生命周期 diagnostics。`Bpp3dRunReport` 与 fixture-suite report 记录 dataset comparison、solver availability、failure、demand coverage、packed-bin output、selected-layer diagnostics 和 feature-matrix 信息。renderer DTO 包含长方体和圆柱的 shape metadata 与 `actualVolume` 输出。

## 使用方式

```rust,ignore
use ospf_rust_framework_bpp3d::application::{
    ColumnGenerationApplicationService, ColumnGenerationConfig,
};

let service = ColumnGenerationApplicationService::new(ColumnGenerationConfig::default());
let result = service.solve(request)?;
```

CSV 与 fixture-suite 入口需要启用 `serde` feature。

## 本地验证

```powershell
cargo check -p ospf-rust-framework-bpp3d
cargo test -p ospf-rust-framework-bpp3d --lib
cargo check -p ospf-rust-framework-bpp3d --features serde
cargo test -p ospf-rust-framework-bpp3d --features serde --lib
```

真实 solver manifest suite 由 feature gate 控制，并需要本地 solver 可用。

## 当前边界

当前迁移覆盖：

1. solver-agnostic `MetaModel` RMP/final executor、可插拔 backend wrapper，以及一轮 RMP -> shadow-price generation -> column refresh -> final flow。
2. 层候选 demand coverage metadata、稳定 demand shadow-price key、selected-layer 提取，以及基于 placement/block trace 的 packed-bin/render replay。
3. BLA local/global 层候选、simple-block 层候选、多轮 axis `ComplexBlockGenerator`、有界 DFS 空间分裂、MLHS branch/depth 候选排序、固定/离散半径圆柱 circle-packing 网格候选，以及 request-aware Pattern/Pile/Historical 生成器。
4. 轴感知长方体和圆柱几何、带 guard 的横向圆柱 final validation、PWL continuous-radius metadata/fixture，以及 renderer `actualVolume` 输出。
5. CSV schema guard、Kotlin Gurobi grouped-layer/material-width-amount 适配、manifest 和递归目录 fixture 加载、backend survey/no-run/fake fallback 报告，以及 feature-matrix diagnostics。
6. PatternedItem 保守 demand coverage、PackageAttribute 校验/装箱 diagnostics，以及 RestAmount/TailBinLoadingRate/BinLoadingOrder 的 `MetaModel` 注册入口。

manifest 已镜像 Kotlin Gurobi CSV 样例全集，共 22 个 fixture，包括 19 个 Kotlin 派生 dataset 样例和 3 个本地回归 fixture。剩余差距以本节的当前边界清单为准；更新迁移覆盖时应同步对照 Kotlin BPP3D README。

## 多箱型与加权目标

BPP3D 现在支持「多箱型混合供给」与「按箱型计价的加权目标」：

1. `LayerGenerationRequest.bins` 让每种箱型分别产出候选列，列通过 `BinLayer.bin` 携带箱型；
   未指定任何箱型时保持单次直通，使不依赖箱型的历史层生成器仍可工作。
2. `BinLayer::identity_key` 把箱型纳入列身份：同一排布放在不同箱型上是不同的列，
   否则多箱型会被去重逻辑直接抹掉。
3. 主目标改为**箱型体积消耗最小**（`BinVolumeMinimization`）。因为需求体积固定，
   `总体积浪费 = Σ 箱型体积 − 需求体积 = Σ 箱型体积 − 常数`，所以最小化体积消耗就是最小化浪费。
   不能对「箱型体积 − 已装层体积」求和：铺满的整箱会记为 0 余料，反而让「一个铺满的大箱」
   优于「多个小箱」，与真实装载相反。
4. 次目标 `BinAmountMinimization` 默认系数降为 `1e-6`，仅在体积消耗相同时打破平局；
   `BinTypeCostMinimization` 按箱型计价，权重默认 0，需调用方显式配置。
5. `bin_waste_volume` / `bin_waste_ratio` / `per_type_bin_cost` 只用于 KPI 与报告，不作为目标。

默认值与原实现的差异：`final_bin_amount_coefficient` 从硬编码 `1.0` 改为 `1e-6`，
体积权重默认 `1.0`。这是一次**行为变更**，已有调用方若依赖「最小化箱数」需要在配置中显式恢复。

## 求解后端

`MetaModelSolverBackend`、`NoopMetaModelSolverBackend`、`ColumnGenerationSolverMetaModelBackend`
与 `MetaModelExecutionDiagnostics` 已上移到 `ospf-rust-framework`，使 BPP2D/BPP3D/CSP1D/CSP2D
共享同一求解边界。本 crate 保留兼容 re-export，并通过 `Bpp3dExecutionDiagnosticsExt` 提供
`layer_count` / `bin_count` 只读别名与 `write_bpp3d_info`，既有 info key 保持不变。

### 真实报告后端基础

本 crate 同样重导出 `ReportMetaModelBackend` 和 `MetaModelStage`。使用显式 `Arc<dyn LinearSolver>` 与 `LinearExecutionOptions` 创建一个后端，将 clone 分别传入 `SolverBackedMetaModelRmpExecutor` 和 `SolverBackedMetaModelFinalExecutor`。原 `run_csv_materialized_one_generation_round` 仍是 RMP → 原层生成器 → 刷新 RMP → final MILP 的一轮流程，不是完整自动列生成循环或完整定价证明。

共享后端通过 `backend.reports()` 保留每个有效子阶段的实际编译模型、指纹／映射、原始 `SolveReport` 和独立目标常数。只有经认证的最优 LP 才提供原行定价对偶；final 保留整数域和实际 incumbent／终态语义。无 incumbent 的超时／取消及未经认证的 LP 会阻止旧结果提取，不使用配置中的 no-op 向量、目标或影子价格。类型化 `backend.execute` 入口仍保留有效无解报告，不伪造向量。clone 共用总合作预算、原始进度、独立子取消和有界历史；每次 application 调用创建新后端。

契约测试覆盖 no-op 回填拒绝和预取消。feature-gated 真实 SCIP／Gurobi 10 测试运行原 CSV 单轮流程，保留两次 LP 证书及 final 整数报告，需要相应运行时／许可证。同时运行两者应显式启用 `serde,scip,scip-bundled,gurobi10`，只开启 bundled 依赖 feature 不会启用本 crate 的 `scip` 测试门禁。

这是 **已绑定 BPP3D SDK／HTTP／N-API 入口背后的 Rust 执行基础**，不是通用 BPP3D 求解器。SDK 提供可移植源校验、显式源层方案回放及有界单轮执行报告 reader；原生执行只经 native 引擎及其显式 `bpp3d` 策略运行。旧 final 提取仍把跨箱分配折叠为已选层集合；缺少物理 trace 时，packing replay 仍可能根据覆盖量构造回填位置。可渲染、受限 final 最优或单轮测试都不构成源 BPP3D 全局可行／最优证书。

### 实验性严格 final 回放

`TraceValidatedMetaModelFinalExecutor` 是仍在验证中的 Rust 显式启用路径，不改变旧执行器。即使校验失败，`execute_with_selection` 仍保留原始箱／层分配及后端报告。分组需求可使用 `execute_with_selection_and_item_amounts`，按物品目录顺序传入数量；物理回放失败时同样保留已提取的分配。严格回放会拒绝缺失 trace、缺少明确坐标的多物品放置摘要和未完整指定的块网格，不推测位置；同时检查几何、物品 ID、逐层物品覆盖、每个物品的精确需求及箱载重上限。`FinalPackingValidation` 可增加几何门禁，但不能放宽基础检查。物理失败会通过应用层 `execute_result` 以 `FinalMilp` 传播。

`replay_strict_grouped_packing` 对给定 application state、分配、分组数量和展开上限复用上述物理与精确需求门禁，不调用求解器。调用方仍须负责 state 与源及模型／求解器来源的绑定；通过物理回放不代表最优。

显式入口 `run_materialized_with_traced_initial_layers` 和 `run_materialized_one_generation_round_with_traced_initial_layers` 接收既有 `LayerGenerationResult`，将初始层的物理 trace 保留到 final 提取。`run_csv_materialized_one_generation_round_with_traced_initial_layers` 按请求初始层顺序接收每层的 `LayerTracePair`，物品索引对应 `request.items`。它从实际 trace 推导空的物品覆盖，保留显式覆盖、包装属性和连续半径配置，并将每个唯一 CSV 物品 ID 的正分组数量传入两次 RMP、生成及 final 复验。总需求上限为 100,000 件；严格回放默认上限为 10,000 件，可配置到 100,000 件。真实求解使用带真实 backend 的 `SolverBackedMetaModelRmpExecutor` 及 `TraceValidatedMetaModelFinalExecutor`；`MetaModelRmpExecutor` 使用空操作 backend。旧执行器／自定义执行器默认拒绝分组数量，除非显式实现 `execute_result_with_item_amounts`。trace 由调用方单独提供，不从 CSV 文件读取；未选中的初始层可无 trace，但选中后严格回放会失败。

`run_csv_materialized_one_generation_round_with_evidence` 使用显式注入的求解器与 `Bpp3dOneRoundOptions` 创建独立报告后端，执行同一单轮流程，RMP 和 final 使用默认模型配置。它保留每次 RMP 的实际输入状态和已接受的模型／报告、final 完整列池及原始块／放置 trace、实际箱层分配。后续阶段失败时仍保留前序证据；物理检查失败时也保留 final 报告与已提取分配。后端身份或可行性门禁拒绝的报告不会作为证据接收。该结果仅在 Rust 内存中使用，尚不是与源绑定的序列化报告，不支持模型扩展，也不证明全局最优；预算和取消采用合作式机制，不强制中断生成或物理回放。

feature-gated 测试已用真实 SCIP 和 Gurobi 10 验证原严格 final 与包含两次认证 RMP 的单轮流程。两种后端均已验证分组 trace 的单层两件及同层跨两个箱复用；更全面的原生验收仍未完成。内建分组 application 路径尚未挂接 RMP/final 模型扩展；自定义数量执行器须自行处理扩展。旧 materialized 和 CSV 入口仍接收不带 trace 的初始层。**旧 CSV 应用流程目前未将 `item_amounts` 用于 RMP/final 需求**，不能作为分组数量已满足的证据。初始列和生成列按归一化后的层身份关联到第一个实际接纳的候选，不会借用无关初始层或后续重复候选的 trace。该门禁不证明任意包装规则或源问题全局最优。BPP3D 源和方案校验、经 HTTP／N-API 的有界单轮原生执行及其版本化执行报告 reader 均已通过 TypeScript SDK 开放；更广规则、可配置生成器链与圆柱支持仍未绑定。

证据入口还分别用两个真实后端验证了必须选中 `BlockLayerGenerator` 实际生成的两件层的分组实例，并保留其网格 trace。测试同时检查受限 final 的不可行报告，以及物理失败、子求解之间取消和报告数量上限时已有阶段的保留；这不等于跨环境报告验收。

## 相关模块

- [根 README](../README_ch.md)
- [BPP3D domain README](src/domain/README_ch.md)
- [BPP3D application README](src/application/README_ch.md)
- [Kotlin BPP3D README](../../ospf-kotlin/ospf-kotlin-framework-bpp3d/README_ch.md)
