//! 续挖第 67 单：CP 支持报告合并进能力矩阵的**推导口径**——原生标志只认 descriptor、`exact_cp_lowering`
//! 空真、逐约束等级不进矩阵（`analysis/mod.rs` 的 `from_constraint_programming_support` / `From<&SolverCapabilities>`）
//! Continuation, delivery 67: the **derivation rules** that merge a CP support report into the capability matrix — the
//! native flags come from the descriptor only, `exact_cp_lowering` is vacuously true, and per-constraint levels never enter
//! the matrix (`from_constraint_programming_support` / `From<&SolverCapabilities>` in `analysis/mod.rs`).
//!
//! 第 66 单量的是**推断格**，这一单量它的**入口**：一张 `ConstraintProgrammingSupportReport` 加一个
//! `SolverDescriptor` 怎样变成矩阵。整段仍不含求解器调用。七条实测：
//!
//! 1. **原生假设只认 descriptor**（`native_assumption_solving_comes_from_the_descriptor_only()`）：
//!    `support.assumptions = true` 不会让 `native_assumption_solving` 变真——源码注释明说 facade 能力不等于
//!    backend 原生 API。
//! 2. **`fallback_satisfaction_only` 就是 `support.satisfaction`**（`the_fallback_flag_is_the_satisfaction_flag()`）：
//!    于是"能求解 satisfaction"立刻把 `assumption_solving` 与 `conflict` 推成 `Conditional`。
//! 3. **`exact_cp_lowering` 是空真**（`exact_lowering_is_vacuously_true()`）：`satisfaction && integer_objective`
//!    且约束表**为空**时，`all()` 对空迭代器给真 → 整份 lowering 为真。
//! 4. **一条不是 `ExactLowering` 就整份假**（`one_inexact_constraint_kills_the_whole_flag()`）：
//!    逐个 `all()`，没有"部分 lower 也算"的余地。
//! 5. **五个特征恒非 `Native`**（`five_features_are_never_native()`）：`BooleanLogic` / `SparseDomain` /
//!    `Assumption` / `SolutionHint` / `ConflictCore` 只会是 `Conditional` 或 `Unsupported`；只有
//!    `IncrementalSolve` 用 `Native`。
//! 6. **逐约束等级被丢掉**（`per_constraint_levels_never_enter_the_matrix()`）：`constraints` 表不进
//!    `constraint_programming_features`，只在算 `exact_cp_lowering` 时被读一次。
//! 7. **`model_types` 无条件插 `ConstraintProgramming`**（`the_cp_model_type_is_inserted_unconditionally()`）：
//!    descriptor 完全没声明 CP，合并后仍出现该模型种类。
//! 8. **三个能力名被静默忽略**（`three_solver_capabilities_are_silently_ignored()`）：`socp` /
//!    `native_indicator` / `native_sos1` 在 `From<&SolverCapabilities>` 里落进 `_ => {}`，一个都不进矩阵。
//!
//! Delivery 66 measured the **inference lattice**; this one measures its **entrance**: how a CP support report plus a
//! descriptor become a matrix. **Native assumption comes from the descriptor only.** **The fallback flag *is* the
//! satisfaction flag.** **`exact_cp_lowering` is vacuously true** — `all()` over an empty constraint map is true. **One
//! inexact constraint kills the whole flag.** **Five features are never `Native`**; only `IncrementalSolve` uses it.
//! **Per-constraint levels never enter the matrix.** **The CP model type is inserted unconditionally.** And **three solver
//! capability names are silently ignored** by `From<&SolverCapabilities>`.

use std::collections::BTreeMap;

use ospf_rust_core::analysis::{
    AnalysisCapability, CapabilityMatrix, ConstraintProgrammingFeature,
};
use ospf_rust_core::solver::{
    CapabilitySupport, ConstraintProgrammingSupport, ConstraintProgrammingSupportReport,
    SolverCapability, SolverCapabilities, SolverDescriptor, StableConstraintId,
};
use ospf_rust_core::SolverModelType;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 一条约束的支持等级规格。 / One constraint's support level.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CpConstraintLevelSpec {
    /// 稳定约束 ID。 / The stable constraint ID.
    pub id: String,
    /// 等级名（`Native` / `ExactLowering` / `Conditional` / `Unsupported`）。 / The level name.
    pub level: String,
}

