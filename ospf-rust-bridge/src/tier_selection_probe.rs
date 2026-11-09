//! 续挖第 79 单：冲突提取**层级选择**——`select` 走 `support()` 而 `is_native` 绕过覆盖，所以"选中原生层"与
//! "原生标志为真"是两件事（`analysis/activation.rs` 的 `ConflictExtractionTier`）
//! Continuation, delivery 79: the conflict-extraction **tier selection** — `select` goes through `support()` while
//! `is_native` bypasses overrides, so "picked the native tier" and "the native flag is true" are two different things
//! (`ConflictExtractionTier` in `analysis/activation.rs`).
//!
//! 第 66 单量过能力矩阵的推断格；这一单量**它下游的那个选择器**。八条实测：
//!
//! 1. **顺序即优先级**（`the_order_is_the_priority()`）：`NativeUnsatCore` 先于 `AssumptionExtraction`，
//!    两者都可用时选前者。
//! 2. **只有 `Supported` 能被选中，`Conditional` 不行**（`only_supported_is_selected()`）：`select` 的判据是
//!    `is_native(..) || support(..) == Supported`。
//! 3. **只有 fallback 的后端走 `RepeatedSolving`，不走 `AssumptionExtraction`**
//!    （`a_fallback_only_backend_lands_on_repeated_solving()`）：`AssumptionSolving` 推断出 `Conditional` 时不被选中。
//! 4. **覆盖能把 `AssumptionSolving` 抬成可选中**（`an_override_can_upgrade_the_selection()`）。
//! 5. **覆盖成 `Unsupported` 能把 fallback 关掉**（`an_override_can_disable_the_fallback()`）：
//!    `RepeatedSolving` 还要求 `support(Conflict) != Unsupported`。
//! 6. **`is_native()` 与"选中原生层"会脱钩**（`selecting_the_native_tier_does_not_make_it_native()`）：
//!    把 `NativeUnsatCore` 覆盖成 `Supported` 能选中该层，但 `matrix.is_native()` 仍读布尔字段为 `false`。
//! 7. **四个词各有精确谓词**（`the_four_tiers_have_exact_predicates()`）：`is_native` 只对
//!    `NativeUnsatCore` 为真，`is_available` 只对 `Unavailable` 为假。
//! 8. **`as_str` 是四个稳定名**（`the_stable_names_are_four_words()`），与枚举序无关。
//!
//! Delivery 66 measured the capability matrix's inference lattice; this one measures **the selector downstream of it**.
//! **The order is the priority.** **Only `Supported` can be selected, never `Conditional`.** **A fallback-only backend
//! lands on `RepeatedSolving`, not `AssumptionExtraction`.** **An override can upgrade the selection.** **An override to
//! `Unsupported` can disable the fallback.** **Selecting the native tier does not make it native** — `is_native` reads the
//! booleans and bypasses overrides. **The four tiers have exact predicates.** And **`as_str` gives four stable names**.

use ospf_rust_core::analysis::{
    AnalysisCapability, CapabilityMatrix, CapabilitySupport, ConflictExtractionTier, SolverModelType,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 一条能力覆盖的声明。 / One capability-override declaration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TierOverrideSpec {
    /// 能力名（`AnalysisCapability::stable_name()`）。 / The capability name.
    pub capability: String,
    /// 支持级别：`supported` / `conditional` / `unsupported`。 / The support level.
    pub support: String,
}

/// 层级选择探针请求。 / The tier-selection probe request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TierSelectionRequest {
    /// 模型种类（`linear` / `mip` / `quadratic` / `miqp` / `constraintProgramming`）。
    /// The model kinds.
    #[serde(default)]
    pub model_types: Vec<String>,
    /// 原生 assumption 求解。 / Native assumption solving.
    #[serde(default)]
    pub native_assumption_solving: bool,
    /// 原生 unsat core。 / Native unsat core.
    #[serde(default)]
    pub native_unsat_core: bool,
    /// 允许 satisfaction-only 回退。 / Whether the satisfaction-only fallback is allowed.
    #[serde(default)]
    pub fallback_satisfaction_only: bool,
    /// LP dual 可用。 / Whether LP duals are available.
    #[serde(default)]
    pub dual: bool,
    /// 精确 CP lowering。 / Exact CP lowering.
    #[serde(default)]
    pub exact_cp_lowering: bool,
    /// 能力覆盖。 / The capability overrides.
    #[serde(default)]
    pub overrides: Vec<TierOverrideSpec>,
}

