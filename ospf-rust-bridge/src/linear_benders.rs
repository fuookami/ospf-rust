//! Benders 主问题**单步**绑定：给定模型工件 + 显式 cuts → 统一报告。
//! Benders **single-step** master binding: given a model artifact plus explicit cuts → unified report.
//!
//! 这一步不需要切生成上下文：cuts 由调用方以数据给出，桥接只负责把它们转成 `LinearCut` 并驱动已绑定求解器。
//! 切生成（子问题侧）需要 `MechanismModel`，而全仓没有"线性模型工件 → MechanismModel"的公开转换，所以本模块
//! 只覆盖可移植的那一半，并在文档里写明另一半卡在哪。
//! This step needs no cut-generation context: the caller supplies cuts as data, and the bridge only converts them into
//! `LinearCut` and drives the already-bound solver. Cut generation (the subproblem side) needs a `MechanismModel`, and
//! the repository has no public "linear model artifact → MechanismModel" conversion, so this module covers only the
//! portable half and documents exactly where the other half is blocked.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use ospf_rust_framework::solver::linear_benders_decomposition_solver::{
    LinearBendersDecompositionSolver, LinearCut, LinearSubResult, QuadraticBendersDecompositionSolver, QuadraticCut,
};
use ospf_rust_framework::solver::{CutSense, FrameworkSolveOptions};

use crate::{BridgeError, ModelArtifact, Result};

/// 一条 cut 的稀疏系数。 / A sparse coefficient of one cut.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BendersCutCoefficient {
    /// 模型中的列索引。 / Column index in the model.
    pub column: u32,
    /// 系数。 / The coefficient.
    pub coefficient: f64,
}

/// 关系方向。 / The relation sense.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum BendersCutSense {
    /// 小于等于。 / Less or equal.
    LessOrEqual,
    /// 大于等于。 / Greater or equal.
    GreaterOrEqual,
    /// 等于。 / Equal.
    Equal,
}

impl BendersCutSense {
    fn to_domain(self) -> CutSense {
        match self {
            Self::LessOrEqual => CutSense::LessOrEqual,
            Self::GreaterOrEqual => CutSense::GreaterOrEqual,
            Self::Equal => CutSense::Equal,
        }
    }
}

/// 一条待应用的 cut。 / One cut to apply.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BendersCutSpec {
    /// 稳定 cut 名称；非空且不重复。 / Stable cut name; non-blank and unique.
    pub name: String,
    /// 稀疏系数。 / Sparse coefficients.
    pub coefficients: Vec<BendersCutCoefficient>,
    /// 关系方向。 / The relation sense.
    pub sense: BendersCutSense,
    /// 右端项。 / The right-hand side.
    pub rhs: f64,
}

/// 主问题结果。 / The master result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BendersMasterReport {
    /// 统一求解报告 JSON。 / The unified solve report JSON.
    pub report: String,
    /// 已应用的 cut 数量。 / The number of applied cuts.
    pub cut_count: u32,
}

/// 把 DTO 转成领域 cut；越界列索引与重复名称在这里拒绝。
/// Convert the DTOs into domain cuts, refusing out-of-range column indices and duplicated names here.
pub fn to_linear_cuts(cuts: &[BendersCutSpec], column_count: usize) -> Result<Vec<LinearCut>> {
    let mut names = std::collections::BTreeSet::new();
    let mut built = Vec::with_capacity(cuts.len());
    for cut in cuts {
        if cut.name.trim().is_empty() {
            return Err(BridgeError::new("InvalidInput", "benders cut names must not be blank"));
        }
        if !names.insert(cut.name.as_str()) {
            return Err(BridgeError::new("InvalidInput", "benders cuts repeat a name"));
        }
        if !cut.rhs.is_finite() {
            return Err(BridgeError::new("InvalidInput", "benders cut right-hand sides must be finite"));
        }
        if cut.coefficients.is_empty() {
            return Err(BridgeError::new("InvalidInput", "benders cuts require at least one coefficient"));
        }
        let mut coefficients = Vec::with_capacity(cut.coefficients.len());
        let mut seen_columns = std::collections::BTreeSet::new();
        for coefficient in &cut.coefficients {
            let column = usize::try_from(coefficient.column)
                .map_err(|_| BridgeError::new("InvalidInput", "benders cut column overflow"))?;
            // 越界列索引不能静默丢弃：那会让 cut 变成一条看不见的错约束。
            // An out-of-range column index is never dropped silently: that would turn the cut into an invisible wrong row.
            if column >= column_count {
                return Err(BridgeError::new("InvalidInput",
                    "benders cut references a column outside the model"));
            }
            if !coefficient.coefficient.is_finite() {
                return Err(BridgeError::new("InvalidInput", "benders cut coefficients must be finite"));
            }
            // 同一列重复出现说明调用方搞错了稀疏表示，拒绝而不是自动累加。
            // A repeated column means the caller misused the sparse form; it is refused rather than auto-summed.
            if !seen_columns.insert(column) {
                return Err(BridgeError::new("InvalidInput", "benders cut repeats a column"));
            }
            coefficients.push((column, coefficient.coefficient));
        }
        built.push(match cut.sense {
            BendersCutSense::LessOrEqual => LinearCut::le(cut.name.as_str(), coefficients, cut.rhs),
            BendersCutSense::GreaterOrEqual => LinearCut::ge(cut.name.as_str(), coefficients, cut.rhs),
            BendersCutSense::Equal => LinearCut::eq(cut.name.as_str(), coefficients, cut.rhs),
        });
    }
    Ok(built)
}

