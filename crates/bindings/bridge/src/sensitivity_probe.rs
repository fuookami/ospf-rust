//! 续挖第 109 单：**灵敏度容差没有顺序约束，而"有没有对偶"完全不看容差**——`integrality_tolerance` 与
//! `dual_tolerance` 只要求有限且非负，**两个容差谁大谁小都不管**；`has_duals()` 只问"有没有 `Some`"，
//! 于是**零对偶算有对偶**、**连非有限的对偶也算有对偶**（而 `validate()` 会拒后者）；按身份查记录是
//! **先到先得**（重复 ID 只给第一条）；范围名与线上名**不一致**（`fixed_integer_incumbent` 对
//! `FIXED_INTEGER_INCUMBENT`）（`analysis/fixed_integer.rs` 的 `FixedIntegerLpSensitivityConfig` /
//! `LocalConstraintSensitivityReport` / `SensitivityScope`）
//! Continuation, delivery 109: **the sensitivity tolerances carry no ordering constraint, and "are there duals" ignores
//! them entirely** — `integrality_tolerance` and `dual_tolerance` must only be finite and non-negative, so **whichever is
//! larger is irrelevant**; `has_duals()` asks only whether a `Some` exists, so **a zero dual counts as a dual** and **even a
//! non-finite dual counts** while `validate()` refuses the latter; looking a record up by identity is **first wins**, so
//! duplicate IDs yield the first entry; and the scope name differs from the wire name (`fixed_integer_incumbent` against
//! `FIXED_INTEGER_INCUMBENT`) (`FixedIntegerLpSensitivityConfig` / `LocalConstraintSensitivityReport` / `SensitivityScope`
//! in `analysis/fixed_integer.rs`).
//!
//! 第 108 单量的是修正建议，这一单量**定点整数 LP 灵敏度的口径**。八条实测：
//!
//! 1. **两个容差必须有限且非负**（`the_tolerances_must_be_finite_and_non_negative()`）。
//! 2. **容差之间没有顺序约束**（`the_tolerances_are_not_ordered()`）：`integrality < dual` 也合法。
//! 3. **默认值是 `1e-7` 与 `1e-9`**（`the_defaults_are_the_documented_pair()`）。
//! 4. **报告的空 schema / 非有限目标值 / 空约束 ID / 非有限对偶都被拒**
//!    （`the_report_validation_covers_five_fields()`）。
//! 5. **`has_duals()` 只问有没有 `Some`**（`has_duals_counts_any_some()`）：**零对偶算有对偶**。
//! 6. **`has_duals()` 连非有限对偶都算**（`has_duals_ignores_validity()`），而 `validate()` 会拒这种报告。
//! 7. **按身份查记录是先到先得**（`the_lookup_is_first_wins()`）。
//! 8. **范围名与线上名不一致**（`the_scope_name_differs_from_the_wire()`）：`fixed_integer_incumbent` 对
//!    `FIXED_INTEGER_INCUMBENT`。
//!
//! Delivery 108 measured the correction recommendation; this one measures **the fixed-integer LP sensitivity readings**.
//! **The tolerances must be finite and non-negative** and **carry no ordering constraint**. **The defaults are `1e-7` and
//! `1e-9`.** **The report validation covers five fields.** **`has_duals()` counts any `Some`**, so a zero dual counts,
//! and **it ignores validity**, counting even a non-finite dual that `validate()` refuses. **The lookup is first wins.** And
//! **the scope name differs from the wire name.**

use ospf_rust_core::analysis::fixed_integer::{
    FixedIntegerLpSensitivityCache, FixedIntegerLpSensitivityConfig, LocalConstraintSensitivity,
    LocalConstraintSensitivityReport, SensitivityScope,
};
use ospf_rust_core::analysis::AnalysisStatus;
use ospf_rust_core::solver::StableConstraintId;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 一条约束局部灵敏度的描述。 / The description of one constraint's local sensitivity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LocalSensitivitySpec {
    /// 稳定约束身份。 / The stable constraint identity.
    pub id: String,
    /// 对偶值形状：`none` / `zero` / `one` / `infinite`（默认 `one`）。 / The dual shape.
    #[serde(default)]
    pub dual: Option<String>,
    /// 是否局部有效（缺省表示未知）。 / Whether it is locally effective.
    #[serde(default)]
    pub local_effective: Option<bool>,
}

