//! 可移植 WASM 边界；不包含原生求解器 / Portable WASM boundary without native solvers.

use std::rc::Rc;
use std::str::FromStr;
use bigdecimal::BigDecimal;
use num_bigint::BigInt;
use num_rational::BigRational;
use wasm_bindgen::prelude::*;
use ospf_rust_multiarray::{AccessOrder, AbstractShape, DynMultiArray, DynShape, MultiArrayBuilder};
use ospf_rust_quantities::quantity::Quantity as RustQuantity;
use ospf_rust_quantities::unit::Unit;
use ospf_rust_core::model::{ConstraintRelation, ObjectiveCategory};
use ospf_rust_core::variable::VariableType;
use ospf_rust_framework::solver::remote::{
    SerializedVariable, SerializedVariableType,
};
use ospf_rust_bridge::{BridgeError, encode_model, linear_variable_range, register_linear_variable};

mod handles;
mod linear;
mod quadratic;
mod cp;
mod analysis;
mod combinatorial;
mod fixed_recourse;
mod column_generation;
mod bpp1d;
mod bpp_loading;
pub use bpp_loading::{DiscreteLoadingApplicationReport, DiscreteLoadingBendersReport, DiscreteLoadingContext, DiscreteLoadingReport, DiscreteLoadingResult};
pub use bpp_loading::{PartitionLoadingBatch, PartitionLoadingContext, PartitionLoadingMaster, PartitionLoadingPricing};
pub use bpp_loading::PartitionColumnGenerationReport;
mod bpp2d;
mod bpp3d;
mod csp1d;
mod csp2d;
mod gantt;
mod geometry2d;
pub use gantt::{GanttActivityContext, GanttActivityExecutionReportResult, GanttActivityReplayResult, GanttActivitySourceSummaryResult};
pub use geometry2d::Geometry2d;
pub use csp1d::{Csp1dCuttingContext, Csp1dPatternGenerationResult, Csp1dProductionResult};
pub use csp2d::{Csp2dCuttingContext, Csp2dExecutionReportResult, Csp2dProductionResult};
pub use bpp1d::{Bpp1dPackingContext, Bpp1dPackingResult, Bpp1dPatternGenerationResult, Bpp1dReport};
pub use bpp2d::{Bpp2dPackingContext, Bpp2dExecutionReportResult, Bpp2dProductionResult};
pub use bpp3d::{Bpp3dExecutionReportResult, Bpp3dPackingContext, Bpp3dPackingResult};
pub use column_generation::ColumnGenerationReport;
pub use analysis::{CertifiedPricingDuals, ConstraintActivityReport, GeneralizedFlowReport, NetworkDesignReport, NetworkStructureReport, AllocationReport, NetworkResilienceReport, VrpReport, QuadraticOrchestrationReport, IndexedRegistrationReport, LinearColumnCertificate, HeartBeatSnapshot, ResponseEnvelope, LinearLifecycleReport, QueryPlanReport, BendersMasterReport, BendersSubReport, LogicBasedBendersPlanReport, RemoteArtifactReport, ConflictExplanationSummary, CandidateFunnelReport, EffectivenessProbeReport, CriticalReportVerdict, GroupSummaryReport, BandProbeReport, EvidenceProbeReport, ProofProbeReport, GapProbeReport, DiagnosticsProbeReport, FingerprintProbeReport, BlockingSetReport, CorrectionProbeReport, SensitivityProbeReport, FunnelProbeReport, EffectivenessRankingReport, PersistenceSortProbeReport, PersistenceSchemaReport, PredicateScopeReport, NativeTerminalProofReport, NativeLoweringReport, SolveValueReport, EffectivenessReport, CriticalAnalysisReport, RelaxationReport, FixedIntegerLpReport, PerturbationVerification, TargetVerification, MultiTargetReportSummary, CriticalityProfileReport, HyperHeuristicConfigReport, RewardPolicyReport, CheckpointReport, LinearMappingReport, LbbResumeIdentityReport, CpCheckpointReport, BendersMetricsReport, BendersCutReport, SymbolConditionReport, SymbolLinearizationReport, SymbolCoreRelationsReport, HeuristicScheduleReport, QTableReport, MasterCutEncodingReport, AnalysisGroupAggregateReport, PredicateFieldResolutionReport, ScalarFunctionReport, PolynomialLiftReport, BoundedNoGoodReport, EffectivenessAggregateReport, AssumptionBindingReport, TierClassificationReport, PolarityReport, MonomialReport, MaterializeReport, BackendFeaturesReport, CanonicalMonomialReport, ExpressionKeyReport, EncodingLabelReport, ActivityBandReport, EvidenceReport, ActivityReportVerdict, AuxiliaryDomainReport, BindingAssignmentReport, SourceIdentityReport, CutValidityReport, AssignmentCompositionReport, DumpOptionsReport, NativeWriteScopeReport, WriterRegistryReport, ShortlistReport, LazyInitReport, DumpBatchReport, QuadraticElasticReport, StableValueReport, MasterAssignmentReport, SparseTriadReport, MatrixRowsReport, PolynomialProbeReport, ModelViewReport, AnalysisOptionsReport, CapabilityMatrixReport, CpSupportMergeReport, SubproblemResultReport, NoGoodOracleReport, CpModelBuildReport, SnapshotGateReport, ConstraintCanonReport, AssignmentEvalReport, ExpressionBoundsReport, DomainProbeReport, IdentityWrapReport, ContainerProbeReport, ActivationSetReport, TierSelectionReport, NonStationaryReport, TrainingSplitReport, AcceptanceRecoveryReport, HypervolumeReport, OperatorCapabilityReport, ScheduleLogReport, EvidenceRemapReport, DominanceReport, DispersionReport, ReferencePointsReport, ArchiveReport, ConstraintRankingReport, PriorityReport, PopulationConfigReport, BoundaryReport, AcceptanceReport, RankingProbeReport, TopologyReport, MigrationReport, ArchiveProbeReport, PiecewiseReport, DiversityReport, SolutionStatusReport, PredicateFieldFilterReport, ModelConfigurationReport, CacheKeyIdentityReport, CacheKeyAddressReport, FlattenCacheReport, UpdateAssignmentReport, BooleanScopeReport, UnsupportedPredicateReport, RepositoryCallReport, FunctionUsageReport, InputDomainReport, NativeWriteReport, ElasticModelReport, CombineStatusReport, NumberTextReport, BigMPolicyReport, ConditionalSemanticsReport, SearchBudgetReport, PredicateSchemaReport, SortByReport, VariableKindDomainReport, CriticalAssemblyReport, FlattenIdentityReport, MasterCutReport, BooleanFoldReport, FieldResolutionReport,  FieldFilterReport, AssignmentReport, SortPlanReport, PipelinePlanReport, LinearIisReport, LinearSolutionPoolReport, LinearSweepReport, LocalConstraintSensitivityReport, NetworkFlowReport};
pub use combinatorial::CombinatorialReport;
pub use fixed_recourse::FixedRecourseReport;
pub use cp::{ConstraintProgrammingAssumptions, ConstraintProgrammingConstraint, ConstraintProgrammingModel, ConstraintProgrammingReport, ConstraintProgrammingSnapshot, IntegerDomain, IntegerExpression, IntegerExpressions};
pub use handles::{LinearExpressionSymbol, QuadraticExpressionSymbol, Token};
pub use linear::{Linear, LinearInequality};
pub use quadratic::{Quadratic, QuadraticInequality};