/// CP 支持报告合并探针请求。 / The CP-support merge probe request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CpSupportMergeRequest {
    #[serde(default)]
    pub constraint_levels: Vec<CpConstraintLevelSpec>,
    #[serde(default)]
    pub satisfaction: Option<bool>,
    #[serde(default)]
    pub integer_objective: Option<bool>,
    #[serde(default)]
    pub sparse_domain: Option<bool>,
    #[serde(default)]
    pub incremental_session: Option<bool>,
    #[serde(default)]
    pub assumptions: Option<bool>,
    #[serde(default)]
    pub solution_hint: Option<bool>,
    #[serde(default)]
    pub verified_conflict_seed: Option<bool>,
    /// descriptor 声明的能力名（`linear` / `mip` / `dual` / `assumption` / `unsat_core` / `socp` 等）。
    /// Capability names the descriptor declares.
    #[serde(default)]
    pub descriptor_capabilities: Vec<String>,
}

/// 一条特征读数。 / One feature reading.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CpFeatureRow {
    pub feature: String,
    pub support: String,
    pub available: bool,
}

/// CP 支持报告合并探针结论。 / The CP-support merge probe verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CpSupportMergeReport {
    /// 十七个 CP 特征的词汇表（`stable_name`）。 / The seventeen-feature vocabulary.
    pub feature_vocabulary: Vec<String>,
    /// 六个被合并的特征读数。 / The six merged features.
    pub features: Vec<CpFeatureRow>,
    /// 矩阵的模型种类名。 / The matrix's model type names.
    pub model_types: Vec<String>,
    pub native_assumption_solving: bool,
    pub native_unsat_core: bool,
    pub fallback_satisfaction_only: bool,
    pub exact_cp_lowering: bool,
    pub dual: bool,
    /// 合并后几个关键能力的级别。 / Key capability levels after the merge.
    pub assumption_solving: String,
    pub conflict: String,
    pub mus: String,
    pub fixed_integer_lp_sensitivity: String,
    pub exact_cp_lowering_capability: String,
    /// 原生假设只认 descriptor（**是**）。 / Whether native assumption comes from the descriptor only (**yes**).
    pub native_assumption_solving_comes_from_the_descriptor_only: bool,
    pub the_fallback_flag_is_the_satisfaction_flag: bool,
    pub exact_lowering_is_vacuously_true: bool,
    pub one_inexact_constraint_kills_the_whole_flag: bool,
    pub five_features_are_never_native: bool,
    pub per_constraint_levels_never_enter_the_matrix: bool,
    pub the_cp_model_type_is_inserted_unconditionally: bool,
    pub three_solver_capabilities_are_silently_ignored: bool,
}

/// 十七个 CP 特征的稳定名。 / The seventeen CP features' stable names.
const FEATURES: [(ConstraintProgrammingFeature, &str); 17] = [
    (ConstraintProgrammingFeature::BooleanLogic, "boolean_logic"),
    (ConstraintProgrammingFeature::Reification, "reification"),
    (ConstraintProgrammingFeature::SparseDomain, "sparse_domain"),
    (ConstraintProgrammingFeature::AllDifferent, "all_different"),
    (ConstraintProgrammingFeature::Element, "element"),
    (ConstraintProgrammingFeature::Table, "table"),
    (ConstraintProgrammingFeature::Interval, "interval"),
    (ConstraintProgrammingFeature::OptionalInterval, "optional_interval"),
    (ConstraintProgrammingFeature::NoOverlap, "no_overlap"),
    (ConstraintProgrammingFeature::Cumulative, "cumulative"),
    (ConstraintProgrammingFeature::Circuit, "circuit"),
    (ConstraintProgrammingFeature::Automaton, "automaton"),
    (ConstraintProgrammingFeature::Reservoir, "reservoir"),
    (ConstraintProgrammingFeature::Assumption, "assumption"),
    (ConstraintProgrammingFeature::ConflictCore, "conflict_core"),
    (ConstraintProgrammingFeature::SolutionHint, "solution_hint"),
    (ConstraintProgrammingFeature::IncrementalSolve, "incremental_solve"),
];