/// 灵敏度探针请求。 / The sensitivity probe request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SensitivityProbeRequest {
    /// 整数性容差（默认 1e-7）。 / The integrality tolerance.
    #[serde(default)]
    pub integrality_tolerance: Option<f64>,
    /// 零对偶容差（默认 1e-9）。 / The zero-dual tolerance.
    #[serde(default)]
    pub dual_tolerance: Option<f64>,
    /// 报告 schema 版本（默认 `sensitivity`）。 / The report schema version.
    #[serde(default)]
    pub schema_version: Option<String>,
    /// baseline 目标值（默认 0）。 / The baseline objective.
    #[serde(default)]
    pub baseline_objective: Option<f64>,
    /// 固定整数 LP 目标值（缺省表示没有）。 / The LP objective value.
    #[serde(default)]
    pub lp_objective: Option<f64>,
    /// 每条约束的局部证据。 / The per-constraint evidence.
    #[serde(default)]
    pub constraints: Vec<LocalSensitivitySpec>,
    /// 要查询的记录身份。 / The identity to look up.
    #[serde(default)]
    pub query: Option<String>,
}

/// 灵敏度探针结论。 / The sensitivity probe verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SensitivityProbeReport {
    /// 配置是否合法。 / Whether the configuration validates.
    pub config_valid: bool,
    /// 配置校验的错误文本。 / The configuration error text.
    pub config_error: String,
    /// 报告是否合法。 / Whether the report validates.
    pub report_valid: bool,
    /// 报告校验的错误文本。 / The report error text.
    pub report_error: String,
    /// 是否有对偶。 / Whether at least one dual is present.
    pub has_duals: bool,
    /// 查询命中的对偶值（`null` 表示没有命中或没有对偶）。 / The dual of the queried record.
    #[serde(default)]
    pub query_dual: Option<f64>,
    /// 查询是否命中。 / Whether the lookup found a record.
    pub query_hit: bool,
    /// 八条规则属性的读数。 / The eight rule readings.
    pub the_tolerances_must_be_finite_and_non_negative: bool,
    pub the_tolerances_are_not_ordered: bool,
    pub the_defaults_are_the_documented_pair: bool,
    pub the_report_validation_covers_five_fields: bool,
    pub has_duals_counts_any_some: bool,
    pub has_duals_ignores_validity: bool,
    pub the_lookup_is_first_wins: bool,
    pub the_scope_name_differs_from_the_wire: bool,
}

/// 造一条局部证据。 / Build one local evidence record.
fn local_of(spec: &LocalSensitivitySpec) -> Result<LocalConstraintSensitivity> {
    let dual = match spec.dual.as_deref() {
        None | Some("one") => Some(1.0),
        Some("zero") => Some(0.0),
        Some("infinite") => Some(f64::INFINITY),
        Some("none") => None,
        Some(_) => return Err(BridgeError::new("InvalidInput",
            "sensitivity probe failed validation: dual reads none/zero/one/infinite")),
    };
    Ok(LocalConstraintSensitivity {
        constraint_id: StableConstraintId(spec.id.clone()),
        dual_value: dual,
        local_effective: spec.local_effective,
        status: AnalysisStatus::Unknown,
        scope: SensitivityScope::FixedIntegerIncumbent,
        activity: None,
    })
}

/// 走一遍灵敏度探针。非法输入是**错误**。 / Walk the probe. An illegal input is an error.
pub fn verify_sensitivity_probe(request: &SensitivityProbeRequest) -> Result<SensitivityProbeReport> {
    if request.constraints.len() > 8 {
        return Err(BridgeError::new("InvalidInput", "sensitivity probe failed validation: \
            at most eight constraints are accepted"));
    }
    let findings = canonical_findings();
    let config = FixedIntegerLpSensitivityConfig {
        integrality_tolerance: request.integrality_tolerance.unwrap_or(1e-7),
        dual_tolerance: request.dual_tolerance.unwrap_or(1e-9),
    };
    let report = LocalConstraintSensitivityReport {
        schema_version: request.schema_version.clone()
            .unwrap_or_else(|| "sensitivity".to_owned()),
        scope: SensitivityScope::FixedIntegerIncumbent,
        baseline_objective: request.baseline_objective.unwrap_or(0.0),
        lp_objective: request.lp_objective,
        fixed_integer_count: 0,
        constraints: request.constraints.iter().map(local_of)
            .collect::<Result<Vec<_>>>()?,
        status: AnalysisStatus::Unknown,
        termination_reason: None,
        unavailable_reason: None,
    };
    let config_result = config.validate();
    let report_result = report.validate();
    let queried = request.query.as_ref()
        .and_then(|id| report.constraint(&StableConstraintId(id.clone())));
    Ok(SensitivityProbeReport {
        config_valid: config_result.is_ok(),
        config_error: config_result.err().map(|error| error.to_string()).unwrap_or_default(),
        report_valid: report_result.is_ok(),
        report_error: report_result.err().map(|error| error.to_string()).unwrap_or_default(),
        has_duals: report.has_duals(),
        query_dual: queried.and_then(|record| record.dual_value),
        query_hit: queried.is_some(),
        the_tolerances_must_be_finite_and_non_negative:
            findings.the_tolerances_must_be_finite_and_non_negative,
        the_tolerances_are_not_ordered: findings.the_tolerances_are_not_ordered,
        the_defaults_are_the_documented_pair: findings.the_defaults_are_the_documented_pair,
        the_report_validation_covers_five_fields:
            findings.the_report_validation_covers_five_fields,
        has_duals_counts_any_some: findings.has_duals_counts_any_some,
        has_duals_ignores_validity: findings.has_duals_ignores_validity,
        the_lookup_is_first_wins: findings.the_lookup_is_first_wins,
        the_scope_name_differs_from_the_wire: findings.the_scope_name_differs_from_the_wire,
    })
}