fn error(message: impl ToString) -> JsValue {
    bridge_error(BridgeError::new("InvalidInput", message))
}

fn bridge_error(error: BridgeError) -> JsValue {
    JsValue::from_str(&serde_json::to_string(&error)
        .unwrap_or_else(|_| "InvalidInput".into()))
}

/// 协议主版本 / Protocol major version.
#[wasm_bindgen(js_name = protocolVersion)]
pub fn protocol_version() -> u32 { ospf_rust_bridge::PROTOCOL_VERSION }

/// Rust 生成的共享协议声明 / Shared protocol declarations generated by Rust.
#[wasm_bindgen(js_name = protocolDeclarations)]
pub fn protocol_declarations() -> String { ospf_rust_bridge::typescript_declarations() }

/// 校验并读取原生报告 / Validate and read a native report.
#[wasm_bindgen(js_name = summarizeReport)]
pub fn summarize_report(json: &str) -> std::result::Result<String, JsValue> {
    serde_json::to_string(&ospf_rust_bridge::summarize_report(json).map_err(bridge_error)?).map_err(error)
}

/// 连续行优先动态数组；所有交换均复制 / Contiguous row-major dynamic array; transfers are copied.
#[wasm_bindgen]
pub struct MultiArray { value: DynMultiArray<f64> }

#[wasm_bindgen]
impl MultiArray {
    /// 从批量数据创建，先校验 shape 防止 Rust 索引 panic / Validate shape before construction to avoid indexing panics.
    #[wasm_bindgen(constructor)]
    pub fn new(shape: &[u32], values: &[f64]) -> std::result::Result<MultiArray, JsValue> {
        let count = shape.iter().rev().try_fold(1usize, |n, &v| n.checked_mul(v as usize)).ok_or_else(|| error("shape overflow"))?;
        if count != values.len() || values.iter().any(|v| !v.is_finite()) { return Err(error("invalid array shape or values")); }
        let shape = DynShape::new(shape.iter().map(|&v| v as usize).collect());
        Ok(Self { value: MultiArrayBuilder::from_list(shape, values.to_vec()) })
    }

    /// 形状副本 / Shape copy.
    pub fn shape(&self) -> Vec<u32> {
        (0..self.value.shape.dimension())
            .filter_map(|axis| self.value.shape.len_of_dimension(axis).ok())
            .map(|item| item as u32)
            .collect()
    }

    /// 读取行优先数据副本 / Read a row-major data copy.
    pub fn values(&self) -> Vec<f64> { self.value.flatten(AccessOrder::RowMajor) }

    /// 读取线性索引 / Read a flat index.
    pub fn get(&self, index: u32) -> std::result::Result<f64, JsValue> {
        if index as usize >= self.value.shape.len() { return Err(error("array index out of bounds")); }
        Ok(self.value[index as usize])
    }

    /// 二维矩阵乘法，复用原 einsum 实现 / 2D matrix multiplication reusing the original einsum implementation.
    #[wasm_bindgen(js_name = matmul)]
    pub fn matmul(&self, other: &MultiArray) -> std::result::Result<MultiArray, JsValue> {
        let value = ospf_rust_multiarray::einsum::matmul(&self.value, &other.value).map_err(|error| self.array_error(error))?;
        if value.flatten(AccessOrder::RowMajor).iter().any(|item| !item.is_finite()) { return Err(error("matrix product overflow")); }
        Ok(Self { value })
    }