/// 六个被合并的特征。 / The six merged features.
const MERGED: [(ConstraintProgrammingFeature, &str); 6] = [
    (ConstraintProgrammingFeature::BooleanLogic, "boolean_logic"),
    (ConstraintProgrammingFeature::SparseDomain, "sparse_domain"),
    (ConstraintProgrammingFeature::Assumption, "assumption"),
    (ConstraintProgrammingFeature::SolutionHint, "solution_hint"),
    (ConstraintProgrammingFeature::ConflictCore, "conflict_core"),
    (ConstraintProgrammingFeature::IncrementalSolve, "incremental_solve"),
];

/// 分析能力级别名。 / An analysis-capability level name.
fn support_name(support: CapabilitySupport) -> &'static str {
    match support {
        CapabilitySupport::Supported => "Supported",
        CapabilitySupport::Conditional => "Conditional",
        CapabilitySupport::Unsupported => "Unsupported",
    }
}

/// CP 级别名。 / A CP level name.
fn cp_name(support: ConstraintProgrammingSupport) -> &'static str {
    match support {
        ConstraintProgrammingSupport::Native => "Native",
        ConstraintProgrammingSupport::ExactLowering => "ExactLowering",
        ConstraintProgrammingSupport::Conditional => "Conditional",
        ConstraintProgrammingSupport::Unsupported => "Unsupported",
    }
}

/// CP 级别枚举。 / The CP level enum.
fn cp_level_of(name: &str) -> Result<ConstraintProgrammingSupport> {
    Ok(match name {
        "Native" => ConstraintProgrammingSupport::Native,
        "ExactLowering" => ConstraintProgrammingSupport::ExactLowering,
        "Conditional" => ConstraintProgrammingSupport::Conditional,
        "Unsupported" => ConstraintProgrammingSupport::Unsupported,
        other => return Err(BridgeError::new("InvalidInput", &format!(
            "cp support merge failed validation: unknown level `{other}`"))),
    })
}

/// 模型种类名。 / A model type name.
fn model_type_name(model_type: SolverModelType) -> &'static str {
    match model_type {
        SolverModelType::Linear => "Linear",
        SolverModelType::Mip => "Mip",
        SolverModelType::Quadratic => "Quadratic",
        SolverModelType::Miqp => "Miqp",
        SolverModelType::ConstraintProgramming => "ConstraintProgramming",
    }
}

/// 造一份 descriptor。 / Build a descriptor.
fn descriptor_of(names: &[String]) -> Result<SolverDescriptor> {
    // descriptor 的能力名是**自由字符串**：框架对未知名字静默忽略，桥接层不额外收紧判据，
    // 否则就测不到第 8 条（`socp`/`native_indicator`/`native_sos1` 落进 `_ => {}`）。
    // Descriptor capability names are **free-form strings**: the framework silently ignores unknown ones, so the
    // bridge does not tighten the rule — otherwise finding 8 could not be measured.
    let mut capabilities = SolverCapabilities::default();
    for name in names {
        if name.trim().is_empty() {
            return Err(BridgeError::new("InvalidInput",
                "cp support merge failed validation: capability names must not be blank"));
        }
        capabilities = capabilities.with(name.clone(), CapabilitySupport::Supported);
    }
    Ok(SolverDescriptor { solver_id: "probe".to_owned(), display_name: "probe".to_owned(),
        backend_name: "probe".to_owned(), backend_version: None, runtime_available: Some(true),
        capabilities, warnings: Vec::new() })
}

