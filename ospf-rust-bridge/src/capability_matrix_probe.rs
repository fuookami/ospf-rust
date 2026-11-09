//! 续挖第 66 单：能力矩阵的**推断格**——空矩阵也敢说"支持活动性"、覆盖项只改查询不改"原生"、
//! `Conditional` 在 `supports()` 里等同于"可用"（`analysis/mod.rs` 的 `CapabilityMatrix` / `inferred_support` / `is_native`）
//! Continuation, delivery 66: the capability matrix's **inference lattice** — an empty matrix still claims activity support,
//! an override changes the query but never the "native" flag, and `Conditional` counts as available to `supports()`
//! (`CapabilityMatrix` / `inferred_support` / `is_native` in `analysis/mod.rs`).
//!
//! 第 65 单量的是**配置闸门**，这一单量它下游的**能力推断**：`inferred_support` 是一张纯数据真值表，
//! 十三个能力、五个输入（`model_types` / `dual` / `fallback_satisfaction_only` / `native_*` / `exact_cp_lowering`），
//! 一个求解器都不用就能全部测到。七条实测：
//!
//! 1. **空矩阵不是"什么都不支持"**（`an_empty_matrix_claims_activity_support()`）：`Activity` 与
//!    `ConstraintGroupAggregation` 恒 `Supported`，`RhsPerturbation` / `RemovalTest` / `ObjectiveTarget` /
//!    `AdaptivePerturbation` 在 `model_types` 为空时给 `Conditional`。
//! 2. **`FixedIntegerLpSensitivity` 是三态阶梯**（`the_fixed_integer_ladder_is_three_tiered()`）：
//!    有 `dual` 且（`Mip` 或 `exact_cp_lowering`）→ `Supported`；只有 `dual` → `Conditional`；无 `dual` → `Unsupported`。
//! 3. **扰动/删除/target 三个能力**不看 `dual`**（`the_perturbation_trio_ignores_the_dual()`）：
//!    只有 `model_types` 决定 `Conditional` 还是 `Supported`。
//! 4. **`AdaptivePerturbation` 是 `RhsPerturbation` 的恒等映射**（`adaptive_mirrors_rhs_perturbation_exactly()`）。
//! 5. **`AssumptionSolving` 与 `Conflict` 吃 fallback，`NativeUnsatCore` 不吃**
//!    （`only_two_capabilities_accept_the_fallback()`）：前者两者都可 `Conditional`，`NativeUnsatCore` 非原生即
//!    `Unsupported`。
//! 6. **`Mus` 先看 `Conflict`**（`mus_is_gated_on_conflict()`）：`Conflict` 为 `Unsupported` 时 `Mus` 一定
//!    `Unsupported`；再按 `model_types` 空否给 `Conditional` / `Supported`。
//! 7. **覆盖项只改查询、不改"原生"**（`an_override_never_upgrades_the_native_flag()`）：`is_native` 直接读四个布尔
//!    字段，绕过 `analysis_capabilities`；把某能力覆盖成 `Supported` 也不会让它变成"原生"。
//! 8. **`supports()` 把 `Conditional` 也算"可用"**（`conditional_counts_as_available()`）：判据是
//!    `!= Unsupported`。
//!
//! Delivery 65 measured the **configuration gates**; this one measures the **capability inference** downstream of them:
//! `inferred_support` is a pure-data truth table over thirteen capabilities and five inputs, testable without a single
//! solver. **An empty matrix does not mean "nothing is supported"**: `Activity` and `ConstraintGroupAggregation` are always
//! `Supported`, while four others fall to `Conditional` with empty `model_types`. **`FixedIntegerLpSensitivity` is a
//! three-tier ladder.** **The perturbation trio ignores the dual.** **`AdaptivePerturbation` is an identity map onto
//! `RhsPerturbation`.** **Only `AssumptionSolving` and `Conflict` accept the fallback** — `NativeUnsatCore` is all-or-nothing.
//! **`Mus` is gated on `Conflict`.** **An override never upgrades the native flag**, because `is_native` reads the four
//! booleans directly. And **`supports()` treats `Conditional` as available**, since the test is `!= Unsupported`.