/// 求解一次 Benders 主问题。 / Solve one Benders master problem.
/// 泛型而非 `&dyn`：`LinearBendersDecompositionSolver` 带泛型方法 `solve_sub_typed<V>`，不是 dyn 兼容的。
/// Generic rather than `&dyn`: `LinearBendersDecompositionSolver` has a generic `solve_sub_typed<V>` and is therefore
/// not dyn compatible.
pub fn solve_benders_master<S>(
    solver: &S,
    artifact: &ModelArtifact,
    cuts: &[BendersCutSpec],
) -> Result<BendersMasterReport>
where
    S: LinearBendersDecompositionSolver,
{
    let model = crate::decode_model(artifact)?;
    let column_count = model.num_variables();
    let domain_cuts = to_linear_cuts(cuts, column_count)?;
    let options = FrameworkSolveOptions::default();
    let report = solver.solve_master_report_with_options(&model, &domain_cuts, &options)
        .map_err(|error| BridgeError::new("ExecutionFailure", &error.to_string()))?;
    // 报告必须自洽才能往外传。 / The report must be self-consistent before it is relayed.
    report.validate().map_err(|error| BridgeError::new("InvalidReport", &error.to_string()))?;
    Ok(BendersMasterReport {
        report: serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?,
        cut_count: u32::try_from(domain_cuts.len()).unwrap_or(u32::MAX),
    })
}

/// Benders 子问题单步的结论。 / The verdict of one Benders subproblem step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum BendersSubOutcome {
    /// 子问题可行（带对偶，可据以生成最优性 cut）。 / Feasible (duals available for optimality cuts).
    Feasible,
    /// 子问题不可行（带 Farkas 乘子，可据以生成可行性 cut）。 / Infeasible (Farkas multipliers for feasibility cuts).
    Infeasible,
}

/// Benders 子问题单步结果。 / The result of one Benders subproblem step.
///
/// 关键事实：桥接**不生成 cut**。没有 cut 生成上下文时框架侧 `cuts` 永远为空，而 cut 的正确形态属于机理模型
/// （用户的业务语义）。所以这一步把**对偶值**如实交出去，由调用方用同一套 `bendersCuts` 数据形态在下一轮 master
/// 里给出——完整的 Benders 循环因此可以完全由 SDK 侧的数据驱动，而不需要框架新增公开面。
/// Key fact: the bridge **does not generate cuts**. Without a cut-generation context the framework's `cuts` is always
/// empty, and the correct cut shape belongs to the mechanism model (caller business semantics). So this step hands the
/// **duals** out faithfully, and the caller supplies them as the same `bendersCuts` data form in the next master round —
/// the full Benders loop is therefore drivable entirely from SDK-side data, with no new framework public surface needed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BendersSubReport {
    /// 子问题结论。 / The subproblem verdict.
    pub outcome: BendersSubOutcome,
    /// 统一报告 JSON；框架不提供报告时为空。 / Unified report JSON; absent when the framework provides none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub report: Option<String>,
    /// 约束对偶值（可行时为对偶，不可行时为 Farkas 乘子）。
    /// Constraint dual values (optimality duals when feasible, Farkas multipliers when infeasible).
    pub constraint_duals: Vec<f64>,
    /// 固定住的列数，等于传进来的 master 解长度。 / The number of fixed columns, equal to the master solution length.
    pub fixed_columns: u32,
    /// 框架侧产出的 cut 数量；无 cut 上下文时恒为 0。
    /// The number of cuts produced by the framework; always 0 without a cut-generation context.
    pub generated_cut_count: u32,
}

