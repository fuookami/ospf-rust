# ospf-rust-framework-csp2d

:us: [English](README.md) | :cn: 简体中文

## 简介

`ospf-rust-framework-csp2d` 是 Kotlin `ospf-kotlin-framework-csp2d` 的 Rust 迁移。它沿用
[`ospf-rust-framework-csp1d`](../csp1d/README_ch.md) 的 CSP 语义（产品、需求、图案、
方案、良率、余料、板长分配、列生成），同时采用
[`ospf-rust-framework-bpp2d`](../bpp2d/README_ch.md) 的泛型分层纪律：领域层泛型于 `V`，
求解器数值类型只出现在建模边界与应用层。

二维几何与 BPP2D 共用 [`ospf-rust-framework-geometry2d`](../geometry2d/README_ch.md)。

内置主问题启发式与 service 回退在每次求解内复用覆盖表，仅当方案参与未满足需求的评分时才惰性计算该方案的余料分母。缓存归属领域层，借用 assignment，求解结束即丢弃，不跨公开方案修改复用。候选顺序、同分取舍及使用上界不变，不引入后台线程或全局方案快照。

## 作用范围

两种供给模式，一套模型：

| 模式 | 问题类别 | 物料模型 | 决策变量 |
| --- | --- | --- | --- |
| 卷材套裁 | 条带排样（幅宽固定，长度待定） | `width` 固定、`length = None` | 消耗长度与排样 |
| 定尺板材 | 二维装箱 | `width × height` 固定 | 板材张数与排样 |

裁切工艺可配置，且与模式正交：

| 工艺 | 含义 | 设备 |
| --- | --- | --- |
| `Freeform` | 任意排样，只需放置坐标 | 激光、水刀、CNC |
| `GuillotineTwoStage` | 先切条再切件，输出切分树 | 剪板机、开料锯 |
| `GuillotineThreeStage` | 切条 → 切块 → 切件，输出切分树 | 剪板机、开料锯 |

异形件（非矩形件）明确不在范围内。

## 模块结构

| Rust 模块 | Kotlin 边界 | 职责 |
| --- | --- | --- |
| [`src/infrastructure`](src/infrastructure) | `csp2d-infrastructure` | 裁切区域、刀缝、断头台刀线与切分树、裁切工艺与错误。 |
| [`src/domain/material`](src/domain/material) | `csp2d-domain-material-context` | 双模式物料、产品与需求、裁切图案与裁切方案。 |
| [`src/domain/pattern_generation`](src/domain/pattern_generation) | `csp2d-domain-pattern-generation-context` | 图案生成请求与货架式生成器。 |
| [`src/domain/produce`](src/domain/produce) | `csp2d-domain-produce-context` | 主问题：图案使用量、需求覆盖、物料可用量、目标项。 |
| [`src/domain/yield`](src/domain/yield) | `csp2d-domain-yield-context` | 欠产与超产分析、超产上界。 |
| [`src/domain/wasting_minimization`](src/domain/wasting_minimization) | `csp2d-domain-wasting-minimization-context` | 余料面积与余料率分析、余料惩罚项。 |
| [`src/application`](src/application) | `csp2d-application` | 问题定义、MILP 路径与列生成路径。 |

## 切分树语义

断头台方案不是一组坐标，而是一棵**切分树**：内部结点是贯穿刀，叶子是成品或余料。根刀线为第 1 级；
同向子刀线保持父级，换向才加一级。因此两级工艺允许一组平行条带刀，再转向切件；工艺级数不等于树深度。

`CutTree2::validate` 会拒绝：区域切分不一致（含刀缝带）、超出工艺级数上限、子刀线级数与父刀方向不一致。

货架式生成器构造逐行／逐件切分链。通过旧树校验不代表成品叶子已与放置一一对应；下述显式原生门禁进一步
复验物理对应、实际切分及资源限制，不把仅满足工艺级数的树当作完整裁切证明。

## 列生成与定价

列追加由「剩余需求生成候选 + 检验数筛选」两步驱动：

1. 按剩余需求生成候选图案。
2. 用检验数 `rc = 消耗量 × 单价 − Σ 覆盖系数 × π` 筛选，只把 `rc < -tolerance` 的候选按升序加入。

旧 `Csp2dSolvers` service 的影子价格 `π` 由**覆盖缺口比例估计**，不是 LP 对偶。因此这是 reduced-cost pricing
的工程近似，不是严格意义上的定价证书。下述独立显式原生主问题 API 不替换该 service 或其回退行为。

## 显式原生报告主问题