use ospf_rust_core::analysis::{
    AnalysisCapability, CapabilityMatrix, ConstraintProgrammingFeature, SolverModelType,
};
use ospf_rust_core::solver::{CapabilitySupport, ConstraintProgrammingSupport};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 能力矩阵探针请求。 / The capability-matrix probe request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CapabilityMatrixRequest {
    /// 支持的模型种类名（`Linear` / `Mip` / `Quadratic` / `Miqp` / `ConstraintProgramming`）。
    /// Supported model-kind names.
    #[serde(default)]
    pub model_types: Vec<String>,
    #[serde(default)]
    pub dual: Option<bool>,
    #[serde(default)]
    pub warm_start: Option<bool>,
    #[serde(default)]
    pub exact_cp_lowering: Option<bool>,
    #[serde(default)]
    pub native_assumption_solving: Option<bool>,
    #[serde(default)]
    pub native_unsat_core: Option<bool>,
    #[serde(default)]
    pub fallback_satisfaction_only: Option<bool>,
    /// 显式覆盖：`能力名=级别`（级别为 `Supported` / `Conditional` / `Unsupported`）。
    /// Explicit overrides as `capability=level`.
    #[serde(default)]
    pub overrides: Vec<String>,
}

/// 一个能力的读数。 / One capability reading.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CapabilityRow {
    /// 稳定能力名。 / The stable capability name.
    pub capability: String,
    /// `support()` 的结论。 / The `support()` verdict.
    pub support: String,
    /// `supports()` 的结论。 / The `supports()` verdict.
    pub available: bool,
    /// `is_native()` 的结论。 / The `is_native()` verdict.
    pub native: bool,
}

/// 能力矩阵探针结论。 / The capability-matrix probe verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CapabilityMatrixReport {
    /// 十三个能力的读数。 / The thirteen capability readings.
    pub rows: Vec<CapabilityRow>,
    /// `lp_dual()` 的读数。 / The `lp_dual()` reading.
    pub lp_dual: bool,
    /// `cp_support(Assumption)` 的读数。 / The `cp_support(Assumption)` reading.
    pub cp_assumption: String,
    /// `cp_support(BooleanLogic)` 的读数。 / The `cp_support(BooleanLogic)` reading.
    pub cp_boolean_logic: String,
    /// 空矩阵的 `Activity` 读数。 / The empty matrix's `Activity` reading.
    pub empty_activity: String,
    /// 空矩阵的 `RhsPerturbation` 读数。 / The empty matrix's `RhsPerturbation` reading.
    pub empty_rhs_perturbation: String,
    /// 空矩阵的 `NativeUnsatCore` 读数。 / The empty matrix's `NativeUnsatCore` reading.
    pub empty_native_unsat_core: String,
    /// 空矩阵的 `Mus` 读数。 / The empty matrix's `Mus` reading.
    pub empty_mus: String,
    /// 空矩阵也敢说支持活动性（**是**）。 / Whether an empty matrix claims activity support (**yes**).
    pub an_empty_matrix_claims_activity_support: bool,
    /// 固定整数灵敏度是三态阶梯（**是**）。 / Whether the ladder is three-tiered (**yes**).
    pub the_fixed_integer_ladder_is_three_tiered: bool,
    /// 扰动三兄弟不看 dual（**是**）。 / Whether the trio ignores the dual (**yes**).
    pub the_perturbation_trio_ignores_the_dual: bool,
    /// 自适应扰动是 RHS 扰动的恒等映射（**是**）。 / Whether adaptive mirrors RHS exactly (**yes**).
    pub adaptive_mirrors_rhs_perturbation_exactly: bool,
    /// 只有两个能力接受 fallback（**是**）。 / Whether only two capabilities accept the fallback (**yes**).
    pub only_two_capabilities_accept_the_fallback: bool,
    /// Mus 以 Conflict 为前置（**是**）。 / Whether `Mus` is gated on `Conflict` (**yes**).
    pub mus_is_gated_on_conflict: bool,
    /// 覆盖项不提升"原生"（**是**）。 / Whether an override never upgrades the native flag (**yes**).
    pub an_override_never_upgrades_the_native_flag: bool,
    /// `Conditional` 也算可用（**是**）。 / Whether `Conditional` counts as available (**yes**).
    pub conditional_counts_as_available: bool,
}