/// 层级选择探针结论。 / The tier-selection probe verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TierSelectionReport {
    /// 选中的层级名。 / The selected tier's name.
    pub selected: String,
    /// 选中层是否原生。 / Whether the selected tier is native.
    pub selected_is_native: bool,
    /// 选中层是否可用。 / Whether the selected tier is available.
    pub selected_is_available: bool,
    /// `matrix.is_native(NativeUnsatCore)` 的读数。 / `matrix.is_native(NativeUnsatCore)`.
    pub native_unsat_core_flag: bool,
    /// `support(NativeUnsatCore)` 的读数。 / `support(NativeUnsatCore)`.
    pub native_unsat_core_support: String,
    /// `support(AssumptionSolving)` 的读数。 / `support(AssumptionSolving)`.
    pub assumption_solving_support: String,
    /// `support(Conflict)` 的读数。 / `support(Conflict)`.
    pub conflict_support: String,
    /// 八条规则属性的读数。 / The eight rule readings.
    pub the_order_is_the_priority: bool,
    pub only_supported_is_selected: bool,
    pub a_fallback_only_backend_lands_on_repeated_solving: bool,
    pub an_override_can_upgrade_the_selection: bool,
    pub an_override_can_disable_the_fallback: bool,
    pub selecting_the_native_tier_does_not_make_it_native: bool,
    pub the_four_tiers_have_exact_predicates: bool,
    pub the_stable_names_are_four_words: bool,
}

/// 造一个能力矩阵。 / Build a capability matrix.
fn matrix_of(request: &TierSelectionRequest) -> Result<CapabilityMatrix> {
    let mut matrix = CapabilityMatrix::default();
    for name in &request.model_types {
        let model_type = match name.as_str() {
            "linear" => SolverModelType::Linear,
            "mip" => SolverModelType::Mip,
            "quadratic" => SolverModelType::Quadratic,
            "miqp" => SolverModelType::Miqp,
            "constraintProgramming" => SolverModelType::ConstraintProgramming,
            other => return Err(BridgeError::new("InvalidInput", &format!(
                "tier selection failed validation: unknown model kind `{other}`"))),
        };
        matrix.model_types.insert(model_type);
    }
    matrix.native_assumption_solving = request.native_assumption_solving;
    matrix.native_unsat_core = request.native_unsat_core;
    matrix.fallback_satisfaction_only = request.fallback_satisfaction_only;
    matrix.dual = request.dual;
    matrix.exact_cp_lowering = request.exact_cp_lowering;
    for entry in &request.overrides {
        let capability = capability_of(&entry.capability)?;
        matrix = matrix.with_capability(capability, support_of(&entry.support)?);
    }
    Ok(matrix)
}

/// 解析能力名。 / Parse a capability name.
fn capability_of(name: &str) -> Result<AnalysisCapability> {
    Ok(match name {
        "activity" => AnalysisCapability::Activity,
        "fixed_integer_lp_sensitivity" => AnalysisCapability::FixedIntegerLpSensitivity,
        "rhs_perturbation" => AnalysisCapability::RhsPerturbation,
        "adaptive_perturbation" => AnalysisCapability::AdaptivePerturbation,
        "removal_test" => AnalysisCapability::RemovalTest,
        "objective_target" => AnalysisCapability::ObjectiveTarget,
        "assumption_solving" => AnalysisCapability::AssumptionSolving,
        "native_unsat_core" => AnalysisCapability::NativeUnsatCore,
        "conflict" => AnalysisCapability::Conflict,
        "mus" => AnalysisCapability::Mus,
        "constraint_group_aggregation" => AnalysisCapability::ConstraintGroupAggregation,
        "warm_start" => AnalysisCapability::WarmStart,
        "exact_cp_lowering" => AnalysisCapability::ExactCpLowering,
        other => return Err(BridgeError::new("InvalidInput", &format!(
            "tier selection failed validation: unknown capability `{other}`"))),
    })
}