/// 固定探针只跑一次。 / The fixed probes run once only.
static FINDINGS: std::sync::OnceLock<Findings> = std::sync::OnceLock::new();

/// 八条规则属性的内部固定探针。 / The eight rule properties measured by fixed internal probes.
struct Findings {
    the_tolerances_must_be_finite_and_non_negative: bool,
    the_tolerances_are_not_ordered: bool,
    the_defaults_are_the_documented_pair: bool,
    the_report_validation_covers_five_fields: bool,
    has_duals_counts_any_some: bool,
    has_duals_ignores_validity: bool,
    the_lookup_is_first_wins: bool,
    the_scope_name_differs_from_the_wire: bool,
}

/// 造一条局部证据（内部用）。 / Build one local record (internal).
fn local(id: &str, dual: Option<f64>) -> LocalConstraintSensitivity {
    LocalConstraintSensitivity {
        constraint_id: StableConstraintId(id.to_owned()),
        dual_value: dual,
        local_effective: None,
        status: AnalysisStatus::Unknown,
        scope: SensitivityScope::FixedIntegerIncumbent,
        activity: None,
    }
}

/// 造一份报告。 / Build one report.
fn report_of(constraints: Vec<LocalConstraintSensitivity>) -> LocalConstraintSensitivityReport {
    LocalConstraintSensitivityReport {
        schema_version: "sensitivity".to_owned(),
        scope: SensitivityScope::FixedIntegerIncumbent,
        baseline_objective: 0.0,
        lp_objective: None,
        fixed_integer_count: 0,
        constraints,
        status: AnalysisStatus::Unknown,
        termination_reason: None,
        unavailable_reason: None,
    }
}