/// 十三个能力的稳定顺序。 / The thirteen capabilities in stable order.
const CAPABILITIES: [(AnalysisCapability, &str); 13] = [
    (AnalysisCapability::Activity, "activity"),
    (AnalysisCapability::FixedIntegerLpSensitivity, "fixed_integer_lp_sensitivity"),
    (AnalysisCapability::RhsPerturbation, "rhs_perturbation"),
    (AnalysisCapability::AdaptivePerturbation, "adaptive_perturbation"),
    (AnalysisCapability::RemovalTest, "removal_test"),
    (AnalysisCapability::ObjectiveTarget, "objective_target"),
    (AnalysisCapability::AssumptionSolving, "assumption_solving"),
    (AnalysisCapability::NativeUnsatCore, "native_unsat_core"),
    (AnalysisCapability::Conflict, "conflict"),
    (AnalysisCapability::Mus, "mus"),
    (AnalysisCapability::ConstraintGroupAggregation, "constraint_group_aggregation"),
    (AnalysisCapability::WarmStart, "warm_start"),
    (AnalysisCapability::ExactCpLowering, "exact_cp_lowering"),
];

/// 分析能力级别名。 / An analysis-capability level name.
fn support_name(support: CapabilitySupport) -> &'static str {
    match support {
        CapabilitySupport::Supported => "Supported",
        CapabilitySupport::Conditional => "Conditional",
        CapabilitySupport::Unsupported => "Unsupported",
    }
}

/// CP 支持级别名。 / A CP-support level name.
fn cp_support_name(support: ConstraintProgrammingSupport) -> &'static str {
    match support {
        ConstraintProgrammingSupport::Native => "Native",
        ConstraintProgrammingSupport::ExactLowering => "ExactLowering",
        ConstraintProgrammingSupport::Conditional => "Conditional",
        ConstraintProgrammingSupport::Unsupported => "Unsupported",
    }
}

/// 按名字找能力。 / Look a capability up by name.
fn capability_of(name: &str) -> Result<AnalysisCapability> {
    CAPABILITIES.iter().find(|(_, stable)| *stable == name).map(|(value, _)| *value)
        .ok_or_else(|| BridgeError::new("InvalidInput", &format!(
            "capability matrix failed validation: unknown capability `{name}`")))
}

/// 按名字找能力级别。 / Look a support level up by name.
fn support_of(name: &str) -> Result<CapabilitySupport> {
    Ok(match name {
        "Supported" => CapabilitySupport::Supported,
        "Conditional" => CapabilitySupport::Conditional,
        "Unsupported" => CapabilitySupport::Unsupported,
        other => return Err(BridgeError::new("InvalidInput", &format!(
            "capability matrix failed validation: unknown support level `{other}`"))),
    })
}

/// 按名字找模型种类。 / Look a model kind up by name.
fn model_type_of(name: &str) -> Result<SolverModelType> {
    Ok(match name {
        "Linear" => SolverModelType::Linear,
        "Mip" => SolverModelType::Mip,
        "Quadratic" => SolverModelType::Quadratic,
        "Miqp" => SolverModelType::Miqp,
        "ConstraintProgramming" => SolverModelType::ConstraintProgramming,
        other => return Err(BridgeError::new("InvalidInput", &format!(
            "capability matrix failed validation: unknown model kind `{other}`"))),
    })
}