/// 造一份 CP 支持报告。 / Build a CP support report.
fn report_of(request: &CpSupportMergeRequest) -> Result<ConstraintProgrammingSupportReport> {
    let mut constraints = BTreeMap::new();
    for spec in &request.constraint_levels {
        if spec.id.trim().is_empty() {
            return Err(BridgeError::new("InvalidInput",
                "cp support merge failed validation: constraint IDs must not be blank"));
        }
        if constraints.insert(StableConstraintId(spec.id.clone()), cp_level_of(&spec.level)?).is_some()
        {
            return Err(BridgeError::new("InvalidInput",
                "cp support merge failed validation: constraint IDs must be distinct"));
        }
    }
    Ok(ConstraintProgrammingSupportReport { constraints,
        satisfaction: request.satisfaction.unwrap_or(false),
        integer_objective: request.integer_objective.unwrap_or(false),
        sparse_domain: request.sparse_domain.unwrap_or(false),
        one_shot: true, rebuild_session: true,
        incremental_session: request.incremental_session.unwrap_or(false),
        assumptions: request.assumptions.unwrap_or(false), cancellation: true,
        solution_hint: request.solution_hint.unwrap_or(false),
        verified_conflict_seed: request.verified_conflict_seed.unwrap_or(false),
        irreducible_conflict: false, progress: true, deterministic: true, solution_pool: false,
        notes: Vec::new() })
}

/// 走一遍合并探针。非法输入是**错误**。 / Walk the merge probe.
pub fn verify_cp_support_merge(request: &CpSupportMergeRequest) -> Result<CpSupportMergeReport> {
    if request.constraint_levels.len() > 16 || request.descriptor_capabilities.len() > 8 {
        return Err(BridgeError::new("InvalidInput",
            "cp support merge failed validation: at most 16 constraints and 8 capabilities are accepted"));
    }
    let descriptor = descriptor_of(&request.descriptor_capabilities)?;
    let report = report_of(request)?;
    let matrix = CapabilityMatrix::from_constraint_programming_support(&descriptor, &report);
    let findings = canonical_findings();
    Ok(CpSupportMergeReport {
        feature_vocabulary: FEATURES.iter().map(|(_, name)| (*name).to_owned()).collect(),
        features: MERGED.iter().map(|(feature, name)| CpFeatureRow {
            feature: (*name).to_owned(),
            support: cp_name(matrix.cp_support(*feature)).to_owned(),
            available: matrix.supports_cp(*feature) }).collect(),
        model_types: matrix.model_types.iter().map(|value| model_type_name(*value).to_owned())
            .collect(),
        native_assumption_solving: matrix.native_assumption_solving,
        native_unsat_core: matrix.native_unsat_core,
        fallback_satisfaction_only: matrix.fallback_satisfaction_only,
        exact_cp_lowering: matrix.exact_cp_lowering, dual: matrix.lp_dual(),
        assumption_solving: support_name(matrix.support(AnalysisCapability::AssumptionSolving))
            .to_owned(),
        conflict: support_name(matrix.support(AnalysisCapability::Conflict)).to_owned(),
        mus: support_name(matrix.support(AnalysisCapability::Mus)).to_owned(),
        fixed_integer_lp_sensitivity:
            support_name(matrix.support(AnalysisCapability::FixedIntegerLpSensitivity)).to_owned(),
        exact_cp_lowering_capability:
            support_name(matrix.support(AnalysisCapability::ExactCpLowering)).to_owned(),
        native_assumption_solving_comes_from_the_descriptor_only:
            findings.native_assumption_solving_comes_from_the_descriptor_only,
        the_fallback_flag_is_the_satisfaction_flag:
            findings.the_fallback_flag_is_the_satisfaction_flag,
        exact_lowering_is_vacuously_true: findings.exact_lowering_is_vacuously_true,
        one_inexact_constraint_kills_the_whole_flag:
            findings.one_inexact_constraint_kills_the_whole_flag,
        five_features_are_never_native: findings.five_features_are_never_native,
        per_constraint_levels_never_enter_the_matrix:
            findings.per_constraint_levels_never_enter_the_matrix,
        the_cp_model_type_is_inserted_unconditionally:
            findings.the_cp_model_type_is_inserted_unconditionally,
        three_solver_capabilities_are_silently_ignored:
            findings.three_solver_capabilities_are_silently_ignored })
}

/// 八条规则属性的内部固定探针。 / The eight rule properties measured by fixed internal probes.
struct Findings {
    native_assumption_solving_comes_from_the_descriptor_only: bool,
    the_fallback_flag_is_the_satisfaction_flag: bool,
    exact_lowering_is_vacuously_true: bool,
    one_inexact_constraint_kills_the_whole_flag: bool,
    five_features_are_never_native: bool,
    per_constraint_levels_never_enter_the_matrix: bool,
    the_cp_model_type_is_inserted_unconditionally: bool,
    three_solver_capabilities_are_silently_ignored: bool,
}