    /// 一维向量点积 / 1D vector dot product.
    pub fn dot(&self, other: &MultiArray) -> std::result::Result<f64, JsValue> {
        let value = ospf_rust_multiarray::einsum::dot(&self.value, &other.value).map_err(|error| self.array_error(error))?;
        if !value.is_finite() { return Err(error("dot product overflow")); }
        Ok(value)
    }

    /// 二维转置 / 2D transpose.
    pub fn transpose(&self) -> std::result::Result<MultiArray, JsValue> {
        let value = ospf_rust_multiarray::einsum::transpose(&self.value).map_err(|error| self.array_error(error))?;
        Ok(Self { value })
    }

    /// 方阵迹 / Square-matrix trace.
    pub fn trace(&self) -> std::result::Result<f64, JsValue> {
        let value = ospf_rust_multiarray::einsum::trace(&self.value).map_err(|error| self.array_error(error))?;
        if !value.is_finite() { return Err(error("trace overflow")); }
        Ok(value)
    }

    /// 两个一维向量的外积 / Outer product of two 1D vectors.
    pub fn outer(&self, other: &MultiArray) -> std::result::Result<MultiArray, JsValue> {
        let value = ospf_rust_multiarray::einsum::outer(&self.value, &other.value).map_err(|error| self.array_error(error))?;
        if value.flatten(AccessOrder::RowMajor).iter().any(|item| !item.is_finite()) { return Err(error("outer product overflow")); }
        Ok(Self { value })
    }

    /// 沿指定轴缩并两个张量：缩并轴长度必须相等，其余轴按原顺序保留。
    /// Contract two tensors along the given axes: the contracted axes must have equal length and every other axis
    /// keeps its original order.
    pub fn contract(&self, axis: u32, other: &MultiArray, other_axis: u32) -> std::result::Result<MultiArray, JsValue> {
        let value = ospf_rust_multiarray::einsum::contract(&self.value, axis as usize, &other.value, other_axis as usize)
            .map_err(|error| self.array_error(error))?;
        if value.flatten(AccessOrder::RowMajor).iter().any(|item| !item.is_finite()) { return Err(error("contraction overflow")); }
        Ok(Self { value })
    }

    /// 重排形状，复用原 `reshape` 语义：元素不足截断、超出补零，不报错也不重算。
    /// Reinterpret the shape through the original `reshape` semantics: missing elements are truncated and extra
    /// ones are zero-filled, without errors or recomputation.
    pub fn reshape(&self, shape: &[u32]) -> std::result::Result<MultiArray, JsValue> {
        let count = shape.iter().rev().try_fold(1usize, |n, &v| n.checked_mul(v as usize)).ok_or_else(|| error("shape overflow"))?;
        if count > 8 * 1024 * 1024 { return Err(error("reshaped array exceeds 8 Mi elements")); }
        let shape = DynShape::new(shape.iter().map(|&v| v as usize).collect());
        Ok(Self { value: self.value.reshape(shape) })
    }

    /// 全局求和 / Sum every element.
    #[wasm_bindgen(js_name = sumAll)]
    pub fn sum_all(&self) -> std::result::Result<f64, JsValue> {
        use ospf_rust_multiarray::FastSum;
        let value = FastSum::sum_all(&self.value);
        if !value.is_finite() { return Err(error("array sum overflow")); }
        Ok(value)
    }

    /// 沿单个轴求和，该轴被消去 / Sum along one axis, removing it.
    #[wasm_bindgen(js_name = sumAxis)]
    pub fn sum_axis(&self, axis: u32) -> std::result::Result<MultiArray, JsValue> {
        use ospf_rust_multiarray::FastSum;
        let value = FastSum::sum_axis(&self.value, axis as usize).map_err(|error| self.array_error(error))?;
        if value.flatten(AccessOrder::RowMajor).iter().any(|item| !item.is_finite()) { return Err(error("array sum overflow")); }
        Ok(Self { value })
    }

    /// 沿多个轴求和，这些轴被消去 / Sum along several axes, removing them.
    #[wasm_bindgen(js_name = sumAxes)]
    pub fn sum_axes(&self, axes: &[u32]) -> std::result::Result<MultiArray, JsValue> {
        use ospf_rust_multiarray::FastSum;
        let axes: Vec<usize> = axes.iter().map(|&axis| axis as usize).collect();
        let value = FastSum::sum_axes(&self.value, &axes).map_err(|error| self.array_error(error))?;
        if value.flatten(AccessOrder::RowMajor).iter().any(|item| !item.is_finite()) { return Err(error("array sum overflow")); }
        Ok(Self { value })
    }

    /// 沿一个轴的前缀和 / Prefix sum along one axis.
    #[wasm_bindgen(js_name = cumsumAxis)]
    pub fn cumsum_axis(&self, axis: u32) -> std::result::Result<MultiArray, JsValue> {
        use ospf_rust_multiarray::FastCumSum;
        let value = FastCumSum::cumsum_axis(&self.value, axis as usize).map_err(|error| self.array_error(error))?;
        if value.flatten(AccessOrder::RowMajor).iter().any(|item| !item.is_finite()) { return Err(error("array prefix sum overflow")); }
        Ok(Self { value })
    }
}

impl MultiArray {
    /// 把 einsum 错误转成边界错误；形状不匹配属于输入错误。
    /// Translate einsum errors into boundary errors; a shape mismatch is an input error.
    fn array_error(&self, error: impl std::fmt::Display) -> JsValue {
        crate::error(&error.to_string())
    }
}