/// 校验并序列化一份可选报告；报告不自洽时立刻失败。 / Validate and serialize an optional report, failing on
/// inconsistency.
fn validated_report(report: Option<&ospf_rust_core::solver::SolveReport<f64>>) -> Result<Option<String>> {
    match report {
        None => Ok(None),
        Some(report) => {
            report.validate().map_err(|error| BridgeError::new("InvalidReport", &error.to_string()))?;
            Ok(Some(serde_json::to_string(report)
                .map_err(|error| BridgeError::new("CodecFailure", error))?))
        }
    }
}

/// 求解一次 Benders 子问题：模型工件 + master 解向量 → 子问题结论与对偶。
///
/// 固定的列由**位置**决定：框架在没有 cut 上下文时按列下标把 `master_solution` 逐项固定，
/// 因此 master 变量必须排在模型的前若干列，其余列保持自由。这是可验证的行为，不是假设。
/// Solve one Benders subproblem: model artifact plus the master solution vector → the subproblem verdict and duals.
/// The fixed columns are decided **positionally**: without a cut context the framework fixes `master_solution`
/// entry-by-entry by column index, so the master variables must occupy the leading columns and the rest stay free. This
/// is measured behaviour, not an assumption.
pub fn solve_benders_sub<S>(
    solver: &S,
    artifact: &ModelArtifact,
    master_solution: &[f64],
) -> Result<BendersSubReport>
where
    S: LinearBendersDecompositionSolver,
{
    let model = crate::decode_model(artifact)?;
    let column_count = model.num_variables();
    if master_solution.iter().any(|value| !value.is_finite()) {
        return Err(BridgeError::new("InvalidInput", "the master solution values must be finite"));
    }
    // 解向量比列数还长：说明调用方把子问题的列也塞进来了，那会静默越界固定。
    // A solution longer than the column count means the caller included subproblem columns too, which would silently fix
    // past the end.
    if master_solution.len() > column_count {
        return Err(BridgeError::new("InvalidInput",
            "the master solution is longer than the model's column count"));
    }
    let options = FrameworkSolveOptions::default();
    let result = solver.solve_sub_with_options(&model, master_solution, &options)
        .map_err(|error| BridgeError::new("ExecutionFailure", &error.to_string()))?;
    let fixed_columns = u32::try_from(master_solution.len()).unwrap_or(u32::MAX);
    let (outcome, report, constraint_duals, generated_cut_count) = match result {
        LinearSubResult::Feasible(feasible) => {
            let report = validated_report(feasible.report.as_ref())?;
            (BendersSubOutcome::Feasible, report,
                feasible.dual_solution.constraints,
                u32::try_from(feasible.cuts.as_ref().map_or(0, Vec::len)).unwrap_or(u32::MAX))
        }
        LinearSubResult::Infeasible(infeasible) => {
            let report = validated_report(infeasible.report.as_ref())?;
            (BendersSubOutcome::Infeasible, report,
                infeasible.farkas_dual_solution.constraints,
                u32::try_from(infeasible.cuts.as_ref().map_or(0, Vec::len)).unwrap_or(u32::MAX))
        }
    };
    // 对偶值必须是有限数：把 NaN 交给调用方去生成 cut，只会得到一条静默错的约束。
    // Duals must be finite: handing NaN to a caller that builds a cut yields a silently wrong row.
    if constraint_duals.iter().any(|value| !value.is_finite()) {
        return Err(BridgeError::new("InvalidReport", "the subproblem returned a non-finite dual value"));
    }
    Ok(BendersSubReport {
        outcome,
        report,
        constraint_duals,
        fixed_columns,
        generated_cut_count,
    })
}

/// 一个二次项：列对 + 系数。 / One quadratic term: a column pair plus its coefficient.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BendersQuadraticTerm {
    /// 第一个列索引。 / The first column index.
    pub left: u32,
    /// 第二个列索引。 / The second column index.
    pub right: u32,
    /// 系数。 / The coefficient.
    pub coefficient: f64,
}

/// 一条二次 cut：线性部分 + 二次项。 / One quadratic cut: a linear part plus quadratic terms.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BendersQuadraticCutSpec {
    /// 稳定 cut 名称；非空且不重复。 / Stable cut name; non-blank and unique.
    pub name: String,
    /// 线性系数。 / Linear coefficients.
    #[serde(default)]
    pub linear: Vec<BendersCutCoefficient>,
    /// 二次项。 / Quadratic terms.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub quadratic: Vec<BendersQuadraticTerm>,
    /// 关系方向。 / The relation sense.
    pub sense: BendersCutSense,
    /// 右端项。 / The right-hand side.
    pub rhs: f64,
}