/// 从请求造一份矩阵。 / Build a matrix from the request.
fn build(request: &CapabilityMatrixRequest) -> Result<CapabilityMatrix> {
    let mut matrix = CapabilityMatrix {
        dual: request.dual.unwrap_or(false),
        warm_start: request.warm_start.unwrap_or(false),
        exact_cp_lowering: request.exact_cp_lowering.unwrap_or(false),
        native_assumption_solving: request.native_assumption_solving.unwrap_or(false),
        native_unsat_core: request.native_unsat_core.unwrap_or(false),
        fallback_satisfaction_only: request.fallback_satisfaction_only.unwrap_or(false),
        ..CapabilityMatrix::default()
    };
    for name in &request.model_types {
        matrix.model_types.insert(model_type_of(name)?);
    }
    for entry in &request.overrides {
        let (capability, support) = entry.split_once('=').ok_or_else(|| BridgeError::new(
            "InvalidInput", "capability matrix failed validation: an override must read `name=Level`"))?;
        matrix = matrix.with_capability(capability_of(capability.trim())?, support_of(support.trim())?);
    }
    Ok(matrix)
}

/// 读一份矩阵。 / Read one matrix.
fn rows_of(matrix: &CapabilityMatrix) -> Vec<CapabilityRow> {
    CAPABILITIES.iter().map(|(capability, stable)| CapabilityRow {
        capability: (*stable).to_owned(), support: support_name(matrix.support(*capability)).to_owned(),
        available: matrix.supports(*capability), native: matrix.is_native(*capability) }).collect()
}

/// 走一遍能力矩阵探针。非法输入是**错误**。 / Walk the capability-matrix probe.
pub fn verify_capability_matrix(request: &CapabilityMatrixRequest)
    -> Result<CapabilityMatrixReport>
{
    if request.model_types.len() > 8 || request.overrides.len() > 16 {
        return Err(BridgeError::new("InvalidInput",
            "capability matrix failed validation: at most 8 model kinds and 16 overrides are accepted"));
    }
    let matrix = build(request)?;
    let empty = CapabilityMatrix::default();
    let findings = canonical_findings();
    Ok(CapabilityMatrixReport {
        rows: rows_of(&matrix), lp_dual: matrix.lp_dual(),
        cp_assumption: cp_support_name(
            matrix.cp_support(ConstraintProgrammingFeature::Assumption)).to_owned(),
        cp_boolean_logic: cp_support_name(
            matrix.cp_support(ConstraintProgrammingFeature::BooleanLogic)).to_owned(),
        empty_activity: support_name(empty.support(AnalysisCapability::Activity)).to_owned(),
        empty_rhs_perturbation:
            support_name(empty.support(AnalysisCapability::RhsPerturbation)).to_owned(),
        empty_native_unsat_core:
            support_name(empty.support(AnalysisCapability::NativeUnsatCore)).to_owned(),
        empty_mus: support_name(empty.support(AnalysisCapability::Mus)).to_owned(),
        an_empty_matrix_claims_activity_support:
            findings.an_empty_matrix_claims_activity_support,
        the_fixed_integer_ladder_is_three_tiered:
            findings.the_fixed_integer_ladder_is_three_tiered,
        the_perturbation_trio_ignores_the_dual:
            findings.the_perturbation_trio_ignores_the_dual,
        adaptive_mirrors_rhs_perturbation_exactly:
            findings.adaptive_mirrors_rhs_perturbation_exactly,
        only_two_capabilities_accept_the_fallback:
            findings.only_two_capabilities_accept_the_fallback,
        mus_is_gated_on_conflict: findings.mus_is_gated_on_conflict,
        an_override_never_upgrades_the_native_flag:
            findings.an_override_never_upgrades_the_native_flag,
        conditional_counts_as_available: findings.conditional_counts_as_available })
}