fn unit(symbol: &str) -> std::result::Result<Unit, JsValue> {
    let unit: ospf_rust_bridge::QuantityUnit = serde_json::from_value(serde_json::Value::String(symbol.into())).map_err(error)?;
    Ok(unit.unit())
}

/// 运行时 f64 物理量，单位逻辑复用 Rust / Runtime f64 quantity reusing Rust unit semantics.
#[wasm_bindgen]
pub struct Quantity { value: RustQuantity<f64, Unit> }

#[wasm_bindgen]
impl Quantity {
    /// 创建带单位物理量 / Create a quantity with a unit.
    #[wasm_bindgen(constructor)]
    pub fn new(value: f64, symbol: &str) -> std::result::Result<Quantity, JsValue> {
        if !value.is_finite() { return Err(error("quantity value must be finite")); }
        Ok(Self { value: RustQuantity::new(value, unit(symbol)?) })
    }

    /// 校验量纲后转换 / Convert after checking dimensions.
    #[wasm_bindgen(js_name = toUnit)]
    pub fn to_unit(&self, symbol: &str) -> std::result::Result<Quantity, JsValue> {
        let value = self.value.to_unit(&unit(symbol)?).map_err(error)?;
        if !value.value().is_finite() { return Err(error("quantity conversion overflow")); }
        Ok(Self { value })
    }

    /// 同量纲相加 / Add quantities of the same dimension.
    pub fn add(&self, other: &Quantity) -> std::result::Result<Quantity, JsValue> {
        let value = self.value.checked_add(&other.value).map_err(error)?;
        if !value.value().is_finite() { return Err(error("quantity addition overflow")); }
        Ok(Self { value })
    }

    /// 同量纲相减 / Subtract quantities of the same dimension.
    pub fn sub(&self, other: &Quantity) -> std::result::Result<Quantity, JsValue> {
        let value = self.value.checked_sub(&other.value).map_err(error)?;
        if !value.value().is_finite() { return Err(error("quantity subtraction overflow")); }
        Ok(Self { value })
    }

    /// 无量纲标量缩放，单位保持不变 / Scale by a dimensionless scalar, keeping the unit.
    pub fn scale(&self, factor: f64) -> std::result::Result<Quantity, JsValue> {
        if !factor.is_finite() { return Err(error("quantity scale factor must be finite")); }
        let value = self.value.clone() * factor;
        if !value.value().is_finite() { return Err(error("quantity scaling overflow")); }
        Ok(Self { value })
    }

    /// 物理量相乘，量纲按原单位代数合并（如 m × m = m^2、m ÷ s = m/s）。
    /// Multiply quantities, combining dimensions through the original unit algebra (for example m × m = m^2 or m ÷ s = m/s).
    pub fn mul(&self, other: &Quantity) -> std::result::Result<Quantity, JsValue> {
        let value = self.value.clone() * other.value.clone();
        if !value.value().is_finite() { return Err(error("quantity multiplication overflow")); }
        Ok(Self { value })
    }

    /// 物理量相除，量纲按原单位代数合并；零除数拒绝，不伪造无穷。
    /// Divide quantities, combining dimensions through the original unit algebra; a zero divisor is rejected instead of fabricating infinity.
    pub fn div(&self, other: &Quantity) -> std::result::Result<Quantity, JsValue> {
        if *other.value.value() == 0.0 { return Err(error("quantity division by zero")); }
        let value = self.value.clone() / other.value.clone();
        if !value.value().is_finite() { return Err(error("quantity division overflow")); }
        Ok(Self { value })
    }

    /// 同量纲精确比较，返回 -1／0／1；跨量纲直接拒绝，不静默按数值比。
    /// Compare quantities of the same dimension, returning -1/0/1; a cross-dimension comparison is rejected instead of silently comparing raw numbers.
    pub fn compare(&self, other: &Quantity) -> std::result::Result<i32, JsValue> {
        if !self.value.unit().same_dimension(other.value.unit()) {
            return Err(error("quantity comparison requires the same dimension"));
        }
        match self.value.partial_cmp(&other.value) {
            Some(std::cmp::Ordering::Less) => Ok(-1),
            Some(std::cmp::Ordering::Greater) => Ok(1),
            Some(std::cmp::Ordering::Equal) => Ok(0),
            None => Err(error("quantity comparison is not total")),
        }
    }

    /// 数值 / Numeric value.
    pub fn value(&self) -> f64 { *self.value.value() }

    /// 单位符号；量纲合并得到的单位可能没有词表条目，此时为空串，请改用 `dimension()`。
    /// Unit symbol; a unit produced by dimension algebra may have no vocabulary entry, in which case this is
    /// empty and callers must use `dimension()` instead.
    pub fn symbol(&self) -> String { self.value.unit().symbol().into() }

    /// 量纲符号，例如 `L`、`L^2`、`L/T` / Dimension symbol such as `L`, `L^2` or `L/T`.
    pub fn dimension(&self) -> String { self.value.unit().dimension().symbol().to_string() }
}

/// f64 元模型绑定，所有索引均属于当前模型。
/// f64 meta-model binding; every index belongs to this model.
#[wasm_bindgen]
pub struct MetaModel {
    scope: Rc<linear::Scope>,
}

impl Drop for MetaModel {
    fn drop(&mut self) {
        self.scope.disposed.set(true);
        self.scope.source.borrow_mut().take();
        self.scope.quadratic_objective.borrow_mut().clear();
    }
}