/// 把 DTO 转成二次 cut；越界列、重复列对、空 cut 都在这里拒绝。
/// Convert the DTOs into quadratic cuts, refusing out-of-range columns, repeated column pairs and empty cuts here.
pub fn to_quadratic_cuts(cuts: &[BendersQuadraticCutSpec], column_count: usize) -> Result<Vec<QuadraticCut>> {
    let mut names = std::collections::BTreeSet::new();
    let mut built = Vec::with_capacity(cuts.len());
    for cut in cuts {
        if cut.name.trim().is_empty() {
            return Err(BridgeError::new("InvalidInput", "benders quadratic cut names must not be blank"));
        }
        if !names.insert(cut.name.as_str()) {
            return Err(BridgeError::new("InvalidInput", "benders quadratic cuts repeat a name"));
        }
        if !cut.rhs.is_finite() {
            return Err(BridgeError::new("InvalidInput", "benders quadratic cut right-hand sides must be finite"));
        }
        // 既没有线性项也没有二次项的 cut 是一条空约束，编译它只会得到恒真的行。
        // A cut with neither linear nor quadratic terms is an empty row that compiles to something always true.
        if cut.linear.is_empty() && cut.quadratic.is_empty() {
            return Err(BridgeError::new("InvalidInput", "benders quadratic cuts require at least one term"));
        }
        let mut linear = Vec::with_capacity(cut.linear.len());
        let mut seen_columns = std::collections::BTreeSet::new();
        for coefficient in &cut.linear {
            let column = usize::try_from(coefficient.column)
                .map_err(|_| BridgeError::new("InvalidInput", "benders quadratic cut column overflow"))?;
            if column >= column_count {
                return Err(BridgeError::new("InvalidInput",
                    "benders quadratic cut references a column outside the model"));
            }
            if !coefficient.coefficient.is_finite() {
                return Err(BridgeError::new("InvalidInput", "benders quadratic cut coefficients must be finite"));
            }
            if !seen_columns.insert(column) {
                return Err(BridgeError::new("InvalidInput", "benders quadratic cut repeats a linear column"));
            }
            linear.push((column, coefficient.coefficient));
        }
        let mut quadratic = Vec::with_capacity(cut.quadratic.len());
        let mut seen_pairs = std::collections::BTreeSet::new();
        for term in &cut.quadratic {
            let left = usize::try_from(term.left)
                .map_err(|_| BridgeError::new("InvalidInput", "benders quadratic term column overflow"))?;
            let right = usize::try_from(term.right)
                .map_err(|_| BridgeError::new("InvalidInput", "benders quadratic term column overflow"))?;
            if left >= column_count || right >= column_count {
                return Err(BridgeError::new("InvalidInput",
                    "benders quadratic term references a column outside the model"));
            }
            if !term.coefficient.is_finite() {
                return Err(BridgeError::new("InvalidInput", "benders quadratic term coefficients must be finite"));
            }
            // 列对规范成左 <= 右；同一列对重复出现说明调用方搞错了稀疏表示，拒绝而不是自动累加。
            // Pairs are normalised to left <= right; a repeated pair means a misused sparse form and is refused rather
            // than auto-summed.
            let pair = if left <= right { (left, right) } else { (right, left) };
            if !seen_pairs.insert(pair) {
                return Err(BridgeError::new("InvalidInput", "benders quadratic cut repeats a column pair"));
            }
            quadratic.push((pair, term.coefficient));
        }
        built.push(QuadraticCut::new(cut.name.as_str(), linear, quadratic, cut.rhs, cut.sense.to_domain()));
    }
    Ok(built)
}