`ProduceContext::register_model` 为 `ProduceStage::Rmp`、`FeasibilityLp`、`Integer` 注册真实
`MetaModel<f64>` 变量、目标和约束。`Csp2dNativeBackend::solve_master` 通过显式注入的 `LinearSolver`
执行该模型，共享 framework 报告执行器的时间、取消、进度与报告数量预算。后端失败不替换为旧启发式。
该 API 本身只执行主问题；下述独立 Rust 原生流程组合注册与生成。SDK 通过显式 HTTP／N-API 门面调用原生流程，不提供浏览器求解器。

- 三阶段共用需求下界、可选硬上界和物料硬库存行，同产品的件数／面积需求保持独立。Phase I 仅给下界添加
  原单位短缺，目标为 `sum(短缺 / 正需求下界)`，真实列成本为零；不放松上界或库存。零短缺不决定真实列的
  唯一用量，也不代表最小用量解。
- LP 用量连续、整数阶段用量为无符号整数。不施加旧 assignment 启发式 `usage_upper_bounds` 的值，由实际库存
  行限制使用次数。每次列使用消耗**一个物料批次**，卷材段也一样；这不是多段共用卷长的优化。
- RMP／整数默认目标为原方案的料域消耗面积；可显式叠加原余料面积、卷材段高与单次使用成本权重。定尺图案的
  region 可以小于整板，不能将消耗或余料静默提升为整板面积。这不改变旧 service 默认的余料评分策略。
- 注册具有事务性，保留重复物理列和池顺序。求解器边界为 `f64`／`Meter`；件数、库存及件数上界输入限
  `0..=2^53-1`，面积必须有限且非负，不给原生求解器增加任意精度保证。
- 物理校验复用原矩形／刀缝谓词，检查输入池内物料／产品定义一致、有限且不塌缩的几何、包含与不重叠。
  物料不能同时指定板高与卷长，已声明物料面积必须有限。此边界显式拒绝重量、设备、最小余料阈值策略和副产物放置。
- 切分树先限制深度 128、节点 4096，再进行递归验证；复验实际切分、缓存切分、根区域及成品叶子与实际放置的
  一一对应，随后保留原工艺／级数校验。旧生成器未标注匹配成品叶子的图案可能被拒绝。浮点切分匹配只容许
  量级相关的末位舍入，不把旧 `1e-9` 绝对容差当作微小件的物理证明。由于未传入外部产品目录，源产品尺寸和
  旋转权限仍需调用方认证。
- 报告保留实际编译模型、身份／指纹、原始报告及仅计一次的目标常数。提取前复验完整原快照（含附加 pipeline），
  不对份数 round 或截短。非最优 incumbent 不丢失，但只有证书验证通过的最优 LP 才交付原行价格。
  受限主问题的结论不提升为源套裁问题全局证明。
- `Csp2dMasterShadowPrices` 合并每项需求上下界贡献，独立保留库存价格。`ProduceContext::reduced_cost`
  使用原件数／面积覆盖、阶段成本及库存贡献；它不是证书或候选来源认证器。附加 pipeline 必须提供对应定价修正，
  忽略这些行不能证明不存在改进列。

附加业务 pipeline 在注册后、执行前继续加到同一模型：

```rust
use std::sync::Arc;
use ospf_rust_core::model::MetaModel;
use ospf_rust_core::solver::LinearSolver;
use ospf_rust_framework_csp2d::application::{Csp2dMasterResult, Csp2dNativeBackend};
use ospf_rust_framework_csp2d::domain::produce::{
    Csp2dMasterProblem, ProduceContext, ProduceModelInput, ProduceStage,
};
use ospf_rust_quantities::quantity::Quantity;
use ospf_rust_quantities::unit::derived::Meter;

fn solve_integer_master(
    master: &Csp2dMasterProblem,
    solver: Arc<dyn LinearSolver>,
    kerf: &Quantity<f64, Meter>,
) -> ospf_rust_core::Result<Csp2dMasterResult> {
    let mut model = MetaModel::new("csp2d-master");
    let registration = ProduceContext::default().register_model(&mut model, ProduceModelInput {
        master, stage: ProduceStage::Integer, kerf,
    })?;
    if let Some(&usage) = registration.usage_variables().first() {
        model.add_le_constraint(&[(usage, 1.0)], 2.0, "business-use-cap")?;
    }
    Csp2dNativeBackend::new(solver, Default::default())?.solve_master(&model, &registration)
}
```

真实 SCIP／Gurobi 测试覆盖三阶段、空 Phase I、绑定库存对偶、需求硬上界、不可行证书、附加规则／常数及
原始报告指纹。fake 测试覆盖错误证书／身份、不完整赋值、预算与取消，不把模拟报告当成真实求解。