/// 八条规则属性的内部固定探针。 / The eight rule properties measured by fixed internal probes.
struct Findings {
    an_empty_matrix_claims_activity_support: bool,
    the_fixed_integer_ladder_is_three_tiered: bool,
    the_perturbation_trio_ignores_the_dual: bool,
    adaptive_mirrors_rhs_perturbation_exactly: bool,
    only_two_capabilities_accept_the_fallback: bool,
    mus_is_gated_on_conflict: bool,
    an_override_never_upgrades_the_native_flag: bool,
    conditional_counts_as_available: bool,
}

/// 用固定探针测出全部规则属性——不读调用方的请求值。
/// Measure every rule property with fixed probes; the caller's request values are not read.
fn canonical_findings() -> Findings {
    // 1) 空矩阵不是"什么都不支持"。 / An empty matrix does not mean "nothing is supported".
    let an_empty_matrix_claims_activity_support = {
        let empty = CapabilityMatrix::default();
        empty.support(AnalysisCapability::Activity) == CapabilitySupport::Supported
            && empty.support(AnalysisCapability::ConstraintGroupAggregation)
                == CapabilitySupport::Supported
            && !empty.supports(AnalysisCapability::FixedIntegerLpSensitivity)
            && empty.support(AnalysisCapability::RhsPerturbation)
                == CapabilitySupport::Conditional
            && empty.support(AnalysisCapability::RemovalTest) == CapabilitySupport::Conditional
            && empty.support(AnalysisCapability::ObjectiveTarget) == CapabilitySupport::Conditional
            && empty.support(AnalysisCapability::NativeUnsatCore)
                == CapabilitySupport::Unsupported
    };
    // 2) 固定整数灵敏度是三态阶梯。 / The fixed-integer ladder is three-tiered.
    let the_fixed_integer_ladder_is_three_tiered = {
        let dual_only = CapabilityMatrix { dual: true, ..CapabilityMatrix::default() };
        let with_mip = CapabilityMatrix { dual: true, ..CapabilityMatrix::default() }
            .with_model_type(SolverModelType::Mip);
        let with_lowering = CapabilityMatrix { dual: true, exact_cp_lowering: true,
            ..CapabilityMatrix::default() };
        let no_dual = CapabilityMatrix { ..CapabilityMatrix::default() }
            .with_model_type(SolverModelType::Mip);
        dual_only.support(AnalysisCapability::FixedIntegerLpSensitivity)
            == CapabilitySupport::Conditional
            && with_mip.support(AnalysisCapability::FixedIntegerLpSensitivity)
                == CapabilitySupport::Supported
            && with_lowering.support(AnalysisCapability::FixedIntegerLpSensitivity)
                == CapabilitySupport::Supported
            && no_dual.support(AnalysisCapability::FixedIntegerLpSensitivity)
                == CapabilitySupport::Unsupported
    };
    // 3) 扰动三兄弟不看 `dual`。 / The perturbation trio ignores the dual.
    let the_perturbation_trio_ignores_the_dual = {
        let bare = CapabilityMatrix::default();
        let dual = CapabilityMatrix { dual: true, ..CapabilityMatrix::default() };
        let typed = CapabilityMatrix::default().with_model_type(SolverModelType::Linear);
        let typed_dual = CapabilityMatrix { dual: true, ..CapabilityMatrix::default() }
            .with_model_type(SolverModelType::Linear);
        let trio = [AnalysisCapability::RhsPerturbation, AnalysisCapability::RemovalTest,
            AnalysisCapability::ObjectiveTarget];
        // 同一 `model_types` 下，`dual` 开关不改变三者的结论。 
        // With the same `model_types`, toggling `dual` changes none of the three.
        trio.iter().all(|capability| bare.support(*capability) == dual.support(*capability)
            && typed.support(*capability) == typed_dual.support(*capability))
            && typed.support(AnalysisCapability::RhsPerturbation)
                == CapabilitySupport::Supported
    };
    // 4) 自适应扰动是 RHS 扰动的恒等映射。 / Adaptive is an identity map onto RHS.
    let adaptive_mirrors_rhs_perturbation_exactly = {
        let cases = [CapabilityMatrix::default(),
            CapabilityMatrix::default().with_model_type(SolverModelType::Mip),
            CapabilityMatrix { dual: true, ..CapabilityMatrix::default() }];
        cases.iter().all(|matrix| matrix.support(AnalysisCapability::AdaptivePerturbation)
            == matrix.support(AnalysisCapability::RhsPerturbation))
    };
    // 5) 只有两个能力接受 fallback。 / Only two capabilities accept the fallback.
    let only_two_capabilities_accept_the_fallback = {
        let fallback = CapabilityMatrix { fallback_satisfaction_only: true,
            ..CapabilityMatrix::default() };
        let native = CapabilityMatrix { native_assumption_solving: true, native_unsat_core: true,
            ..CapabilityMatrix::default() };
        fallback.support(AnalysisCapability::AssumptionSolving) == CapabilitySupport::Conditional
            && fallback.support(AnalysisCapability::Conflict)
                == CapabilitySupport::Conditional
            && fallback.support(AnalysisCapability::NativeUnsatCore)
                == CapabilitySupport::Unsupported
            && native.support(AnalysisCapability::AssumptionSolving)
                == CapabilitySupport::Supported
            && native.support(AnalysisCapability::Conflict) == CapabilitySupport::Supported
            && native.support(AnalysisCapability::NativeUnsatCore)
                == CapabilitySupport::Supported
    };
    // 6) `Mus` 以 `Conflict` 为前置。 / `Mus` is gated on `Conflict`.
    let mus_is_gated_on_conflict = {
        let blind = CapabilityMatrix::default().with_model_type(SolverModelType::Linear);
        let conflict_only = CapabilityMatrix { fallback_satisfaction_only: true,
            ..CapabilityMatrix::default() };
        let conflict_typed = CapabilityMatrix { fallback_satisfaction_only: true,
            ..CapabilityMatrix::default() }.with_model_type(SolverModelType::Mip);
        // 有 `model_types` 但 `Conflict` 不可用 → `Mus` 仍 `Unsupported`。 
        // With `model_types` present but `Conflict` unavailable, `Mus` stays `Unsupported`.
        blind.support(AnalysisCapability::Conflict) == CapabilitySupport::Unsupported
            && blind.support(AnalysisCapability::Mus) == CapabilitySupport::Unsupported
            && conflict_only.support(AnalysisCapability::Mus) == CapabilitySupport::Conditional
            && conflict_typed.support(AnalysisCapability::Mus) == CapabilitySupport::Supported
    };
    // 7) 覆盖项只改查询、不改"原生"。 / An override changes the query but never the "native" flag.
    let an_override_never_upgrades_the_native_flag = {
        let overridden = CapabilityMatrix { warm_start: true, ..CapabilityMatrix::default() }
            .with_capability(AnalysisCapability::ExactCpLowering,
                CapabilitySupport::Supported)
            .with_capability(AnalysisCapability::NativeUnsatCore, CapabilitySupport::Supported);
        // `ExactCpLowering` 的布尔字段仍是假，所以覆盖成 `Supported` 只让 `supports()` 为真。 
        // The boolean stays false, so the override moves `supports()` only.
        overridden.support(AnalysisCapability::ExactCpLowering) == CapabilitySupport::Supported
            && overridden.supports(AnalysisCapability::ExactCpLowering)
            && !overridden.is_native(AnalysisCapability::ExactCpLowering)
            && !overridden.is_native(AnalysisCapability::NativeUnsatCore)
            && overridden.is_native(AnalysisCapability::WarmStart)
            // 覆盖成 `Unsupported` 也照样能压掉推断出来的 `Supported`。 
            // An `Unsupported` override also beats an inferred `Supported`.
            && CapabilityMatrix::default()
                .with_capability(AnalysisCapability::Activity, CapabilitySupport::Unsupported)
                .support(AnalysisCapability::Activity) == CapabilitySupport::Unsupported
    };
    // 8) `Conditional` 也算可用。 / `Conditional` counts as available.
    let conditional_counts_as_available = {
        let empty = CapabilityMatrix::default();
        empty.support(AnalysisCapability::RhsPerturbation) == CapabilitySupport::Conditional
            && empty.supports(AnalysisCapability::RhsPerturbation)
            && !empty.supports(AnalysisCapability::NativeUnsatCore)
            // CP 侧同理：`Conditional` 算可用，`Unsupported` 不算。 
            // The same holds for CP features: `Conditional` counts, `Unsupported` does not.
            && CapabilityMatrix::default()
                .with_cp_feature(ConstraintProgrammingFeature::Assumption,
                    ConstraintProgrammingSupport::Conditional)
                .supports_cp(ConstraintProgrammingFeature::Assumption)
            && !empty.supports_cp(ConstraintProgrammingFeature::Assumption)
    };
    Findings { an_empty_matrix_claims_activity_support, the_fixed_integer_ladder_is_three_tiered,
        the_perturbation_trio_ignores_the_dual, adaptive_mirrors_rhs_perturbation_exactly,
        only_two_capabilities_accept_the_fallback, mus_is_gated_on_conflict,
        an_override_never_upgrades_the_native_flag, conditional_counts_as_available }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> CapabilityMatrixRequest {
        CapabilityMatrixRequest { model_types: vec!["Mip".to_owned()],
            dual: Some(true), warm_start: None, exact_cp_lowering: None,
            native_assumption_solving: None, native_unsat_core: None,
            fallback_satisfaction_only: None, overrides: Vec::new() }
    }

    #[test]
    fn the_matrix_reports_all_thirteen_capabilities() {
        let report = verify_capability_matrix(&request()).unwrap();
        assert_eq!(report.rows.len(), 13);
        assert_eq!(report.rows.iter().map(|row| row.capability.clone()).collect::<Vec<_>>(),
            CAPABILITIES.iter().map(|(_, stable)| (*stable).to_owned()).collect::<Vec<_>>());
        let row = |name: &str| report.rows.iter().find(|row| row.capability == name).unwrap();
        assert_eq!(row("activity").support, "Supported");
        assert_eq!(row("fixed_integer_lp_sensitivity").support, "Supported");
        assert_eq!(row("rhs_perturbation").support, "Supported");
        assert_eq!(row("adaptive_perturbation").support, "Supported");
        assert_eq!(row("native_unsat_core").support, "Unsupported");
        // `native_unsat_core` 为假 → `Conflict` 不可用 → `Mus` 一定不可用。 
        // `native_unsat_core` is false, so `Conflict` is unavailable and `Mus` must be too.
        assert_eq!(row("conflict").support, "Unsupported");
        assert_eq!(row("mus").support, "Unsupported");
        assert!(report.lp_dual);
        assert_eq!(report.cp_assumption, "Unsupported");
        assert_eq!(report.empty_activity, "Supported");
        assert_eq!(report.empty_rhs_perturbation, "Conditional");
        assert_eq!(report.empty_native_unsat_core, "Unsupported");
        assert_eq!(report.empty_mus, "Unsupported");
        assert!(report.an_empty_matrix_claims_activity_support);
        assert!(report.the_fixed_integer_ladder_is_three_tiered);
        assert!(report.the_perturbation_trio_ignores_the_dual);
        assert!(report.adaptive_mirrors_rhs_perturbation_exactly);
        assert!(report.only_two_capabilities_accept_the_fallback);
        assert!(report.mus_is_gated_on_conflict);
        assert!(report.an_override_never_upgrades_the_native_flag);
        assert!(report.conditional_counts_as_available);
    }

    #[test]
    fn the_three_tier_ladder_and_the_fallback_tier_are_visible() {
        let report = verify_capability_matrix(&CapabilityMatrixRequest { dual: Some(true),
            model_types: Vec::new(), ..request() }).unwrap();
        assert_eq!(report.rows.iter().find(|row| row.capability == "fixed_integer_lp_sensitivity")
            .unwrap().support, "Conditional");
        let report = verify_capability_matrix(&CapabilityMatrixRequest {
            fallback_satisfaction_only: Some(true), ..request() }).unwrap();
        assert_eq!(report.rows.iter().find(|row| row.capability == "assumption_solving")
            .unwrap().support, "Conditional");
        assert_eq!(report.rows.iter().find(|row| row.capability == "conflict")
            .unwrap().support, "Conditional");
    }

    #[test]
    fn an_override_moves_the_query_but_not_the_native_flag() {
        let report = verify_capability_matrix(&CapabilityMatrixRequest {
            overrides: vec!["exact_cp_lowering=Supported".to_owned()], ..request() }).unwrap();
        let row = report.rows.iter().find(|row| row.capability == "exact_cp_lowering").unwrap();
        assert_eq!(row.support, "Supported");
        assert!(row.available);
        assert!(!row.native);
        let report = verify_capability_matrix(&CapabilityMatrixRequest {
            overrides: vec!["activity=Unsupported".to_owned()], ..request() }).unwrap();
        assert_eq!(report.rows.iter().find(|row| row.capability == "activity").unwrap().support,
            "Unsupported");
    }

    #[test]
    fn bad_input_is_refused() {
        assert!(verify_capability_matrix(&CapabilityMatrixRequest {
            model_types: vec!["Unknown".to_owned()], ..request() }).unwrap_err().message
            .contains("unknown model kind"));
        assert!(verify_capability_matrix(&CapabilityMatrixRequest {
            overrides: vec!["nope=Supported".to_owned()], ..request() }).unwrap_err().message
            .contains("unknown capability"));
        assert!(verify_capability_matrix(&CapabilityMatrixRequest {
            overrides: vec!["activity=Maybe".to_owned()], ..request() }).unwrap_err().message
            .contains("unknown support level"));
        assert!(verify_capability_matrix(&CapabilityMatrixRequest {
            overrides: vec!["activity".to_owned()], ..request() }).unwrap_err().message
            .contains("must read `name=Level`"));
        assert!(verify_capability_matrix(&CapabilityMatrixRequest {
            overrides: vec!["activity=Supported".to_owned(); 17], ..request() }).unwrap_err()
            .message.contains("at most 8 model kinds and 16 overrides"));
    }

    /// 守门：DTO 必须是 camelCase 且拒绝未知字段。 / A guard: camelCase wire, unknown fields refused.
    #[test]
    fn the_report_wire_is_camel_case_and_rejects_unknown_fields() {
        let report = verify_capability_matrix(&request()).unwrap();
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"lpDual\""), "{json}");
        assert!(json.contains("\"anOverrideNeverUpgradesTheNativeFlag\""), "{json}");
        assert!(json.contains("\"emptyMus\""), "{json}");
        assert!(!json.contains("lp_dual"), "{json}");
        let mut copy: serde_json::Value = serde_json::from_str(&json).unwrap();
        copy["unexpected"] = serde_json::json!(1);
        assert!(serde_json::from_str::<CapabilityMatrixReport>(&copy.to_string()).is_err());
        let mut copy: serde_json::Value = serde_json::from_str(&json).unwrap();
        copy["rows"][0]["unexpected"] = serde_json::json!(1);
        assert!(serde_json::from_str::<CapabilityMatrixReport>(&copy.to_string()).is_err());
    }
}