#[wasm_bindgen]
impl MetaModel {
    /// 创建元模型 / Create a meta model.
    #[wasm_bindgen(constructor)]
    pub fn new(name: &str) -> std::result::Result<MetaModel, JsValue> {
        if name.trim().is_empty() { return Err(error("model name is blank")); }
        Ok(Self { scope: Rc::new(linear::Scope::new(name)) })
    }

    /// 注册变量；null 表示无界 / Register a variable; null means unbounded.
    #[wasm_bindgen(js_name = addVariable)]
    pub fn add_variable(
        &mut self,
        name: &str,
        kind: &str,
        lower: Option<f64>,
        upper: Option<f64>,
    ) -> std::result::Result<u32, JsValue> {
        let variable_type = match kind {
            "continuous" => SerializedVariableType::Continuous,
            "integer" => SerializedVariableType::Integer,
            "binary" => SerializedVariableType::Binary,
            _ => return Err(error("unknown variable type")),
        };
        let index = self.scope.variables.get();
        let variable = SerializedVariable {
            stable_id: format!("variable:{index}"), index, name: name.into(),
            lower_bound: lower.unwrap_or(f64::NEG_INFINITY),
            upper_bound: upper.unwrap_or(f64::INFINITY), variable_type,
        };
        let mut source = self.scope.model_mut()?;
        source.transaction(|source| {
            register_linear_variable(source, &variable).map_err(bridge_error)?;
            source.try_to_quadratic_tetrad_model().map_err(error)?;
            source.clear_solution();
            Ok::<_, JsValue>(())
        })?;
        self.scope.variables.set(source.tokens().len());
        u32::try_from(index).map_err(error)
    }

    /// 替换变量范围并清除旧赋值；二元变量省略边界时恢复 0 或 1。
    /// Replace a variable range and clear old assignments; omitted binary bounds default to 0 or 1.
    #[wasm_bindgen(js_name = setVariableRange)]
    pub fn set_variable_range(
        &mut self,
        index: u32,
        lower: Option<f64>,
        upper: Option<f64>,
    ) -> std::result::Result<(), JsValue> {
        let mut source = self.scope.model_mut()?;
        let kind = match source.tokens().get(index as usize).map(|token| token.var_type()) {
            Some(VariableType::Continuous) => SerializedVariableType::Continuous,
            Some(VariableType::Integer) => SerializedVariableType::Integer,
            Some(VariableType::Binary) => SerializedVariableType::Binary,
            Some(_) => return Err(error("token type is not exposed by this binding")),
            None => return Err(error("variable index out of bounds")),
        };
        let binary = kind == SerializedVariableType::Binary;
        let range = linear_variable_range(
            kind,
            lower.unwrap_or(if binary { 0.0 } else { f64::NEG_INFINITY }),
            upper.unwrap_or(if binary { 1.0 } else { f64::INFINITY }),
        ).map_err(bridge_error)?;
        source.transaction(|source| {
            source.set_variable_range_by_index(index as usize, range).map_err(error)?;
            source.try_to_quadratic_tetrad_model().map_err(error)?;
            source.clear_solution();
            Ok::<_, JsValue>(())
        })
    }

    /// 按注册索引获取稳定身份的原生 token 句柄。
    /// Get a stable-identity native token handle by its registration index.
    pub fn token(&self, index: u32) -> std::result::Result<Token, JsValue> {
        Token::new(self.scope.clone(), index as usize)
    }

    /// 注册真正的 Rust 线性表达式符号，依赖沿表达式运算自动传递。
    /// Register a real Rust linear-expression symbol, preserving dependencies through arithmetic.
    #[wasm_bindgen(js_name = addLinearSymbol)]
    pub fn add_linear_symbol(&mut self, name: &str, expression: &Linear) -> std::result::Result<LinearExpressionSymbol, JsValue> {
        handles::register_symbol(self.scope.clone(), name, expression)
    }

    /// 注册真正的 Rust 二次表达式符号，依赖沿表达式运算自动传递。
    /// Register a real Rust quadratic-expression symbol, preserving dependencies through arithmetic.
    #[wasm_bindgen(js_name = addQuadraticSymbol)]
    pub fn add_quadratic_symbol(&mut self, name: &str, expression: &Quadratic) -> std::result::Result<QuadraticExpressionSymbol, JsValue> {
        handles::register_quadratic_symbol(self.scope.clone(), name, expression)
    }

    /// 按变量顺序绑定显式赋值；仅检查数值、范围和整数性，不宣称是可行解。
    /// Bind an explicit assignment in variable order; validate values, bounds and integrality, not feasibility.
    #[wasm_bindgen(js_name = setSolution)]
    pub fn set_solution(&mut self, values: &[f64]) -> std::result::Result<(), JsValue> {
        let mut source = self.scope.model_mut()?;
        if values.len() != source.tokens().len()
            || source.tokens().iter().zip(values).any(|(token, &value)| {
                !value.is_finite() || !token.variable.is_valid_value(value)
                    || (token.var_type().is_integer() && (value.fract() != 0.0 || value.abs() > 9_007_199_254_740_991.0))
            })
        { return Err(error("invalid assignment shape, value, range or integrality")); }
        source.set_solution(values);
        Ok(())
    }

    /// 清除 token 赋值 / Clear token assignments.
    #[wasm_bindgen(js_name = clearSolution)]
    pub fn clear_solution(&mut self) -> std::result::Result<(), JsValue> {
        self.scope.model_mut()?.clear_solution();
        Ok(())
    }