/// 解析支持级别。 / Parse a support level.
fn support_of(name: &str) -> Result<CapabilitySupport> {
    Ok(match name {
        "supported" => CapabilitySupport::Supported,
        "conditional" => CapabilitySupport::Conditional,
        "unsupported" => CapabilitySupport::Unsupported,
        other => return Err(BridgeError::new("InvalidInput", &format!(
            "tier selection failed validation: unknown support level `{other}`"))),
    })
}

/// 支持级别的稳定名。 / The stable support name.
fn support_name(support: CapabilitySupport) -> String {
    match support {
        CapabilitySupport::Supported => "supported",
        CapabilitySupport::Conditional => "conditional",
        CapabilitySupport::Unsupported => "unsupported",
    }.to_owned()
}

/// 内部探针用的读面。 / The readable face used by internal probes.
fn probe_report(request: &TierSelectionRequest) -> TierSelectionReport {
    let matrix = matrix_of(request).expect("probe matrix");
    let selected = ConflictExtractionTier::select(&matrix);
    TierSelectionReport {
        selected: selected.as_str().to_owned(),
        selected_is_native: selected.is_native(),
        selected_is_available: selected.is_available(),
        native_unsat_core_flag: matrix.is_native(AnalysisCapability::NativeUnsatCore),
        native_unsat_core_support: support_name(matrix.support(AnalysisCapability::NativeUnsatCore)),
        assumption_solving_support: support_name(matrix.support(AnalysisCapability::AssumptionSolving)),
        conflict_support: support_name(matrix.support(AnalysisCapability::Conflict)),
        the_order_is_the_priority: false,
        only_supported_is_selected: false,
        a_fallback_only_backend_lands_on_repeated_solving: false,
        an_override_can_upgrade_the_selection: false,
        an_override_can_disable_the_fallback: false,
        selecting_the_native_tier_does_not_make_it_native: false,
        the_four_tiers_have_exact_predicates: false,
        the_stable_names_are_four_words: false,
    }
}

/// 走一遍层级选择探针。非法输入是**错误**。 / Walk the tier-selection probe.
pub fn verify_tier_selection(request: &TierSelectionRequest) -> Result<TierSelectionReport> {
    if request.overrides.len() > 16 {
        return Err(BridgeError::new("InvalidInput",
            "tier selection failed validation: at most 16 overrides are accepted"));
    }
    let findings = canonical_findings();
    let matrix = matrix_of(request)?;
    let selected = ConflictExtractionTier::select(&matrix);
    Ok(TierSelectionReport {
        selected: selected.as_str().to_owned(),
        selected_is_native: selected.is_native(),
        selected_is_available: selected.is_available(),
        native_unsat_core_flag: matrix.is_native(AnalysisCapability::NativeUnsatCore),
        native_unsat_core_support: support_name(matrix.support(AnalysisCapability::NativeUnsatCore)),
        assumption_solving_support: support_name(matrix.support(AnalysisCapability::AssumptionSolving)),
        conflict_support: support_name(matrix.support(AnalysisCapability::Conflict)),
        the_order_is_the_priority: findings.the_order_is_the_priority,
        only_supported_is_selected: findings.only_supported_is_selected,
        a_fallback_only_backend_lands_on_repeated_solving:
            findings.a_fallback_only_backend_lands_on_repeated_solving,
        an_override_can_upgrade_the_selection: findings.an_override_can_upgrade_the_selection,
        an_override_can_disable_the_fallback: findings.an_override_can_disable_the_fallback,
        selecting_the_native_tier_does_not_make_it_native:
            findings.selecting_the_native_tier_does_not_make_it_native,
        the_four_tiers_have_exact_predicates: findings.the_four_tiers_have_exact_predicates,
        the_stable_names_are_four_words: findings.the_stable_names_are_four_words,
    })
}

