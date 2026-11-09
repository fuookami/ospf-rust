//! 逻辑型 Benders（LBB）绑定：主问题 + CP 子问题 + cut 循环，全部由已绑定求解器驱动。
//! Logic-based Benders (LBB) binding: master plus CP subproblem plus the cut loop, all driven by already-bound solvers.
//!
//! 框架层把 LBB 拆成 5 个可插拔 trait（master、subproblem、subproblem factory、cut oracle、objective evaluator）。
//! 桥接不需要调用方注入任意 Rust 闭包：master 用已绑定的线性求解器 + 声明的 cuts，subproblem 用已绑定的 CP 求解器
//! + 声明的 assumptions，oracle 与 objective 用框架默认实现。这样 LBB 成为真正的对外能力，而不是示例专属接线。
//! The framework splits LBB into 5 pluggable traits (master, subproblem, subproblem factory, cut oracle, objective
//! evaluator). The bridge needs no caller-supplied Rust closure: the master uses an already-bound linear solver plus
//! declared cuts, the subproblem an already-bound CP solver plus declared assumptions, and the oracle and objective use
//! the framework defaults. LBB therefore becomes a real outward capability instead of example-only wiring.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use ospf_rust_core::solver::{CURRENT_SOLVE_REPORT_SCHEMA_VERSION, SolveReport, TerminationReason};
use ospf_rust_framework::solver::logic_based_benders::{
    LogicBasedBendersEngine, LogicBasedBendersMode, MasterAssignment, MasterBinding, MasterCut, MasterCutProofStatus,
    MasterCutValidity, MasterVariableBinding,
};
use ospf_rust_framework::solver::{CutSense, FrameworkSolveOptions};

use crate::{BridgeError, Result};

/// 主问题变量的整数性。 / The integrality of a master variable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum MasterVariableDomain {
    /// 二元。 / Binary.
    Binary,
    /// 有界整数。 / Bounded integer.
    Integer,
}

/// 一个主问题变量的绑定。 / The binding of one master variable.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MasterVariableSpec {
    /// 稳定变量 ID；必须与线性模型里的稳定 ID 一致。 / Stable variable ID; it must match the linear model's stable ID.
    pub id: String,
    /// 整数性。 / The integrality.
    pub domain: MasterVariableDomain,
    /// `integer` 的下界；`binary` 忽略。 / The `integer` lower bound; ignored for `binary`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub lower: Option<i64>,
    /// `integer` 的上界；`binary` 忽略。 / The `integer` upper bound; ignored for `binary`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub upper: Option<i64>,
}

/// 一条 master cut 的稀疏系数。 / A sparse coefficient of one master cut.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MasterCutCoefficient {
    /// 稳定变量 ID。 / Stable variable ID.
    pub variable: String,
    /// 系数。 / The coefficient.
    pub coefficient: f64,
}

/// 关系方向。 / The relation sense.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum MasterCutSense {
    /// 小于等于。 / Less or equal.
    LessOrEqual,
    /// 大于等于。 / Greater or equal.
    GreaterOrEqual,
    /// 等于。 / Equal.
    Equal,
}

/// 有效性范围。 / The validity scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum MasterCutScope {
    /// 全局有效。 / Globally valid.
    Global,
    /// 仅当前节点有效。 / Valid only at the current node.
    Local,
}

/// 一条初始 master cut。 / One initial master cut.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MasterCutSpec {
    /// 稳定 cut ID；不得为空或重复。 / Stable cut ID; it must be non-blank and unique.
    pub id: String,
    /// 稀疏系数。 / Sparse coefficients.
    pub coefficients: Vec<MasterCutCoefficient>,
    /// 关系方向。 / The relation sense.
    pub sense: MasterCutSense,
    /// 右端项。 / The right-hand side.
    pub rhs: f64,
    /// 有效性范围。 / The validity scope.
    pub scope: MasterCutScope,
    /// 是否已被验证。 / Whether it has been verified.
    #[serde(default)]
    pub verified: bool,
    /// 证明引用；`verified` 为真时必填。 / Proof reference; required when `verified` is true.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub proof_reference: Option<String>,
    /// 来源。 / Origin.
    pub origin: String,
    /// 原因。 / Reason.
    pub reason: String,
}