## 报告驱动的原生列生成

`solve_csp2d_column_generation(solver, execution, input, generators)` 通过上述共享注册执行实际 Phase I、认证 LP
定价及 final 整数主问题。需显式提供后端和有序 `PatternGenerator2<f64, Meter>` 链，不选择默认求解器或启发式
主问题回退；可以显式传入原 `ShelfPatternGenerator2`。旧 `Csp2dSolvers` service 不变。

`Csp2dColumnGenerationInput` 包含每物料一个源生成请求、独立主问题需求、初始列、RMP／final 共用的
`ProduceObjective` 及 `Csp2dColumnGenerationConfig`。请求中的产品件数是**生成提示**，不是需求约束或份数上界；
件数／面积上下界使用独立主问题需求。所有请求共用有限非负刀缝，跨物料产品尺寸及旋转权限必须一致。即使当前
无该物料的列，也注册其源库存行。卷材生成会受已声明卷长和更小的显式段高限制；定尺板的段高配置直接拒绝，不静默忽略。

- 非空池从 RMP 开始、空池从 Phase I 开始；认证 RMP 不可行会进入 Phase I 恢复。恢复条件检查原单位实际短缺，
  不仅检查归一化目标很小。只有最优 LP 证书复验通过，才允许继续定价。
- 初始列及实际观察到的生成列先核对源物料、工艺、产品尺寸、旋转权限，再通过共享物理／刀缝／切分树门禁。
  这是与传入目录的一致性校验，不是外部来源认证。非法观察候选即使不能改进目标也报错，未实现领域规则不静默丢弃。
- 检验数按独立件数／面积口径、需求上界贡献和**库存行价格**重算。旧生成器只接收按面积的提示，由件数价格／
  单件面积加面积价格合成，不是权威定价依据。请求自带迭代、历史、价格及候选上限被实际流程状态替换，不信任这些提示
  携带的已有候选池或价格。
- 保留全部初始重复位置；新增列按物理身份去重，不依赖方案／图案名称或放置顺序。身份包含物料、region 和物理放置；
  在当前默认目标下，相同排布的不同有效切分树等价。
- 默认定价 20 批、每批跨**全部物料及生成器**合计观察 32 个候选、完整池 10000 列；各配置上限最大 100000。
  零迭代预算仍可在认证 LP 后执行 final。原生成器同步执行，观察上限不是任意生成器的内存配额或抢占限制。
  总时间、合作取消、原始子进度和报告上限共用执行预算，不是硬实时截止。
- 输出保留完整池、每个实际模型／报告及其池前缀长度、每个完成批次的新增位置／重算价格。正常无改进、迭代或列池
  上限停止后，在同后端、完整池运行 final，除非禁用或时间／取消／报告预算不允许。无证书或其他异常停止的主问题
  不触发 final。定价停止原因与 final 原报告终态分开保留，迟到取消也不改写已完成原报告。
- `candidate_complete` 始终为 false：原生成器不提供 exact pricing。final 最优／不可行只属于实际受限主问题；
  空生成池可能不可行，而源问题可行。不虚构源问题全局结论。

此循环只注册默认行；额外业务 pipeline 继续通过显式 master／context 扩展，必须提供匹配定价修正后才能整合迭代。
不将任意 TS 回调作为跨进程 Rust 策略。严格切树匹配仍会拒绝部分旧 shelf 断头台输出：它们缺少完整成品叶子或叶子
物理尺寸不匹配。有效的显式断头台树已支持，自动修复旧树不在该循环内。