/// 八条规则属性的内部固定探针。 / The eight rule properties measured by fixed internal probes.
struct Findings {
    the_order_is_the_priority: bool,
    only_supported_is_selected: bool,
    a_fallback_only_backend_lands_on_repeated_solving: bool,
    an_override_can_upgrade_the_selection: bool,
    an_override_can_disable_the_fallback: bool,
    selecting_the_native_tier_does_not_make_it_native: bool,
    the_four_tiers_have_exact_predicates: bool,
    the_stable_names_are_four_words: bool,
}

/// 造一份空请求。 / Build an empty request.
fn empty() -> TierSelectionRequest {
    TierSelectionRequest { model_types: Vec::new(), native_assumption_solving: false,
        native_unsat_core: false, fallback_satisfaction_only: false, dual: false,
        exact_cp_lowering: false, overrides: Vec::new() }
}

/// 加一条覆盖。 / Add one override.
fn with(mut request: TierSelectionRequest, capability: &str,
    support: &str) -> TierSelectionRequest {
    request.overrides.push(TierOverrideSpec { capability: capability.to_owned(),
        support: support.to_owned() });
    request
}

/// 只取选中的层级名。**内部探针专用**：直连 `select`，不经公开入口。
/// Take only the selected tier's name. **Internal probes only**: it calls `select` directly instead of going through the
/// public entry, which would recurse forever via `canonical_findings`.
fn selected(request: &TierSelectionRequest) -> String {
    let matrix = matrix_of(request).expect("probe matrix");
    ConflictExtractionTier::select(&matrix).as_str().to_owned()
}