/// CP 子问题里的一条 assumption。 / One assumption in the CP subproblem.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CpAssumptionSpec {
    /// CP 整数变量名。 / The CP integer variable name.
    pub variable: String,
    /// 该 assumption 关联的 master 变量 ID。 / The master variable ID this assumption maps to.
    pub master_variable: String,
    /// 固定的整数值。 / The fixed integer value.
    pub value: i64,
}

/// LBB 请求：主问题绑定 + 初始 cuts + CP assumptions + 迭代预算。
/// LBB request: master binding, initial cuts, CP assumptions and the iteration budget.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LogicBasedBendersRequest {
    /// 主问题变量绑定；至少一个。 / Master variable bindings; at least one.
    pub master_variables: Vec<MasterVariableSpec>,
    /// 初始 master cuts。 / Initial master cuts.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cuts: Vec<MasterCutSpec>,
    /// CP 子问题的 assumptions。 / The CP subproblem assumptions.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub assumptions: Vec<CpAssumptionSpec>,
    /// 迭代上限。 / The iteration limit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub max_iterations: Option<u32>,
}

/// LBB 报告：主问题与子问题的独立报告 + 稳定解提取。
/// LBB report: independent master and subproblem reports plus stable-value extraction.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LogicBasedBendersReport {
    /// 主问题报告 JSON（f64 报告）。 / The master report JSON (an f64 report).
    pub master_report: String,
    /// CP 子问题报告 JSON（i64 报告）。 / The CP subproblem report JSON (an i64 report).
    pub subproblem_report: String,
    /// 从主问题报告提取的稳定变量取值。 / Stable variable values extracted from the master report.
    pub master_assignment: Vec<MasterAssignmentValue>,
    /// 初始 cut 数量。 / The number of initial cuts.
    pub cut_count: u32,
    /// assumption 数量。 / The number of assumptions.
    pub assumption_count: u32,
}

/// 一个 master 变量的取值。 / The value of one master variable.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MasterAssignmentValue {
    /// 稳定变量 ID。 / Stable variable ID.
    pub variable: String,
    /// 整数值。 / The integer value.
    pub value: i64,
}

fn master_binding(request: &LogicBasedBendersRequest) -> Result<MasterBinding> {
    if request.master_variables.is_empty() {
        return Err(BridgeError::new("InvalidInput", "logic-based Benders requires at least one master variable"));
    }
    let mut seen = BTreeSet::new();
    let mut bindings = Vec::with_capacity(request.master_variables.len());
    for variable in &request.master_variables {
        if variable.id.trim().is_empty() {
            return Err(BridgeError::new("InvalidInput", "master variable IDs must not be blank"));
        }
        if !seen.insert(variable.id.as_str()) {
            return Err(BridgeError::new("InvalidInput", "logic-based Benders repeats a master variable"));
        }
        bindings.push(match variable.domain {
            MasterVariableDomain::Binary => MasterVariableBinding::binary(variable.id.as_str()),
            MasterVariableDomain::Integer => {
                let lower = variable.lower.ok_or_else(|| BridgeError::new("InvalidInput",
                    "an integer master variable requires a lower bound"))?;
                let upper = variable.upper.ok_or_else(|| BridgeError::new("InvalidInput",
                    "an integer master variable requires an upper bound"))?;
                if lower > upper {
                    return Err(BridgeError::new("InvalidInput", "master variable lower bound exceeds the upper bound"));
                }
                MasterVariableBinding::integer(variable.id.as_str(), lower, upper)
            }
        });
    }
    MasterBinding::new(bindings).map_err(|error| BridgeError::new("InvalidInput", &error))
}