    /// 构造当前模型的线性表达式 / Construct a linear expression owned by this model.
    #[wasm_bindgen(js_name = linearExpression)]
    pub fn linear_expression(
        &self,
        indices: &[u32],
        coefficients: &[f64],
        constant: f64,
    ) -> std::result::Result<Linear, JsValue> {
        Linear::new(self.scope.clone(), indices, coefficients, constant)
    }

    /// 设置对象型线性目标；当前交换格式不支持非零目标常数。
    /// Set an object-based linear objective; nonzero objective constants are not supported by the exchange yet.
    #[wasm_bindgen(js_name = setLinearExpressionObjective)]
    pub fn set_linear_expression_objective(&mut self, expression: &Linear, maximize: bool) -> std::result::Result<(), JsValue> {
        expression.check_scope(&self.scope)?;
        if expression.constant()? != 0.0 {
            return Err(bridge_error(BridgeError::new("Unsupported", "nonzero objective constants are not yet supported")));
        }
        self.set_linear_objective(&expression.coefficients()?, maximize)
    }

    /// 注册关系快照；释放原表达式不会删除已注册的约束。
    /// Register a relation snapshot; freeing its source expression does not remove the registered constraint.
    #[wasm_bindgen(js_name = addInequality)]
    pub fn add_inequality(&mut self, inequality: &LinearInequality, name: &str) -> std::result::Result<(), JsValue> {
        inequality.expression.check_scope(&self.scope)?;
        let relation = match inequality.sign {
            ospf_rust_framework::solver::remote::SerializedConstraintSign::LessEqual => "le",
            ospf_rust_framework::solver::remote::SerializedConstraintSign::GreaterEqual => "ge",
            ospf_rust_framework::solver::remote::SerializedConstraintSign::Equal => "eq",
            _ => return Err(error("unknown constraint relation")),
        };
        self.add_linear_constraint(&inequality.expression.coefficients()?, relation, inequality.rhs, name)
    }

    /// 设置稠密线性目标 / Set a dense linear objective.
    #[wasm_bindgen(js_name = setLinearObjective)]
    pub fn set_linear_objective(&mut self, coefficients: &[f64], maximize: bool) -> std::result::Result<(), JsValue> {
        if coefficients.len() != self.scope.variables.get() || coefficients.iter().any(|value| !value.is_finite()) {
            return Err(error("invalid objective shape or coefficients"));
        }
        self.scope.model_mut()?.transaction(|source| {
            source.set_linear_objective(coefficients.to_vec(), if maximize { ObjectiveCategory::Maximum } else { ObjectiveCategory::Minimum });
            source.try_to_quadratic_tetrad_model().map_err(error)?;
            source.clear_solution();
            Ok::<_, JsValue>(())
        })?;
        self.scope.quadratic_objective.borrow_mut().clear();
        Ok(())
    }

    /// 添加稠密线性约束 / Add a dense linear constraint.
    #[wasm_bindgen(js_name = addLinearConstraint)]
    pub fn add_linear_constraint(
        &mut self,
        coefficients: &[f64],
        relation: &str,
        rhs: f64,
        name: &str,
    ) -> std::result::Result<(), JsValue> {
        if coefficients.len() != self.scope.variables.get() || !rhs.is_finite() || coefficients.iter().any(|value| !value.is_finite()) {
            return Err(error("invalid constraint shape or values"));
        }
        let sign = match relation {
            "le" => ConstraintRelation::LessEqual,
            "ge" => ConstraintRelation::GreaterEqual,
            "eq" => ConstraintRelation::Equal,
            _ => return Err(error("unknown constraint relation")),
        };
        let terms: Vec<_> = coefficients.iter().copied().enumerate().filter(|(_, value)| *value != 0.0).collect();
        self.scope.model_mut()?.transaction(|source| {
            source.add_linear_constraint(&terms, sign, rhs, name).map_err(error)?;
            source.try_to_quadratic_tetrad_model().map_err(error)?;
            source.clear_solution();
            Ok::<_, JsValue>(())
        })?;
        Ok(())
    }

    /// 导出带 core 指纹的无损模型 / Export a lossless model with a core fingerprint.
    #[wasm_bindgen(js_name = toLinearTriadModel)]
    pub fn to_linear_triad_model(&self) -> std::result::Result<String, JsValue> {
        let quadratic = self.scope.compile_quadratic()?;
        if !quadratic.quadratic_constraints.is_empty() || quadratic.Q.rows.iter().any(|row| !row.entries.is_empty()) {
            return Err(bridge_error(BridgeError::new("Unsupported", "quadratic model cannot be exported as linear")));
        }
        let model = self.scope.model()?.try_to_linear_triad_model().map_err(error)?;
        serde_json::to_string(&encode_model(&model).map_err(bridge_error)?).map_err(error)
    }
}

/// 无损十进制数 / Lossless decimal number.
#[wasm_bindgen]
pub struct Decimal { value: BigDecimal }

#[wasm_bindgen]
impl Decimal {
    /// 从文本创建 / Create from text.
    #[wasm_bindgen(constructor)]
    pub fn new(value: &str) -> std::result::Result<Decimal, JsValue> {
        Ok(Self { value: BigDecimal::from_str(value).map_err(error)? })
    }

    /// 精确相加 / Add exactly.
    pub fn add(&self, other: &Decimal) -> Decimal { Self { value: &self.value + &other.value } }

    /// 精确相乘 / Multiply exactly.
    pub fn mul(&self, other: &Decimal) -> Decimal { Self { value: &self.value * &other.value } }

    /// 精确相减 / Subtract exactly.
    pub fn sub(&self, other: &Decimal) -> Decimal { Self { value: &self.value - &other.value } }