/// 用固定探针测出全部规则属性——不读调用方的请求值。
/// Measure every rule property with fixed probes; the caller's request values are not read.
fn canonical_findings() -> Findings {
    // 1) 顺序即优先级。 / The order is the priority.
    let the_order_is_the_priority = {
        let both = TierSelectionRequest { native_unsat_core: true,
            native_assumption_solving: true, ..empty() };
        // 两个原生都在时选 unsat core。 / With both natives present the unsat core wins.
        selected(&both) == "native-unsat-core"
            && selected(&TierSelectionRequest { native_assumption_solving: true, ..empty() })
                == "assumption-extraction"
    };
    // 2) 只有 `Supported` 能被选中。 / Only `Supported` can be selected.
    let only_supported_is_selected = {
        // `Conditional` 的 assumption 求解不选。 / A conditional assumption solver is not selected.
        let conditional = TierSelectionRequest { fallback_satisfaction_only: true, ..empty() };
        let report = probe_report(&conditional);
        report.assumption_solving_support == "conditional"
            && report.selected != "assumption-extraction"
            // 覆盖成 `Supported` 才选。 / Only an override to `Supported` selects it.
            && selected(&with(conditional, "assumption_solving", "supported"))
                == "assumption-extraction"
    };
    // 3) 只有 fallback 的后端走 `RepeatedSolving`。 / A fallback-only backend lands on `RepeatedSolving`.
    let a_fallback_only_backend_lands_on_repeated_solving = {
        let fallback = TierSelectionRequest { fallback_satisfaction_only: true, ..empty() };
        let report = probe_report(&fallback);
        report.selected == "repeated-solving"
            && report.conflict_support == "conditional"
            && report.selected_is_native == false
            && report.selected_is_available == true
    };
    // 4) 覆盖能把 assumption 抬成可选中。 / An override can upgrade the selection.
    let an_override_can_upgrade_the_selection = {
        // 原本没有原生能力、也没有 fallback → `unavailable`；覆盖成 `Supported` 后选 assumption 层。 
        // No native capability and no fallback gives `unavailable`; overriding to `Supported` selects assumption tier.
        let bare = empty();
        selected(&bare) == "unavailable"
            && selected(&with(bare, "assumption_solving", "supported"))
                == "assumption-extraction"
    };
    // 5) 覆盖成 `Unsupported` 能关掉 fallback。 / An override to `Unsupported` disables the fallback.
    let an_override_can_disable_the_fallback = {
        let fallback = TierSelectionRequest { fallback_satisfaction_only: true, ..empty() };
        let disabled = with(fallback, "conflict", "unsupported");
        selected(&disabled) == "unavailable"
            && probe_report(&disabled).conflict_support == "unsupported"
    };
    // 6) 选中原生层不等于原生标志为真。 / Selecting the native tier does not make it native.
    let selecting_the_native_tier_does_not_make_it_native = {
        let overridden = with(empty(), "native_unsat_core", "supported");
        let report = probe_report(&overridden);
        // `select` 经 `support()` 看到 `supported`，于是选中该层；`is_native()` 仍读布尔字段。 
        // `select` sees `supported` through `support()` and picks the tier while `is_native()` still reads the boolean.
        report.selected == "native-unsat-core"
            && report.native_unsat_core_flag == false
            && report.native_unsat_core_support == "supported"
            && report.selected_is_native == true
    };
    // 7) 四个词各有精确谓词。 / The four tiers have exact predicates.
    let the_four_tiers_have_exact_predicates = {
        let tiers = [ConflictExtractionTier::NativeUnsatCore,
            ConflictExtractionTier::AssumptionExtraction, ConflictExtractionTier::RepeatedSolving,
            ConflictExtractionTier::Unavailable];
        // `is_native` 只对第一个为真；`is_available` 只对最后一个为假。 
        // `is_native` is true only for the first; `is_available` is false only for the last.
        tiers.iter().filter(|tier| tier.is_native()).count() == 1
            && tiers[0].is_native()
            && tiers.iter().filter(|tier| tier.is_available()).count() == 3
            && !tiers[3].is_available()
    };
    // 8) `as_str` 是四个稳定名。 / `as_str` gives four stable names.
    let the_stable_names_are_four_words = {
        let names = [ConflictExtractionTier::NativeUnsatCore,
            ConflictExtractionTier::AssumptionExtraction, ConflictExtractionTier::RepeatedSolving,
            ConflictExtractionTier::Unavailable].map(|tier| tier.as_str());
        names == ["native-unsat-core", "assumption-extraction", "repeated-solving", "unavailable"]
    };
    Findings {
        the_order_is_the_priority,
        only_supported_is_selected,
        a_fallback_only_backend_lands_on_repeated_solving,
        an_override_can_upgrade_the_selection,
        an_override_can_disable_the_fallback,
        selecting_the_native_tier_does_not_make_it_native,
        the_four_tiers_have_exact_predicates,
        the_stable_names_are_four_words,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> TierSelectionRequest {
        empty()
    }

    #[test]
    fn an_empty_matrix_selects_nothing_usable() {
        let report = verify_tier_selection(&request()).unwrap();
        assert_eq!(report.selected, "unavailable");
        assert!(!report.selected_is_native);
        assert!(!report.selected_is_available);
        assert_eq!(report.native_unsat_core_support, "unsupported");
        assert_eq!(report.assumption_solving_support, "unsupported");
        assert_eq!(report.conflict_support, "unsupported");
        assert!(report.the_order_is_the_priority);
        assert!(report.only_supported_is_selected);
        assert!(report.a_fallback_only_backend_lands_on_repeated_solving);
        assert!(report.an_override_can_upgrade_the_selection);
        assert!(report.an_override_can_disable_the_fallback);
        assert!(report.selecting_the_native_tier_does_not_make_it_native);
        assert!(report.the_four_tiers_have_exact_predicates);
        assert!(report.the_stable_names_are_four_words);
    }

    #[test]
    fn the_native_tier_wins_over_the_assumption_tier() {
        let report = verify_tier_selection(&TierSelectionRequest { native_unsat_core: true,
            native_assumption_solving: true, ..request() }).unwrap();
        assert_eq!(report.selected, "native-unsat-core");
        assert!(report.selected_is_native);
        assert!(report.native_unsat_core_flag);
        let assumption_only = verify_tier_selection(&TierSelectionRequest {
            native_assumption_solving: true, ..request() }).unwrap();
        assert_eq!(assumption_only.selected, "assumption-extraction");
        assert!(!assumption_only.selected_is_native);
    }

    #[test]
    fn a_fallback_only_backend_lands_on_repeated_solving() {
        let report = verify_tier_selection(&TierSelectionRequest {
            fallback_satisfaction_only: true, ..request() }).unwrap();
        assert_eq!(report.selected, "repeated-solving");
        assert_eq!(report.assumption_solving_support, "conditional");
        assert_eq!(report.conflict_support, "conditional");
        assert!(!report.selected_is_native);
        assert!(report.selected_is_available);
    }

    #[test]
    fn an_override_can_upgrade_or_disable_the_selection() {
        // 抬成 `Supported` → 选 assumption 层。 / Upgrading to `Supported` selects the assumption tier.
        let upgraded = verify_tier_selection(&with(request(), "assumption_solving", "supported"))
            .unwrap();
        assert_eq!(upgraded.selected, "assumption-extraction");
        // 把 `Conflict` 关掉 → fallback 也没了。 / Disabling `Conflict` removes the fallback too.
        let disabled = verify_tier_selection(&with(TierSelectionRequest {
            fallback_satisfaction_only: true, ..request() }, "conflict", "unsupported")).unwrap();
        assert_eq!(disabled.selected, "unavailable");
        assert_eq!(disabled.conflict_support, "unsupported");
    }

    #[test]
    fn selecting_the_native_tier_does_not_make_it_native() {
        let report = verify_tier_selection(&with(request(), "native_unsat_core", "supported"))
            .unwrap();
        assert_eq!(report.selected, "native-unsat-core");
        assert!(report.selected_is_native);
        // 覆盖不改布尔字段。 / The override never touches the boolean.
        assert!(!report.native_unsat_core_flag);
        assert_eq!(report.native_unsat_core_support, "supported");
    }

    #[test]
    fn the_model_kinds_do_not_move_the_selection() {
        // 模型种类只影响 `Mus` 的推断，不影响冲突提取层级。 
        // The model kinds only affect `Mus` inference and never the conflict-extraction tier.
        for kind in ["linear", "mip", "quadratic", "miqp", "constraintProgramming"] {
            let report = verify_tier_selection(&TierSelectionRequest {
                model_types: vec![kind.to_owned()], native_unsat_core: true, ..request() }).unwrap();
            assert_eq!(report.selected, "native-unsat-core", "{kind}");
        }
    }

    #[test]
    fn bad_input_is_refused() {
        assert!(verify_tier_selection(&TierSelectionRequest { model_types: vec!["nope".to_owned()],
            ..request() }).unwrap_err().message.contains("unknown model kind"));
        assert!(verify_tier_selection(&with(request(), "nope", "supported")).unwrap_err()
            .message.contains("unknown capability"));
        assert!(verify_tier_selection(&with(request(), "conflict", "maybe")).unwrap_err()
            .message.contains("unknown support level"));
        assert!(verify_tier_selection(&TierSelectionRequest { overrides: (0..17)
            .map(|_| TierOverrideSpec { capability: "conflict".to_owned(),
                support: "supported".to_owned() }).collect(), ..request() }).unwrap_err()
            .message.contains("at most 16 overrides"));
    }

    /// 守门：DTO 必须是 camelCase 且拒绝未知字段。 / A guard: camelCase wire, unknown fields refused.
    #[test]
    fn the_report_wire_is_camel_case_and_rejects_unknown_fields() {
        let report = verify_tier_selection(&request()).unwrap();
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"selectedIsNative\""), "{json}");
        assert!(json.contains("\"nativeUnsatCoreSupport\""), "{json}");
        assert!(json.contains("\"theOrderIsThePriority\""), "{json}");
        assert!(!json.contains("selected_is_native"), "{json}");
        let mut copy: serde_json::Value = serde_json::from_str(&json).unwrap();
        copy["unexpected"] = serde_json::json!(1);
        assert!(serde_json::from_str::<TierSelectionReport>(&copy.to_string()).is_err());
    }
}