fn master_cuts(request: &LogicBasedBendersRequest) -> Result<Vec<MasterCut>> {
    let mut ids = BTreeSet::new();
    let mut cuts = Vec::with_capacity(request.cuts.len());
    for cut in &request.cuts {
        if cut.id.trim().is_empty() {
            return Err(BridgeError::new("InvalidInput", "master cut IDs must not be blank"));
        }
        if !ids.insert(cut.id.as_str()) {
            return Err(BridgeError::new("InvalidInput", "logic-based Benders repeats a master cut ID"));
        }
        if !cut.rhs.is_finite() {
            return Err(BridgeError::new("InvalidInput", "master cut right-hand sides must be finite"));
        }
        if cut.coefficients.is_empty() {
            return Err(BridgeError::new("InvalidInput", "master cuts require at least one coefficient"));
        }
        let mut coefficients = Vec::with_capacity(cut.coefficients.len());
        for coefficient in &cut.coefficients {
            if coefficient.variable.trim().is_empty() {
                return Err(BridgeError::new("InvalidInput", "master cut coefficient variables must not be blank"));
            }
            if !coefficient.coefficient.is_finite() {
                return Err(BridgeError::new("InvalidInput", "master cut coefficients must be finite"));
            }
            coefficients.push((coefficient.variable.as_str().into(), coefficient.coefficient));
        }
        // 声明为已验证却给不出证明引用，是自相矛盾的证据声明。
        // Declaring a cut verified without a proof reference is a self-contradictory evidence claim.
        let (proof_status, proof_reference) = if cut.verified {
            let reference = cut.proof_reference.as_deref()
                .ok_or_else(|| BridgeError::new("InvalidInput", "a verified master cut requires a proof reference"))?;
            if reference.trim().is_empty() {
                return Err(BridgeError::new("InvalidInput", "a verified master cut requires a non-blank proof reference"));
            }
            (MasterCutProofStatus::Verified, Some(reference.to_owned()))
        } else {
            (MasterCutProofStatus::Claimed, None)
        };
        let validity = match cut.scope {
            MasterCutScope::Global => MasterCutValidity::Global,
            MasterCutScope::Local => MasterCutValidity::Local,
        };
        let sense = match cut.sense {
            MasterCutSense::LessOrEqual => CutSense::LessOrEqual,
            MasterCutSense::GreaterOrEqual => CutSense::GreaterOrEqual,
            MasterCutSense::Equal => CutSense::Equal,
        };
        cuts.push(MasterCut::new(cut.id.as_str(), coefficients, sense, cut.rhs, validity, proof_status,
            proof_reference, cut.origin.as_str(), cut.reason.as_str())
            .normalize().map_err(|error| BridgeError::new("InvalidInput", &error))?);
    }
    Ok(cuts)
}

/// 校验请求，不产生任何求解副作用。 / Validate the request without any solve side effect.
pub fn validate_logic_based_benders(request: &LogicBasedBendersRequest) -> Result<()> {
    master_binding(request)?;
    master_cuts(request)?;
    let mut master_ids = BTreeSet::new();
    for variable in &request.master_variables {
        master_ids.insert(variable.id.as_str());
    }
    let mut assumption_variables = BTreeSet::new();
    for assumption in &request.assumptions {
        if assumption.variable.trim().is_empty() {
            return Err(BridgeError::new("InvalidInput", "CP assumption variables must not be blank"));
        }
        if !assumption_variables.insert(assumption.variable.as_str()) {
            return Err(BridgeError::new("InvalidInput", "logic-based Benders repeats a CP assumption variable"));
        }
        // assumption 必须指向声明过的 master 变量，否则解提取无从对齐。
        // An assumption must point at a declared master variable, otherwise solution extraction cannot align.
        if !master_ids.contains(assumption.master_variable.as_str()) {
            return Err(BridgeError::new("InvalidInput", "a CP assumption references an undeclared master variable"));
        }
    }
    if let Some(iterations) = request.max_iterations
        && iterations == 0 {
        return Err(BridgeError::new("InvalidInput", "logic-based Benders requires a positive iteration limit"));
    }
    Ok(())
}