真实 SCIP／Gurobi 测试覆盖原 shelf 的空池恢复、不完整池恢复、多物料绑定库存定价、硬需求上界下 final 不可行、
卷材加权目标、正刀缝有效断头台列及实际子报告身份。fake 契约额外覆盖预算、非最优 LP、取消竞争、物理重复、目录伪造
及微小归一化短缺。bridge 已提供共享无损源 codec、生成源 DTO 和 `ospf-wasm/csp2d` 的 `CuttingContext`
源／列校验。`Csp2dColumnGenerationInput::validate` 与 `validate_columns` 复用原注册及物理门禁，不启动生成器、
求解器或时钟，不证明用量满足需求。`column_keys` 保留原顺序及重复位置，供独立物理轨迹复验。共享执行报告 codec 与
可移植 `Csp2dReport` reader 已复验实际子阶段、原单位短缺、认证定价及精确 final 产出，不声明全局证明。
SDK `Csp2dColumnGeneration` 已通过显式 HTTP／N-API 客户端与共享原生执行器开放，保留有界配置、原始进度／取消，
不隐式回退。真实 SCIP/Gurobi 求解、认证 in-process HTTP 及隔离 N-API 验收，与需显式配置宿主的独立浏览器 HTTP
测试分开记录。资源限制见[桥接说明](../../../docs/typescript-bridge_ch.md#能力矩阵)。

[可执行原生示例](examples/report_column_generation.rs) 从空池开始，显式选择后端；先把所选求解器运行库加入 `PATH`：

```powershell
cargo +nightly-2026-08-01 run -p ospf-rust-framework-csp2d --example report_column_generation --features scip-bundled --target x86_64-pc-windows-msvc -- scip
```

有有效许可证的 Gurobi 环境可改用 `--features gurobi10`，末尾参数改为 `gurobi`。

## 泛型数值边界

1. 领域层泛型于 `V` 并保留编译时单位 `U`。
2. 坐标、尺寸与面积使用 `Quantity<V, U>`；无量纲值使用裸 `V`。
3. 求解器数值类型只出现在 `produce`（建模边界）与应用层。
4. 泛型代码不得调用 `to_f64()`。

## 公开 API

| API | 职责 | 稳定性 |
| --- | --- | --- |
| `Region2` / `CutTree2` / `CuttingProcess2` | 裁切几何、断头台切分树与工艺能力。 | migration |
| `Material2` / `Product2` / `Pattern2` / `CuttingPlan2` | 双模式领域模型。 | migration |
| `ShelfPatternGenerator2` | 带刀缝节距的构造式图案生成。 | migration |
| `Csp2dMasterProblem` | 主问题建模。 | migration |
| `Csp2dSolvers` | MILP 路径与列生成路径。 | migration |
| `Csp2dPricingConfig` / `improving_columns` | 检验数定价与改进列筛选。 | migration |
| `SheetAssignment` | 定尺张数 / 卷材段高的分配口径。 | migration |
| `CostarFiller` | 副产物填充余料。 | migration |
| `Csp2dMasterSolver` | 可注入的主问题求解接口与内置启发式。 | migration |
| `ProduceContext` / `ProduceModel` / `ProduceStage` | 事务注册真实主问题、提取结果及显式 LP 定价口径。 | 显式原生前置 |
| `Csp2dNativeBackend` / `Csp2dMasterResult` | 注入后端的报告主问题执行，无启发式回退。 | 显式原生前置 |
| `solve_csp2d_column_generation` / `Csp2dColumnGenerationInput` | 显式生成器的报告恢复、认证 LP 定价及 final 整数执行。 | 原生；默认规则 HTTP／N-API SDK 门面 |

## 相关文档

- [ospf-rust README](../../../README_ch.md) 列出全部 crate 及其 Kotlin 边界。
- [csp1d](../csp1d/README_ch.md) 是成熟的一维参考。
- [geometry2d](../geometry2d/README_ch.md) 提供共享二维几何。
- [csp2d.md](csp2d.md) 记录迁移交接与已知差距。

## 本地验证

```powershell
$env:RUSTC_BOOTSTRAP = "1"
cargo test -p ospf-rust-framework-csp2d
```

显式主问题的真实后端测试需将所选求解器运行库加入 `PATH`；Gurobi 还需要匹配的安装及有效许可证。
求解器 feature 仍然可选，Gurobi 版本继续互斥。

```powershell
cargo +nightly-2026-08-01 test -p ospf-rust-framework-csp2d --features scip-bundled --target x86_64-pc-windows-msvc --lib application::report_solver
cargo +nightly-2026-08-01 test -p ospf-rust-framework-csp2d --features gurobi10 --target x86_64-pc-windows-msvc --lib application::report_solver
```

### 服务路径规模观测

`tests/workload_profile_contract.rs` 通过已有 `Csp2dMasterSolver` 注入扩展点，记录完整服务路径的
主问题调用次数、候选数量、需求口径及放置数量等聚合信息，验证委托、回退、最终阶段回调和共享记录器并发。
它不增加生产计数器或后台线程。这是契约规模的观测，不是生产流量或耗时基准，也不测内部表读取次数、
缓存命中率或原生求解器 CPU 占用。

```powershell
cargo +nightly-2026-08-01 test --offline -p ospf-rust-framework-csp2d --test workload_profile_contract -- --nocapture --test-threads=1
```

原生求解器线程配置属于单实例，不是与构表线程共享的进程总预算。默认服务不会自动并行构表；生产采用前
需要真实输入及复用频率、明确的原生求解器/并发预算，以及混合负载尾延迟验证。候选数量相同不能证明表内容
可以互换；注入求解器可能在阶段间改变状态，不能直接删掉重复主问题调用。