/// 用固定探针测出全部规则属性——不读调用方的请求值。
/// Measure every rule property with fixed probes; the caller's request values are not read.
fn canonical_findings() -> &'static Findings {
    FINDINGS.get_or_init(|| {
        // 1) 两个容差必须有限且非负。 / Both tolerances must be finite and non-negative.
        let the_tolerances_must_be_finite_and_non_negative = {
            let bad = |integrality: f64, dual: f64| {
                FixedIntegerLpSensitivityConfig {
                    integrality_tolerance: integrality, dual_tolerance: dual }.validate().is_err()
            };
            FixedIntegerLpSensitivityConfig::default().validate().is_ok()
                && bad(f64::NAN, 1e-9) && bad(-1.0, 1e-9)
                && bad(1e-7, f64::INFINITY) && bad(1e-7, -1.0)
                // 零是允许的。 / Zero is accepted.
                && FixedIntegerLpSensitivityConfig { integrality_tolerance: 0.0,
                    dual_tolerance: 0.0 }.validate().is_ok()
        };
        // 2) 容差之间没有顺序约束。 / The tolerances carry no ordering constraint.
        let the_tolerances_are_not_ordered = {
            // 整数性容差比零对偶容差小，这在 activity 配置里是非法的，这里合法。 
            // A smaller integrality tolerance is illegal in the activity config yet legal here.
            FixedIntegerLpSensitivityConfig { integrality_tolerance: 1e-12,
                dual_tolerance: 1e-3 }.validate().is_ok()
                && FixedIntegerLpSensitivityConfig { integrality_tolerance: 1e-3,
                    dual_tolerance: 1e-12 }.validate().is_ok()
        };
        // 3) 默认值是 1e-7 与 1e-9。 / The defaults are one e minus seven and one e minus nine.
        let the_defaults_are_the_documented_pair = {
            let defaults = FixedIntegerLpSensitivityConfig::default();
            defaults.integrality_tolerance == 1e-7 && defaults.dual_tolerance == 1e-9
        };
        // 4) 报告校验覆盖五个字段。 / The report validation covers five fields.
        let the_report_validation_covers_five_fields = {
            let honest = report_of(vec![local("c1", Some(1.0))]);
            honest.validate().is_ok()
                && { let mut blank = honest.clone(); blank.schema_version = "  ".to_owned();
                    blank.validate().is_err() }
                && { let mut nan = honest.clone(); nan.baseline_objective = f64::NAN;
                    nan.validate().is_err() }
                && { let mut blank_id = honest.clone();
                    blank_id.constraints = vec![local("   ", Some(1.0))];
                    blank_id.validate().is_err() }
                && { let mut nan_dual = honest.clone();
                    nan_dual.constraints = vec![local("c1", Some(f64::NAN))];
                    nan_dual.validate().is_err() }
                && { let mut nan_lp = honest.clone(); nan_lp.lp_objective = Some(f64::INFINITY);
                    nan_lp.validate().is_err() }
        };
        // 5) has_duals 只问有没有 Some。 / has_duals only asks whether a Some exists.
        let has_duals_counts_any_some = {
            // 零对偶算有对偶。 / A zero dual counts.
            report_of(vec![local("c1", Some(0.0))]).has_duals()
                && report_of(vec![local("c1", Some(1.0))]).has_duals()
                // 全为 None 时不算。 / All-none does not count.
                && !report_of(vec![local("c1", None), local("c2", None)]).has_duals()
                && !report_of(Vec::new()).has_duals()
        };
        // 6) has_duals 不看有效性。 / has_duals ignores validity.
        let has_duals_ignores_validity = {
            // 非有限对偶：validate 拒，has_duals 仍为真。 
            // A non-finite dual is refused by validate yet still counts as a dual.
            let broken = report_of(vec![local("c1", Some(f64::NAN))]);
            broken.validate().is_err() && broken.has_duals()
        };
        // 7) 查记录先到先得。 / The lookup is first wins.
        let the_lookup_is_first_wins = {
            let duplicated = report_of(vec![local("c1", Some(1.0)), local("c1", Some(9.0))]);
            duplicated.constraint(&StableConstraintId("c1".to_owned()))
                .is_some_and(|record| record.dual_value == Some(1.0))
                && duplicated.constraint(&StableConstraintId("c2".to_owned())).is_none()
        };
        // 8) 范围名与线上名不一致。 / The scope name differs from the wire name.
        let the_scope_name_differs_from_the_wire = {
            let scope = SensitivityScope::FixedIntegerIncumbent;
            let encoded = serde_json::to_value(scope).expect("scope serialises");
            scope.stable_name() == "fixed_integer_incumbent"
                && encoded.as_str() == Some("FIXED_INTEGER_INCUMBENT")
                && SensitivityScope::default() == scope
        };
        // 顺手记下缓存默认是空的。 / Note the cache starts empty.
        let mut cache = FixedIntegerLpSensitivityCache::new();
        debug_assert!(cache.is_empty() && cache.len() == 0);
        cache.clear();
        Findings {
            the_tolerances_must_be_finite_and_non_negative,
            the_tolerances_are_not_ordered,
            the_defaults_are_the_documented_pair,
            the_report_validation_covers_five_fields,
            has_duals_counts_any_some,
            has_duals_ignores_validity,
            the_lookup_is_first_wins,
            the_scope_name_differs_from_the_wire,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ask(request: SensitivityProbeRequest) -> SensitivityProbeReport {
        verify_sensitivity_probe(&request).unwrap()
    }

    fn spec(id: &str, dual: Option<&str>) -> LocalSensitivitySpec {
        LocalSensitivitySpec { id: id.to_owned(), dual: dual.map(str::to_owned),
            local_effective: None }
    }

    fn request() -> SensitivityProbeRequest {
        SensitivityProbeRequest { integrality_tolerance: None, dual_tolerance: None,
            schema_version: None, baseline_objective: None, lp_objective: None,
            constraints: vec![spec("c1", Some("one"))], query: None }
    }

    #[test]
    fn the_eight_rules_hold_on_a_plain_run() {
        let report = ask(request());
        assert!(report.config_valid, "{}", report.config_error);
        assert!(report.report_valid, "{}", report.report_error);
        assert!(report.has_duals);
        assert!(report.the_tolerances_must_be_finite_and_non_negative);
        assert!(report.the_tolerances_are_not_ordered);
        assert!(report.the_defaults_are_the_documented_pair);
        assert!(report.the_report_validation_covers_five_fields);
        assert!(report.has_duals_counts_any_some);
        assert!(report.has_duals_ignores_validity);
        assert!(report.the_lookup_is_first_wins);
        assert!(report.the_scope_name_differs_from_the_wire);
    }

    #[test]
    fn the_tolerances_are_only_bounded_not_ordered() {
        assert!(ask(SensitivityProbeRequest { integrality_tolerance: Some(1e-12),
            dual_tolerance: Some(1e-3), ..request() }).config_valid);
        assert!(ask(SensitivityProbeRequest { integrality_tolerance: Some(1e-3),
            dual_tolerance: Some(1e-12), ..request() }).config_valid);
        let negative = ask(SensitivityProbeRequest { integrality_tolerance: Some(-1.0),
            ..request() });
        assert!(!negative.config_valid);
        assert!(negative.config_error.contains("finite and non-negative"),
            "{}", negative.config_error);
        assert!(!ask(SensitivityProbeRequest { dual_tolerance: Some(f64::NAN), ..request() })
            .config_valid);
    }

    #[test]
    fn the_report_validation_covers_five_fields() {
        assert!(!ask(SensitivityProbeRequest { schema_version: Some("  ".to_owned()),
            ..request() }).report_valid);
        assert!(!ask(SensitivityProbeRequest { baseline_objective: Some(f64::NAN),
            ..request() }).report_valid);
        assert!(!ask(SensitivityProbeRequest { lp_objective: Some(f64::INFINITY),
            ..request() }).report_valid);
        assert!(!ask(SensitivityProbeRequest { constraints: vec![spec("  ", Some("one"))],
            ..request() }).report_valid);
        assert!(!ask(SensitivityProbeRequest { constraints: vec![spec("c1", Some("infinite"))],
            ..request() }).report_valid);
    }

    #[test]
    fn has_duals_counts_any_some_and_ignores_validity() {
        // 零对偶算有对偶。 / A zero dual counts.
        assert!(ask(SensitivityProbeRequest { constraints: vec![spec("c1", Some("zero"))],
            ..request() }).has_duals);
        assert!(!ask(SensitivityProbeRequest { constraints: vec![spec("c1", Some("none"))],
            ..request() }).has_duals);
        assert!(!ask(SensitivityProbeRequest { constraints: Vec::new(), ..request() })
            .has_duals);
        // 非有限对偶：报告非法，但仍算"有对偶"。 
        // A non-finite dual makes the report illegal yet still counts.
        let broken = ask(SensitivityProbeRequest {
            constraints: vec![spec("c1", Some("infinite"))], ..request() });
        assert!(!broken.report_valid);
        assert!(broken.has_duals);
    }

    #[test]
    fn the_lookup_is_first_wins() {
        let duplicated = ask(SensitivityProbeRequest {
            constraints: vec![spec("c1", Some("one")), spec("c1", Some("zero"))],
            query: Some("c1".to_owned()), ..request() });
        assert!(duplicated.query_hit);
        assert_eq!(duplicated.query_dual, Some(1.0));
        let missing = ask(SensitivityProbeRequest { query: Some("c2".to_owned()), ..request() });
        assert!(!missing.query_hit);
        assert_eq!(missing.query_dual, None);
    }

    #[test]
    fn bad_input_is_refused() {
        assert!(verify_sensitivity_probe(&SensitivityProbeRequest {
            constraints: (0..9).map(|index| spec(&format!("c{index}"), None)).collect(),
            ..request() }).unwrap_err().message.contains("at most eight constraints"));
        assert!(verify_sensitivity_probe(&SensitivityProbeRequest {
            constraints: vec![spec("c1", Some("two"))], ..request() }).unwrap_err()
            .message.contains("none/zero/one/infinite"));
    }

    /// 守门：DTO 必须是 camelCase 且拒绝未知字段。 / A guard: camelCase wire, unknown fields refused.
    #[test]
    fn the_report_wire_is_camel_case_and_rejects_unknown_fields() {
        let report = ask(request());
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"configValid\""), "{json}");
        assert!(json.contains("\"hasDuals\""), "{json}");
        assert!(json.contains("\"theScopeNameDiffersFromTheWire\""), "{json}");
        assert!(!json.contains("config_valid"), "{json}");
        let mut copy: serde_json::Value = serde_json::from_str(&json).unwrap();
        copy["unexpected"] = serde_json::json!(1);
        assert!(serde_json::from_str::<SensitivityProbeReport>(&copy.to_string()).is_err());
    }
}