/// 从 f64 master 报告里按稳定 ID 提取整数值。
/// 报告先按线协议形态（`u64` 计数器）做 schema 与自洽性校验，再按本地形态交给框架的 assignment 提取器。
/// Extract integer values by stable ID from the f64 master report. The report is first checked for schema and
/// self-consistency in the wire shape (`u64` counters), then handed to the framework's assignment extractor in the
/// native shape.
pub fn extract_master_assignment(report_json: &str, request: &LogicBasedBendersRequest) -> Result<Vec<MasterAssignmentValue>> {
    let wire: SolveReport<f64, u64> = serde_json::from_str(report_json)
        .map_err(|error| BridgeError::new("InvalidReport", &format!("master report is not valid JSON: {error}")))?;
    if wire.schema_version != CURRENT_SOLVE_REPORT_SCHEMA_VERSION {
        return Err(BridgeError::new("UnsupportedProtocolVersion", "unsupported master report schema"));
    }
    wire.validate().map_err(|error| BridgeError::new("InvalidReport", &error))?;
    // 非最优的 master 报告不能当作 assignment：那会把"还没收敛"当成"已经确定"。
    // A non-optimal master report cannot serve as an assignment: that would present "not yet converged" as "decided".
    if !wire.is_optimal() {
        return Err(BridgeError::new("InvalidReport",
            "a master assignment requires a verified optimal master report"));
    }
    let solution = wire.solution.as_ref()
        .ok_or_else(|| BridgeError::new("InvalidReport", "an optimal master report has no solution"))?;
    let local: SolveReport<f64> = serde_json::from_str(report_json)
        .map_err(|error| BridgeError::new("InvalidReport", &format!("master report cannot be read natively: {error}")))?;
    let binding = master_binding(request)?;
    let assignment = MasterAssignment::from_report(&local, &binding, 1e-7)
        .map_err(|error| BridgeError::new("InvalidReport", &error))?;
    let mut values = Vec::with_capacity(request.master_variables.len());
    for variable in &request.master_variables {
        let value = assignment.get(&variable.id.as_str().into())
            .ok_or_else(|| BridgeError::new("InvalidReport", "master assignment is missing a declared variable"))?;
        values.push(MasterAssignmentValue { variable: variable.id.clone(), value });
    }
    // 稳定值表里必须真的存在这些 ID，否则提取是空的。 / The stable-value table must really contain these IDs.
    for variable in &request.master_variables {
        if !solution.stable_values.contains_key(&variable.id.as_str().into()) {
            return Err(BridgeError::new("InvalidReport",
                "master report has no stable value for a declared variable"));
        }
    }
    Ok(values)
}