    /// 精确取负 / Negate exactly.
    pub fn neg(&self) -> Decimal { Self { value: -&self.value } }

    /// 精确比较，返回 -1／0／1；不做浮点转换。 / Exact comparison returning -1/0/1 without float conversion.
    pub fn compare(&self, other: &Decimal) -> i32 {
        match self.value.partial_cmp(&other.value) {
            Some(std::cmp::Ordering::Less) => -1,
            Some(std::cmp::Ordering::Greater) => 1,
            _ => 0,
        }
    }

    /// 是否恰好为零 / Whether the value is exactly zero.
    #[wasm_bindgen(js_name = isZero)]
    pub fn is_zero(&self) -> bool { self.value.partial_cmp(&BigDecimal::from(0)) == Some(std::cmp::Ordering::Equal) }

    /// 无损文本 / Lossless text.
    #[wasm_bindgen(js_name = toString)]
    pub fn to_text(&self) -> String { self.value.to_string() }
}

/// 任意精度有理数 / Arbitrary-precision rational number.
#[wasm_bindgen]
pub struct Rational { value: BigRational }

#[wasm_bindgen]
impl Rational {
    /// 从整数文本创建，分母不能为零 / Create from integer text with a nonzero denominator.
    #[wasm_bindgen(constructor)]
    pub fn new(numerator: &str, denominator: &str) -> std::result::Result<Rational, JsValue> {
        let denominator = BigInt::from_str(denominator).map_err(error)?;
        if denominator == BigInt::from(0) { return Err(error("zero denominator")); }
        Ok(Self { value: BigRational::new(BigInt::from_str(numerator).map_err(error)?, denominator) })
    }

    /// 精确相加 / Add exactly.
    pub fn add(&self, other: &Rational) -> Rational { Self { value: &self.value + &other.value } }

    /// 精确相乘 / Multiply exactly.
    pub fn mul(&self, other: &Rational) -> Rational { Self { value: &self.value * &other.value } }

    /// 精确相减 / Subtract exactly.
    pub fn sub(&self, other: &Rational) -> Rational { Self { value: &self.value - &other.value } }

    /// 精确相除；零除数必须拒绝，不能伪造无穷。 / Divide exactly, rejecting a zero divisor instead of fabricating infinity.
    pub fn div(&self, other: &Rational) -> std::result::Result<Rational, JsValue> {
        if other.value.numer() == &BigInt::from(0) { return Err(error("zero divisor")); }
        Ok(Self { value: &self.value / &other.value })
    }

    /// 精确取负 / Negate exactly.
    pub fn neg(&self) -> Rational { Self { value: -&self.value } }

    /// 精确比较，返回 -1／0／1；不做浮点转换。 / Exact comparison returning -1/0/1 without float conversion.
    pub fn compare(&self, other: &Rational) -> i32 {
        match self.value.cmp(&other.value) {
            std::cmp::Ordering::Less => -1,
            std::cmp::Ordering::Greater => 1,
            std::cmp::Ordering::Equal => 0,
        }
    }

    /// 是否恰好为零 / Whether the value is exactly zero.
    #[wasm_bindgen(js_name = isZero)]
    pub fn is_zero(&self) -> bool { self.value.numer() == &BigInt::from(0) }

    /// 无损文本 / Lossless text.
    #[wasm_bindgen(js_name = toString)]
    pub fn to_text(&self) -> String { self.value.to_string() }
}

#[cfg(all(test, target_arch = "wasm32"))]
mod tests {
    use super::*;
    use wasm_bindgen_test::wasm_bindgen_test;

    #[wasm_bindgen_test]
    fn portable_report_metadata_is_not_limited_by_wasm_pointer_width() {
        use ospf_rust_core::solver::{CombinatorialSolveReport, ProblemStatus, SolveReport, TerminationReason};
        let report = SolveReport::<f64>::builder(ProblemStatus::Unknown, TerminationReason::Completed).build().unwrap();
        let text = serde_json::to_string(&report).unwrap()
            .replace("\"nodes\":null", "\"nodes\":18446744073709551615")
            .replace("\"generatedColumns\":0", "\"generatedColumns\":18446744073709551615");
        ospf_rust_bridge::summarize_report(&text).unwrap();
        let mut portable: ospf_rust_core::solver::SolveReport<f64, u64> = serde_json::from_str(&text).unwrap();
        portable.validate().unwrap();
        assert_eq!(portable.statistics.nodes, Some(u64::MAX));
        assert_eq!(portable.trace.generated_columns, u64::MAX);
        portable.trace.iteration_snapshots.push(ospf_rust_core::solver::SolveIterationSnapshot {
            iteration: 1, stage: "pricing".into(), items_added: u64::MAX, items_total: u64::MAX,
            ..Default::default()
        });
        ospf_rust_bridge::summarize_report(&serde_json::to_string(&portable).unwrap()).unwrap();
        let aggregate = CombinatorialSolveReport {
            report, attempts: vec![], selected_attempt_id: None, selection_reason: None,
        };
        let text = serde_json::to_string(&aggregate).unwrap()
            .replace("\"threadCount\":null", "\"threadCount\":18446744073709551615");
        let portable: CombinatorialSolveReport<f64, u64> = serde_json::from_str(&text).unwrap();
        portable.validate().unwrap();
        assert_eq!(portable.report.provenance.thread_count, Some(u64::MAX));
    }