/// 一份只有 CP 支持报告、descriptor 空白的报告。 / A support report with a blank descriptor.
fn bare_report(satisfaction: bool, integer_objective: bool,
    constraints: BTreeMap<StableConstraintId, ConstraintProgrammingSupport>)
    -> ConstraintProgrammingSupportReport
{
    ConstraintProgrammingSupportReport { constraints, satisfaction, integer_objective,
        sparse_domain: true, one_shot: true, rebuild_session: true, incremental_session: false,
        assumptions: true, cancellation: true, solution_hint: true, verified_conflict_seed: true,
        irreducible_conflict: false, progress: true, deterministic: true, solution_pool: false,
        notes: Vec::new() }
}

/// 用固定探针测出全部规则属性——不读调用方的请求值。
/// Measure every rule property with fixed probes; the caller's request values are not read.
fn canonical_findings() -> Findings {
    let blind = SolverDescriptor::unknown("blind", &[]);
    // 1) 原生假设只认 descriptor。 / Native assumption comes from the descriptor only.
    let native_assumption_solving_comes_from_the_descriptor_only = {
        let report = bare_report(true, true, BTreeMap::new());
        let without = CapabilityMatrix::from_constraint_programming_support(&blind, &report);
        let declared = SolverDescriptor::unknown("declared",
            &[SolverCapability::ConstraintProgramming])
            .capabilities.with("native_assumption_solving", CapabilitySupport::Supported);
        let with = CapabilityMatrix::from_constraint_programming_support(
            &SolverDescriptor { capabilities: declared, ..blind.clone() }, &report);
        // `support.assumptions` 为真也不给原生标志；只有 descriptor 里写了才有。 
        // `support.assumptions` being true grants no native flag; only the descriptor does.
        report.assumptions && !without.native_assumption_solving
            && !without.is_native(AnalysisCapability::AssumptionSolving)
            && with.native_assumption_solving && with.is_native(AnalysisCapability::AssumptionSolving)
    };
    // 2) fallback 就是 satisfaction。 / The fallback flag is the satisfaction flag.
    let the_fallback_flag_is_the_satisfaction_flag = {
        let capable = CapabilityMatrix::from_constraint_programming_support(&blind,
            &bare_report(true, true, BTreeMap::new()));
        let incapable = CapabilityMatrix::from_constraint_programming_support(&blind,
            &bare_report(false, false, BTreeMap::new()));
        capable.fallback_satisfaction_only && !incapable.fallback_satisfaction_only
            && capable.support(AnalysisCapability::AssumptionSolving)
                == CapabilitySupport::Conditional
            && capable.support(AnalysisCapability::Conflict) == CapabilitySupport::Conditional
            && incapable.support(AnalysisCapability::Conflict) == CapabilitySupport::Unsupported
    };
    // 3) `exact_cp_lowering` 空真。 / `exact_cp_lowering` is vacuously true.
    let exact_lowering_is_vacuously_true = {
        let empty = CapabilityMatrix::from_constraint_programming_support(&blind,
            &bare_report(true, true, BTreeMap::new()));
        let not_objective = CapabilityMatrix::from_constraint_programming_support(&blind,
            &bare_report(true, false, BTreeMap::new()));
        // 空约束表 + 两个布尔 → 空真为真；少一个布尔则假。 
        // Empty constraints plus both booleans is vacuously true; missing either boolean is false.
        empty.exact_cp_lowering && !not_objective.exact_cp_lowering
            && empty.support(AnalysisCapability::ExactCpLowering) == CapabilitySupport::Supported
    };
    // 4) 一条不精确就整份假。 / One inexact constraint kills the whole flag.
    let one_inexact_constraint_kills_the_whole_flag = {
        let all_exact = BTreeMap::from([(StableConstraintId("a".to_owned()),
            ConstraintProgrammingSupport::ExactLowering),
            (StableConstraintId("b".to_owned()),
                ConstraintProgrammingSupport::ExactLowering)]);
        let one_off = BTreeMap::from([(StableConstraintId("a".to_owned()),
            ConstraintProgrammingSupport::ExactLowering),
            (StableConstraintId("b".to_owned()),
                ConstraintProgrammingSupport::Conditional)]);
        let clean = CapabilityMatrix::from_constraint_programming_support(&blind,
            &bare_report(true, true, all_exact));
        let dirty = CapabilityMatrix::from_constraint_programming_support(&blind,
            &bare_report(true, true, one_off));
        clean.exact_cp_lowering && !dirty.exact_cp_lowering
    };
    // 5) 五个特征恒非 `Native`。 / Five features are never `Native`.
    let five_features_are_never_native = {
        let everything = CapabilityMatrix::from_constraint_programming_support(&blind,
            &bare_report(true, true, BTreeMap::new()));
        let on = [ConstraintProgrammingFeature::BooleanLogic,
            ConstraintProgrammingFeature::SparseDomain, ConstraintProgrammingFeature::Assumption,
            ConstraintProgrammingFeature::SolutionHint,
            ConstraintProgrammingFeature::ConflictCore];
        // "关掉"的那份要把五个布尔**全部**置假，否则 `sparse_domain` 之类仍是 `Conditional`。 
        // The "off" report must turn **all five** booleans off; otherwise `sparse_domain` and friends stay
        // `Conditional` and the probe would not prove anything.
        let off = CapabilityMatrix::from_constraint_programming_support(&blind,
            &ConstraintProgrammingSupportReport {
                satisfaction: false, integer_objective: false, sparse_domain: false,
                assumptions: false, solution_hint: false, verified_conflict_seed: false,
                ..bare_report(false, false, BTreeMap::new())
            });
        on.iter().all(|feature| everything.cp_support(*feature)
            == ConstraintProgrammingSupport::Conditional
            && off.cp_support(*feature) == ConstraintProgrammingSupport::Unsupported)
            // 只有 `IncrementalSolve` 用 `Native`。 / Only `IncrementalSolve` uses `Native`.
            && {
                let report = ConstraintProgrammingSupportReport { incremental_session: true,
                    ..bare_report(true, true, BTreeMap::new()) };
                CapabilityMatrix::from_constraint_programming_support(&blind, &report)
                    .cp_support(ConstraintProgrammingFeature::IncrementalSolve)
                    == ConstraintProgrammingSupport::Native
            }
    };
    // 6) 逐约束等级不进矩阵。 / Per-constraint levels never enter the matrix.
    let per_constraint_levels_never_enter_the_matrix = {
        let native_all = CapabilityMatrix::from_constraint_programming_support(&blind,
            &bare_report(true, true, BTreeMap::from([(StableConstraintId("x".to_owned()),
                ConstraintProgrammingSupport::Native)])));
        let exact_all = CapabilityMatrix::from_constraint_programming_support(&blind,
            &bare_report(true, true, BTreeMap::from([(StableConstraintId("x".to_owned()),
                ConstraintProgrammingSupport::ExactLowering)])));
        // 两张矩阵的特征表**完全等价**（逐约束等级只影响 `exact_cp_lowering` 一个布尔）。 
        // The two feature tables are **identical**; the levels only move one boolean.
        FEATURES.iter().all(|(feature, _)| native_all.cp_support(*feature)
            == exact_all.cp_support(*feature))
            && !native_all.exact_cp_lowering && exact_all.exact_cp_lowering
    };
    // 7) `ConstraintProgramming` 模型种类无条件插入。 / The CP model type is inserted unconditionally.
    let the_cp_model_type_is_inserted_unconditionally = {
        let merged = CapabilityMatrix::from_constraint_programming_support(&blind,
            &bare_report(false, false, BTreeMap::new()));
        merged.model_types.contains(&SolverModelType::ConstraintProgramming)
            // 而纯 descriptor 路径不会插入它。 / The pure descriptor path does not insert it.
            && !CapabilityMatrix::from(&blind).model_types
                .contains(&SolverModelType::ConstraintProgramming)
    };
    // 8) 三个能力名被静默忽略。 / Three capability names are silently ignored.
    let three_solver_capabilities_are_silently_ignored = {
        let descriptors = SolverCapabilities::default()
            .with("socp", CapabilitySupport::Supported)
            .with("native_indicator", CapabilitySupport::Supported)
            .with("native_sos1", CapabilitySupport::Supported);
        let ignored = CapabilityMatrix::from(&descriptors);
        let kept = CapabilityMatrix::from(&SolverCapabilities::default()
            .with("mip", CapabilitySupport::Supported));
        // 三个名字一个都不进矩阵；而 `mip` 会进。 
        // None of the three enters the matrix, while `mip` does.
        ignored == CapabilityMatrix::default()
            && kept.model_types.contains(&SolverModelType::Mip)
    };
    Findings { native_assumption_solving_comes_from_the_descriptor_only,
        the_fallback_flag_is_the_satisfaction_flag, exact_lowering_is_vacuously_true,
        one_inexact_constraint_kills_the_whole_flag, five_features_are_never_native,
        per_constraint_levels_never_enter_the_matrix,
        the_cp_model_type_is_inserted_unconditionally, three_solver_capabilities_are_silently_ignored }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> CpSupportMergeRequest {
        CpSupportMergeRequest { constraint_levels: vec![CpConstraintLevelSpec {
            id: "c0".to_owned(), level: "ExactLowering".to_owned() }],
            satisfaction: Some(true), integer_objective: Some(true), sparse_domain: Some(true),
            incremental_session: None, assumptions: Some(true), solution_hint: Some(true),
            verified_conflict_seed: Some(true),
            descriptor_capabilities: vec!["constraint_programming".to_owned(),
                "native_assumption_solving".to_owned()] }
    }

    #[test]
    fn the_merge_reports_the_vocabulary_and_the_matrix() {
        let report = verify_cp_support_merge(&request()).unwrap();
        assert_eq!(report.feature_vocabulary.len(), 17);
        assert_eq!(report.feature_vocabulary[0], "boolean_logic");
        assert_eq!(report.feature_vocabulary[16], "incremental_solve");
        assert_eq!(report.features.len(), 6);
        // 前五个是 `Conditional`；`incremental_session` 缺省为假 → `incremental_solve` 是 `Unsupported`。 
        // The first five are `Conditional`; `incremental_session` defaults false, so `incremental_solve` is
        // `Unsupported` until it is switched on.
        assert!(report.features.iter().take(5).all(|row| row.support == "Conditional"));
        assert_eq!(report.features[5].feature, "incremental_solve");
        assert_eq!(report.features[5].support, "Unsupported");
        assert_eq!(report.model_types, vec!["ConstraintProgramming".to_owned()]);
        assert!(report.native_assumption_solving);
        assert!(!report.native_unsat_core);
        assert!(report.fallback_satisfaction_only);
        assert!(report.exact_cp_lowering);
        assert_eq!(report.assumption_solving, "Supported");
        assert_eq!(report.conflict, "Conditional");
        // `Conflict` 只是 `Conditional`，但无条件插入的 `ConstraintProgramming` 让 `model_types` 非空，
        // 于是 `Mus` 直接是 `Supported`——这正是"插入模型种类"那条规则的连带效果。
        // `Conflict` is only `Conditional`, yet the unconditionally inserted `ConstraintProgramming` model type
        // makes `model_types` non-empty, so `Mus` lands on `Supported` — a side effect of finding 7.
        assert_eq!(report.mus, "Supported");
        assert_eq!(report.exact_cp_lowering_capability, "Supported");
        assert!(report.native_assumption_solving_comes_from_the_descriptor_only);
        assert!(report.the_fallback_flag_is_the_satisfaction_flag);
        assert!(report.exact_lowering_is_vacuously_true);
        assert!(report.one_inexact_constraint_kills_the_whole_flag);
        assert!(report.five_features_are_never_native);
        assert!(report.per_constraint_levels_never_enter_the_matrix);
        assert!(report.the_cp_model_type_is_inserted_unconditionally);
        assert!(report.three_solver_capabilities_are_silently_ignored);
    }

    #[test]
    fn the_descriptor_decides_the_native_flags_and_the_report_decides_the_fallback() {
        let blind = verify_cp_support_merge(&CpSupportMergeRequest { descriptor_capabilities:
            Vec::new(), ..request() }).unwrap();
        assert!(!blind.native_assumption_solving);
        assert_eq!(blind.assumption_solving, "Conditional");
        let quiet = verify_cp_support_merge(&CpSupportMergeRequest { satisfaction: Some(false),
            ..request() }).unwrap();
        assert!(!quiet.fallback_satisfaction_only);
        assert_eq!(quiet.conflict, "Unsupported");
        assert_eq!(quiet.mus, "Unsupported");
        let core = verify_cp_support_merge(&CpSupportMergeRequest { descriptor_capabilities:
            vec!["unsat_core".to_owned()], ..request() }).unwrap();
        assert!(core.native_unsat_core);
        assert_eq!(core.conflict, "Supported");
        assert_eq!(core.mus, "Supported");
    }

    #[test]
    fn an_inexact_constraint_drops_the_whole_lowering_flag() {
        let inexact = verify_cp_support_merge(&CpSupportMergeRequest { constraint_levels: vec![
            CpConstraintLevelSpec { id: "a".to_owned(), level: "ExactLowering".to_owned() },
            CpConstraintLevelSpec { id: "b".to_owned(), level: "Conditional".to_owned() }],
            ..request() }).unwrap();
        assert!(!inexact.exact_cp_lowering);
        assert_eq!(inexact.exact_cp_lowering_capability, "Unsupported");
        let empty = verify_cp_support_merge(&CpSupportMergeRequest {
            constraint_levels: Vec::new(), ..request() }).unwrap();
        assert!(empty.exact_cp_lowering);
    }

    #[test]
    fn bad_input_is_refused() {
        assert!(verify_cp_support_merge(&CpSupportMergeRequest { constraint_levels: vec![
            CpConstraintLevelSpec { id: "a".to_owned(), level: "ExactLowering".to_owned() },
            CpConstraintLevelSpec { id: "a".to_owned(), level: "Native".to_owned() }],
            ..request() }).unwrap_err().message.contains("must be distinct"));
        assert!(verify_cp_support_merge(&CpSupportMergeRequest { constraint_levels: vec![
            CpConstraintLevelSpec { id: "  ".to_owned(), level: "Native".to_owned() }],
            ..request() }).unwrap_err().message.contains("must not be blank"));
        assert!(verify_cp_support_merge(&CpSupportMergeRequest { constraint_levels: vec![
            CpConstraintLevelSpec { id: "a".to_owned(), level: "Maybe".to_owned() }],
            ..request() }).unwrap_err().message.contains("unknown level"));
        // descriptor 名字自由：未知名字被静默忽略，只有空名被拒。 
        // Descriptor names are free-form: unknown ones are ignored, only blank ones are refused.
        assert!(verify_cp_support_merge(&CpSupportMergeRequest { descriptor_capabilities:
            vec!["exotic".to_owned()], ..request() }).unwrap().model_types
            == vec!["ConstraintProgramming".to_owned()]);
        assert!(verify_cp_support_merge(&CpSupportMergeRequest { descriptor_capabilities:
            vec!["  ".to_owned()], ..request() }).unwrap_err().message
            .contains("capability names must not be blank"));
        assert!(verify_cp_support_merge(&CpSupportMergeRequest { constraint_levels: (0..17)
            .map(|index| CpConstraintLevelSpec { id: format!("c{index}"),
                level: "Native".to_owned() }).collect(), ..request() }).unwrap_err().message
            .contains("at most 16 constraints"));
    }

    /// 守门：DTO 必须是 camelCase 且拒绝未知字段。 / A guard: camelCase wire, unknown fields refused.
    #[test]
    fn the_report_wire_is_camel_case_and_rejects_unknown_fields() {
        let report = verify_cp_support_merge(&request()).unwrap();
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"featureVocabulary\""), "{json}");
        assert!(json.contains("\"exactLoweringIsVacuouslyTrue\""), "{json}");
        assert!(json.contains("\"nativeUnsatCore\""), "{json}");
        assert!(!json.contains("feature_vocabulary"), "{json}");
        let mut copy: serde_json::Value = serde_json::from_str(&json).unwrap();
        copy["unexpected"] = serde_json::json!(1);
        assert!(serde_json::from_str::<CpSupportMergeReport>(&copy.to_string()).is_err());
    }
}