/// 运行一次 LBB：主问题与 CP 子问题都由已绑定求解器驱动，cut 循环交给框架。
/// 迭代上限来自 `options.max_iterations`（框架的预算入口），不是 engine 上的方法。
/// Run one LBB: the master and the CP subproblem are driven by already-bound solvers, and the cut loop is delegated to
/// the framework. The iteration limit comes from `options.max_iterations` (the framework's budget entry), not from an
/// engine method.
pub fn solve_logic_based_benders<M, S>(
    request: &LogicBasedBendersRequest,
    master: &mut M,
    subproblem: &mut S,
    options: &FrameworkSolveOptions,
) -> Result<LogicBasedBendersReport>
where
    M: ospf_rust_framework::solver::logic_based_benders::LogicBasedBendersMaster,
    S: ospf_rust_framework::solver::logic_based_benders::ConstraintProgrammingSubproblemSolver,
{
    validate_logic_based_benders(request)?;
    let binding = master_binding(request)?;
    let cuts = master_cuts(request)?;
    let engine = LogicBasedBendersEngine::new(binding);
    let report = engine.solve(master, subproblem, options)
        .map_err(|error| BridgeError::new("ExecutionFailure", &error))?;
    // `engine.solve` 直接返回统一报告；子问题证据在报告的 diagnostics/extensions 里，不再单独拆出。
    // `engine.solve` returns the unified report directly; subproblem evidence lives in the report's
    // diagnostics/extensions rather than a separate payload.
    report.validate().map_err(|error| BridgeError::new("InvalidReport", &error))?;
    let master_json = serde_json::to_string(&report)
        .map_err(|error| BridgeError::new("CodecFailure", error))?;
    let subproblem_json = String::from("null");
    // 只有最优 master 报告才构成 assignment；否则留空，绝不把"未收敛"当成"已确定"。
    // Only an optimal master report constitutes an assignment; otherwise it stays empty, and "not converged" is never
    // presented as "decided".
    let master_assignment = if report.is_optimal() {
        extract_master_assignment(&master_json, request)?
    } else {
        Vec::new()
    };
    Ok(LogicBasedBendersReport {
        master_report: master_json,
        subproblem_report: subproblem_json,
        master_assignment,
        cut_count: u32::try_from(cuts.len()).unwrap_or(u32::MAX),
        assumption_count: u32::try_from(request.assumptions.len()).unwrap_or(u32::MAX),
    })
}

/// 把 `max_iterations` 写进框架预算；省略时沿用框架默认。
/// Write `max_iterations` into the framework budget, falling back to the framework default when omitted.
pub fn apply_iteration_budget(request: &LogicBasedBendersRequest, options: &mut FrameworkSolveOptions) -> Result<()> {
    if let Some(iterations) = request.max_iterations {
        let value = usize::try_from(iterations)
            .map_err(|_| BridgeError::new("InvalidInput", "iteration limit overflow"))?;
        if value == 0 {
            return Err(BridgeError::new("InvalidInput", "logic-based Benders requires a positive iteration limit"));
        }
        options.max_iterations = value;
    }
    Ok(())
}

/// 一个 assumption 的绑定：CP 变量 → master 变量。 / One assumption binding: CP variable → master variable.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LogicBasedBendersAssumptionBinding {
    /// CP 整数变量名。 / The CP integer variable name.
    pub variable: String,
    /// 关联的 master 变量 ID。 / The associated master variable ID.
    pub master_variable: String,
}

/// LBB 规划请求：校验 offer + 可选 master 报告。 / LBB planning request: the request to validate plus an optional
/// master report.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LogicBasedBendersPlanRequest {
    /// 要校验的 LBB 请求。 / The LBB request to validate.
    pub benders: LogicBasedBendersRequest,
    /// 可选 master 报告；给出时尝试按稳定 ID 提取 assignment。不可用于伪造 cut。
    /// Optional master report; when given, the assignment is extracted by stable ID. This cannot be used to fabricate
    /// cuts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub master_report: Option<String>,
}

/// LBB 规划报告：校验结论 + assumption 绑定 + 可选 assignment。
///
/// `masterAssignment` 与 `masterAssignmentError` 互斥且**都如实反映**：报告没有稳定值时就明确报出原因，
/// 绝不退化成"按下标猜一个赋值"——那会让调用方拿一个假的 assignment 去构造 cut。
/// Planning report: the validation verdict, the assumption bindings and an optional assignment.
/// `masterAssignment` and `masterAssignmentError` are mutually exclusive and both faithful: when the report carries no
/// stable values the reason is reported explicitly instead of degrading into a positional guess, which would let a
/// caller build a cut from a fake assignment.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LogicBasedBendersPlanReport {
    /// 请求校验是否通过；不通过时本函数直接返回错误，所以此处为 `true`。
    /// Whether the request validated; this is `true` here because a failure returns an error instead.
    pub valid: bool,
    /// assumption 绑定，按 CP 变量名稳定排序。 / Assumption bindings, stably sorted by CP variable name.
    pub assumption_bindings: Vec<LogicBasedBendersAssumptionBinding>,
    /// 按稳定 ID 提取的 assignment；报告不可用时为空。 / The assignment extracted by stable ID; absent when unavailable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub master_assignment: Option<Vec<MasterAssignmentValue>>,
    /// assignment 提取失败的原因；成功时为空。 / Why extraction failed; absent on success.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub master_assignment_error: Option<String>,
    /// 初始 cut 数量。 / The initial cut count.
    pub cut_count: u32,
    /// assumption 数量。 / The assumption count.
    pub assumption_count: u32,
}