/// 求解一次 Benders **二次**主问题：二次模型工件 + 显式线性/二次 cuts → 统一报告。
/// 与线性侧对称，同样不需要 cut 生成上下文。
/// Solve one Benders **quadratic** master: a quadratic model artifact plus explicit linear and quadratic cuts → a
/// unified report. Symmetric with the linear side and likewise needing no cut-generation context.
pub fn solve_benders_quadratic_master<S>(
    solver: &S,
    artifact: &ModelArtifact,
    linear_cuts: &[BendersCutSpec],
    quadratic_cuts: &[BendersQuadraticCutSpec],
) -> Result<BendersMasterReport>
where
    S: QuadraticBendersDecompositionSolver,
{
    let model = crate::decode_quadratic_model(artifact)?;
    let column_count = model.num_variables();
    let domain_linear = to_linear_cuts(linear_cuts, column_count)?;
    let domain_quadratic = to_quadratic_cuts(quadratic_cuts, column_count)?;
    let report = solver.solve_master_quadratic_report(&model, &domain_linear, &domain_quadratic)
        .map_err(|error| BridgeError::new("ExecutionFailure", &error.to_string()))?;
    report.validate().map_err(|error| BridgeError::new("InvalidReport", &error.to_string()))?;
    Ok(BendersMasterReport {
        report: serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?,
        cut_count: u32::try_from(domain_linear.len() + domain_quadratic.len()).unwrap_or(u32::MAX),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cut() -> BendersCutSpec {
        BendersCutSpec { name: "optimality-1".into(),
            coefficients: vec![BendersCutCoefficient { column: 0, coefficient: 1.0 }],
            sense: BendersCutSense::GreaterOrEqual, rhs: 0.0 }
    }

    #[test]
    fn cuts_are_converted_and_out_of_range_columns_are_refused() {
        let cuts = to_linear_cuts(&[cut()], 2).unwrap();
        assert_eq!(cuts.len(), 1);

        let broken: Vec<(&str, Vec<BendersCutSpec>, usize)> = vec![
            ("blank name", vec![BendersCutSpec { name: "  ".into(), ..cut() }], 2),
            ("repeated name", vec![cut(), cut()], 2),
            ("no coefficients", vec![BendersCutSpec { coefficients: Vec::new(), ..cut() }], 2),
            ("non-finite rhs", vec![BendersCutSpec { rhs: f64::NAN, ..cut() }], 2),
            ("column out of range", vec![cut()], 0),
            ("repeated column", vec![BendersCutSpec { coefficients: vec![
                BendersCutCoefficient { column: 0, coefficient: 1.0 },
                BendersCutCoefficient { column: 0, coefficient: 2.0 }], ..cut() }], 2),
            ("non-finite coefficient", vec![BendersCutSpec { coefficients: vec![
                BendersCutCoefficient { column: 0, coefficient: f64::INFINITY }], ..cut() }], 2),
        ];
        for (label, cuts, column_count) in broken {
            assert!(to_linear_cuts(&cuts, column_count).is_err(), "{label} must be refused");
        }
    }

    #[test]
    fn quadratic_cuts_are_normalised_and_bad_shapes_are_refused() {
        let cut = || BendersQuadraticCutSpec { name: "q1".into(),
            linear: vec![BendersCutCoefficient { column: 0, coefficient: 1.0 }],
            quadratic: vec![BendersQuadraticTerm { left: 1, right: 0, coefficient: 2.0 }],
            sense: BendersCutSense::LessOrEqual, rhs: 4.0 };
        let cuts = to_quadratic_cuts(&[cut()], 2).unwrap();
        assert_eq!(cuts.len(), 1);
        // 列对被规范成左 <= 右，稀疏表示不因下标顺序不同而分裂。
        // The column pair is normalised to left <= right so a sparse form does not split on index order.
        assert_eq!(cuts[0].quadratic_coefficients, vec![((0, 1), 2.0)]);
        let broken: Vec<(&str, Vec<BendersQuadraticCutSpec>, usize)> = vec![
            ("blank name", vec![BendersQuadraticCutSpec { name: "  ".into(), ..cut() }], 2),
            ("repeated name", vec![cut(), cut()], 2),
            ("no terms", vec![BendersQuadraticCutSpec { linear: Vec::new(), quadratic: Vec::new(), ..cut() }], 2),
            ("non-finite rhs", vec![BendersQuadraticCutSpec { rhs: f64::NAN, ..cut() }], 2),
            ("linear column out of range", vec![BendersQuadraticCutSpec {
                linear: vec![BendersCutCoefficient { column: 5, coefficient: 1.0 }], ..cut() }], 2),
            ("quadratic column out of range", vec![BendersQuadraticCutSpec {
                quadratic: vec![BendersQuadraticTerm { left: 0, right: 9, coefficient: 1.0 }], ..cut() }], 2),
            ("repeated pair", vec![BendersQuadraticCutSpec { quadratic: vec![
                BendersQuadraticTerm { left: 0, right: 1, coefficient: 1.0 },
                BendersQuadraticTerm { left: 1, right: 0, coefficient: 2.0 }], ..cut() }], 2),
            ("non-finite term", vec![BendersQuadraticCutSpec { quadratic: vec![
                BendersQuadraticTerm { left: 0, right: 1, coefficient: f64::INFINITY }], ..cut() }], 2),
        ];
        for (label, cuts, column_count) in broken {
            assert!(to_quadratic_cuts(&cuts, column_count).is_err(), "{label} must be refused");
        }
    }
}