    #[wasm_bindgen_test]
    fn model_and_exact_numbers() {
        let mut model = MetaModel::new("example").unwrap();
        model.add_variable("x", "continuous", Some(0.0), None).unwrap();
        model.set_linear_objective(&[1.0], true).unwrap();
        let artifact: ospf_rust_bridge::ModelArtifact = serde_json::from_str(&model.to_linear_triad_model().unwrap()).unwrap();
        ospf_rust_bridge::decode_model(&artifact).unwrap();
        let value = Decimal::new("9007199254740993.1").unwrap();
        assert_eq!(value.add(&Decimal::new("0.2").unwrap()).to_text(), "9007199254740993.3");
    }

    #[wasm_bindgen_test]
    fn variable_ranges_keep_identity_and_reject_invalid_updates_atomically() {
        let mut model = MetaModel::new("variable-ranges").unwrap();
        model.add_variable("x", "continuous", Some(0.0), Some(10.0)).unwrap();
        model.add_variable("i", "integer", Some(-3.0), Some(3.0)).unwrap();
        model.add_variable("b", "binary", Some(0.0), Some(1.0)).unwrap();
        let token = model.token(0).unwrap();
        let id = token.id().unwrap();
        model.set_solution(&[4.0, -2.0, 1.0]).unwrap();
        let before = model.to_linear_triad_model().unwrap();
        for (index, lower, upper) in [
            (0, Some(f64::NAN), Some(2.0)), (0, Some(3.0), Some(2.0)),
            (1, Some(0.5), Some(2.0)), (1, Some(0.0), Some(9_007_199_254_740_992.0)),
            (2, Some(-1.0), Some(1.0)), (3, None, None),
        ] {
            assert!(model.set_variable_range(index, lower, upper).is_err());
            assert_eq!(model.to_linear_triad_model().unwrap(), before);
            assert_eq!(token.result().unwrap(), Some(4.0));
        }
        model.set_variable_range(0, Some(-2.0), Some(3.0)).unwrap();
        assert_eq!(token.id().unwrap(), id);
        assert_eq!(token.lower_bound().unwrap(), Some(-2.0));
        assert_eq!(token.upper_bound().unwrap(), Some(3.0));
        assert_eq!(token.result().unwrap(), None);
        assert_ne!(model.to_linear_triad_model().unwrap(), before);
        model.set_variable_range(0, None, None).unwrap();
        assert_eq!(token.lower_bound().unwrap(), None);
        assert_eq!(token.upper_bound().unwrap(), None);
        let binary = model.token(2).unwrap();
        model.set_variable_range(2, Some(1.0), None).unwrap();
        assert_eq!(binary.lower_bound().unwrap(), Some(1.0));
        assert_eq!(binary.upper_bound().unwrap(), Some(1.0));
        model.set_variable_range(2, None, None).unwrap();
        assert_eq!(binary.lower_bound().unwrap(), Some(0.0));
        assert_eq!(binary.upper_bound().unwrap(), Some(1.0));
    }

    #[wasm_bindgen_test]
    fn expressions_match_dense_models_and_keep_ownership() {
        let mut model = MetaModel::new("expressions").unwrap();
        model.add_variable("x", "continuous", Some(0.0), Some(10.0)).unwrap();
        model.add_variable("y", "binary", Some(0.0), Some(1.0)).unwrap();
        let x = model.linear_expression(&[0], &[1.0], 0.0).unwrap();
        let y = model.linear_expression(&[1], &[1.0], 0.0).unwrap();
        let objective = x.mul(2.0).unwrap().add(&y).unwrap();
        assert_eq!(objective.evaluate(&[3.0, 1.0]).unwrap(), 7.0);
        assert_eq!(objective.coefficients().unwrap(), vec![2.0, 1.0]);
        assert_eq!(objective.sub(&x).unwrap().coefficients().unwrap(), vec![1.0, 1.0]);
        model.set_linear_expression_objective(&objective, true).unwrap();
        let inequality = x.add(&y).unwrap().offset(3.0).unwrap().compare("le", 7.0).unwrap();
        model.add_inequality(&inequality, "capacity").unwrap();
        let artifact = model.to_linear_triad_model().unwrap();
        model.set_linear_objective(&[2.0, 1.0], true).unwrap();
        let mut dense = MetaModel::new("expressions").unwrap();
        dense.add_variable("x", "continuous", Some(0.0), Some(10.0)).unwrap();
        dense.add_variable("y", "binary", Some(0.0), Some(1.0)).unwrap();
        dense.set_linear_objective(&[2.0, 1.0], true).unwrap();
        dense.add_linear_constraint(&[1.0, 1.0], "le", 4.0, "capacity").unwrap();
        assert_eq!(artifact, dense.to_linear_triad_model().unwrap());
        let foreign = dense.linear_expression(&[0], &[1.0], 0.0).unwrap();
        assert!(x.add(&foreign).is_err());
        assert!(dense.add_inequality(&inequality, "foreign").is_err());
        assert!(model.set_linear_expression_objective(&x.offset(1.0).unwrap(), true).is_err());
        assert!(model.linear_expression(&[2], &[1.0], 0.0).is_err());
        assert!(x.mul(f64::MAX).unwrap().mul(2.0).is_err());
        assert!(objective.evaluate(&[1.0]).is_err());
        model.add_variable("later", "continuous", None, None).unwrap();
        assert_eq!(x.coefficients().unwrap(), vec![1.0, 0.0, 0.0]);
        assert!(x.evaluate(&[1.0, 0.0]).is_err());
        drop(model);
        assert!(x.constant().is_err());
        assert!(objective.mul(1.0).is_err());
    }
}