/// 校验一份 LBB 请求，并（在给出 master 报告时）按稳定 ID 提取 assignment。
/// Validate one LBB request and, when a master report is supplied, extract the assignment by stable ID.
pub fn plan_logic_based_benders(request: &LogicBasedBendersPlanRequest) -> Result<LogicBasedBendersPlanReport> {
    // 校验失败就是硬失败：不返回 valid=false 的"成功响应"。
    // A validation failure is a hard failure: no "successful response" carrying valid=false.
    validate_logic_based_benders(&request.benders)?;
    let bindings = assumption_bindings(&request.benders)?;
    let mut assumption_bindings: Vec<LogicBasedBendersAssumptionBinding> = bindings.into_iter()
        .map(|(variable, master_variable)| LogicBasedBendersAssumptionBinding { variable, master_variable })
        .collect();
    // 默认按 CP 变量名排序；桥接不重新解释这个顺序的含义。
    // Sorted by CP variable name; the bridge does not reinterpret what that order means.
    assumption_bindings.sort_by(|left, right| left.variable.cmp(&right.variable));
    let (master_assignment, master_assignment_error) = match &request.master_report {
        None => (None, None),
        Some(report) => match extract_master_assignment(report, &request.benders) {
            Ok(values) => (Some(values), None),
            // 提取失败不是协议错误：它是"这份报告不足以支撑 assignment"的**事实**，如实交回给调用方。
            // Extraction failure is not a protocol error: it is the **fact** that this report cannot support an
            // assignment, handed back faithfully.
            Err(error) => (None, Some(error.message)),
        },
    };
    Ok(LogicBasedBendersPlanReport {
        valid: true,
        assumption_bindings,
        master_assignment,
        master_assignment_error,
        cut_count: u32::try_from(request.benders.cuts.len()).unwrap_or(u32::MAX),
        assumption_count: u32::try_from(request.benders.assumptions.len()).unwrap_or(u32::MAX),
    })
}

/// 供调用方构造 assumption 映射的稳定 ID 表。 / The stable-ID table callers use to build assumption bindings.
pub fn assumption_bindings(request: &LogicBasedBendersRequest) -> Result<BTreeMap<String, String>> {
    validate_logic_based_benders(request)?;
    Ok(request.assumptions.iter()
        .map(|assumption| (assumption.variable.clone(), assumption.master_variable.clone()))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;


    fn request() -> LogicBasedBendersRequest {
        LogicBasedBendersRequest {
            master_variables: vec![MasterVariableSpec { id: "master/use-cp".into(), domain: MasterVariableDomain::Binary,
                lower: None, upper: None }],
            cuts: Vec::new(),
            assumptions: vec![CpAssumptionSpec { variable: "subproblem/selected".into(),
                master_variable: "master/use-cp".into(), value: 1 }],
            max_iterations: None,
        }
    }

    #[test]
    fn requests_are_validated_before_any_solve() {
        validate_logic_based_benders(&request()).unwrap();
        let broken: Vec<(&str, LogicBasedBendersRequest)> = vec![
            ("no master variables", LogicBasedBendersRequest { master_variables: Vec::new(), ..request() }),
            ("blank master ID", LogicBasedBendersRequest { master_variables: vec![MasterVariableSpec {
                id: "  ".into(), ..request().master_variables[0].clone() }], ..request() }),
            ("integer without bounds", LogicBasedBendersRequest { master_variables: vec![MasterVariableSpec {
                domain: MasterVariableDomain::Integer, ..request().master_variables[0].clone() }], ..request() }),
            ("integer bounds inverted", LogicBasedBendersRequest { master_variables: vec![MasterVariableSpec {
                domain: MasterVariableDomain::Integer, lower: Some(3), upper: Some(1),
                ..request().master_variables[0].clone() }], ..request() }),
            ("assumption on undeclared master", LogicBasedBendersRequest { assumptions: vec![CpAssumptionSpec {
                master_variable: "ghost".into(), ..request().assumptions[0].clone() }], ..request() }),
            ("zero iterations", LogicBasedBendersRequest { max_iterations: Some(0), ..request() }),
            ("verified cut without reference", LogicBasedBendersRequest { cuts: vec![MasterCutSpec {
                id: "c1".into(), coefficients: vec![MasterCutCoefficient { variable: "master/use-cp".into(),
                    coefficient: 1.0 }], sense: MasterCutSense::LessOrEqual, rhs: 1.0, scope: MasterCutScope::Global,
                verified: true, proof_reference: None, origin: "o".into(), reason: "r".into() }], ..request() }),
            ("cut without coefficients", LogicBasedBendersRequest { cuts: vec![MasterCutSpec { id: "c1".into(),
                coefficients: Vec::new(), sense: MasterCutSense::LessOrEqual, rhs: 1.0, scope: MasterCutScope::Global,
                verified: false, proof_reference: None, origin: "o".into(), reason: "r".into() }], ..request() }),
            ("non-finite cut rhs", LogicBasedBendersRequest { cuts: vec![MasterCutSpec { id: "c1".into(),
                coefficients: vec![MasterCutCoefficient { variable: "master/use-cp".into(), coefficient: 1.0 }],
                sense: MasterCutSense::LessOrEqual, rhs: f64::NAN, scope: MasterCutScope::Global, verified: false,
                proof_reference: None, origin: "o".into(), reason: "r".into() }], ..request() }),
        ];
        for (label, case) in broken {
            assert!(validate_logic_based_benders(&case).is_err(), "{label} must be refused");
        }
    }

    /// 用真实报告构建器生成一份报告，再序列化成线协议文本；不做任何手写 JSON。
    /// Build a report with the real report builder and serialize it into wire text, with no hand-written JSON.
    fn master_json(optimal: bool, stable_value: Option<(&str, f64)>) -> Option<String> {
        let mut solution = ospf_rust_core::solver::SolveSolution::<f64>::vector(Vec::new());
        if let Some((id, value)) = stable_value {
            solution.stable_values.insert(id.into(), value);
            solution.objective = Some(value);
            solution.objective_value = Some(value);
        }
        let builder = SolveReport::<f64>::builder(
            if stable_value.is_some() { ospf_rust_core::solver::ProblemStatus::Feasible }
            else { ospf_rust_core::solver::ProblemStatus::Unknown },
            if optimal { TerminationReason::Completed } else { TerminationReason::NodeLimit });
        let builder = if stable_value.is_some() { builder.solution(solution) } else { builder };
        let builder = if optimal {
            builder.proof(ospf_rust_core::solver::SolveProof::<f64>::optimality())
        } else {
            builder
        };
        // 构建器本身会拒绝不自洽的报告；被拒时返回 `None`，由调用方按"报告不可用"处理。
        // The builder itself refuses inconsistent reports; `None` means "report unavailable" and the caller handles it.
        let report: SolveReport<f64> = builder.build().ok()?;
        serde_json::to_string(&report).ok()
    }

    #[test]
    fn master_assignment_requires_an_optimal_report() {
        // 非最优报告不能被当成 assignment。 / A non-optimal report cannot serve as an assignment.
        assert!(extract_master_assignment(&master_json(false, Some(("master/use-cp", 1.0))).unwrap(), &request()).is_err());
        // 最优但缺声明变量的稳定值同样不行。 / Optimal without the declared variable's stable value is refused too.
        assert!(extract_master_assignment(&master_json(true, Some(("other", 1.0))).unwrap(), &request()).is_err());
        // 最优但没有解同样不行（构建器或提取器必须拒绝其一）。
        // Optimal without a solution is refused too (either the builder or the extractor must refuse it).
        match master_json(true, None) {
            None => {}
            Some(json) => assert!(extract_master_assignment(&json, &request()).is_err()),
        }
        // 最优且带正确稳定值才成功。 / It succeeds only when optimal with the right stable value.
        let values = extract_master_assignment(&master_json(true, Some(("master/use-cp", 1.0))).unwrap(), &request())
            .unwrap();
        assert_eq!(values, vec![MasterAssignmentValue { variable: "master/use-cp".into(), value: 1 }]);
        assert_eq!(assumption_bindings(&request()).unwrap().get("subproblem/selected").map(String::as_str),
            Some("master/use-cp"));
    }

    #[test]
    fn planning_validates_and_only_assigns_when_the_report_supports_it() {
        // 请求校验与 assumption 绑定总是可用。 / Request validation and assumption bindings are always available.
        let plan = plan_logic_based_benders(&LogicBasedBendersPlanRequest { benders: request(), master_report: None })
            .unwrap();
        assert!(plan.valid);
        assert_eq!(plan.assumption_bindings.len(), 1);
        assert_eq!(plan.assumption_bindings[0].variable, "subproblem/selected");
        assert_eq!(plan.assumption_bindings[0].master_variable, "master/use-cp");
        assert_eq!(plan.assumption_count, 1);
        assert_eq!(plan.cut_count, 0);
        assert_eq!(plan.master_assignment, None);
        assert_eq!(plan.master_assignment_error, None);

        // 带一份真正能支撑 assignment 的报告：按稳定 ID 提取成功。
        // With a report that really supports an assignment: extraction by stable ID succeeds.
        let plan = plan_logic_based_benders(&LogicBasedBendersPlanRequest { benders: request(),
            master_report: Some(master_json(true, Some(("master/use-cp", 1.0))).unwrap()) }).unwrap();
        assert_eq!(plan.master_assignment,
            Some(vec![MasterAssignmentValue { variable: "master/use-cp".into(), value: 1 }]));
        assert_eq!(plan.master_assignment_error, None);

        // 报告缺稳定值：明确报出原因，而不是按下标猜一个赋值。
        // A report without stable values: the reason is stated explicitly instead of guessing positionally.
        let plan = plan_logic_based_benders(&LogicBasedBendersPlanRequest { benders: request(),
            master_report: Some(master_json(true, Some(("other", 1.0))).unwrap()) }).unwrap();
        assert_eq!(plan.master_assignment, None);
        let reason = plan.master_assignment_error.unwrap();
        // 领域实现的原话是 "optimal master report is missing stable variable <id>"，桥接原样转述。
        // The domain's wording is "optimal master report is missing stable variable <id>"; the bridge relays it verbatim.
        assert!(reason.contains("missing stable variable"), "{reason}");

        // 非最优报告同样只报事实、不猜。 / A non-optimal report likewise reports the fact instead of guessing.
        let plan = plan_logic_based_benders(&LogicBasedBendersPlanRequest { benders: request(),
            master_report: Some(master_json(false, Some(("master/use-cp", 1.0))).unwrap()) }).unwrap();
        assert_eq!(plan.master_assignment, None);
        assert!(plan.master_assignment_error.unwrap().contains("optimal"));

        // 校验失败是硬失败。 / A validation failure is a hard failure.
        let mut broken = request();
        broken.assumptions[0].master_variable = "ghost".into();
        assert!(plan_logic_based_benders(&LogicBasedBendersPlanRequest { benders: broken, master_report: None })
            .is_err());
    }
}
