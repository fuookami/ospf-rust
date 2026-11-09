//! 续挖第 49 单：`DiagnosticSource` 的六种来源与它们的**身份合并**
//! （`analysis/mod.rs`：`kind()` / `stable_id()` / `validate()` / `as_infeasibility_member()`）
//! Continuation, delivery 49: the six `DiagnosticSource` kinds and where their **identities merge**
//! (`kind()`, `stable_id()`, `validate()` and `as_infeasibility_member()` in `analysis/mod.rs`).
//!
//! 第 36/38/40 单把 `DiagnosticSource` 当作**输入**用过；这一单量它自己的身份面。七条实测：
//!
//! 1. **`kind()` 的六个名字是固定词表**（`kinds_are_stable_names()`）：`constraint` /
//!    `variable-lower-bound` / `variable-upper-bound` / `variable-bound` / `sparse-domain` / `objective-target`。
//! 2. **`stable_id()` 把"下界"与"带侧边界"合并成同一个 ID**（`stable_id_merges_lower_bound_and_variable_bound()`）：
//!    `VariableLowerBound{x}` 与 `VariableBound{x, Lower}` 都给出 `variable:x:lower`，而它们的 `kind()` **不同**——
//!    所以**单看 `stable_id` 无法区分来源种类**。
//! 3. **目标来源另用一个命名空间**（`the_target_is_a_separate_namespace()`）：前缀是 `objective-target:`，
//!    不是 `constraint:` 或 `variable:`。
//! 4. **目标 ID 里的数值走 `canonical_float`**（`target_ids_use_canonical_floats()`）：`2` 与 `2.0` 给出**同一个**
//!    ID（整数补 `.0`），而 `0.1 + 0.2` 与 `0.3` 给出**不同**的 ID；`-0.0` 与 `0.0` 也给出**不同**的 ID
//!    （Rust 的 `-0.0.to_string()` 是 `"-0"`），但这条**客户端表示不出来**（`JSON.stringify(-0) === "0"`）。
//! 5. **空白 ID 被拒，且约束与变量是两条不同措辞**（`blank_ids_are_refused()`）：
//!    "constraint ID must not be blank" 与 "variable ID must not be blank"。
//! 6. **`kind` + `stable_id` 的复合键才唯一**（`the_kind_completes_the_id()`）：六种来源两两不撞。
//! 7. **目标不是不可行性证据成员**（`the_target_is_not_a_member()`）：`as_infeasibility_member()` 对目标返回 `None`，
//!    对其余五种返回 `Some`。
//!
//! Deliveries 36/38/40 used `DiagnosticSource` as an **input**; this one measures its own identity surface.
//! **The six `kind()` names are a fixed vocabulary** (`kinds_are_stable_names()`). **`stable_id()` merges "lower bound" and
//! "sided bound" into one ID** (`stable_id_merges_lower_bound_and_variable_bound()`): `VariableLowerBound{x}` and
//! `VariableBound{x, Lower}` both yield `variable:x:lower` while their `kind()` **differs**, so **a `stable_id` alone cannot
//! tell the kind**. **The target uses its own namespace** (`the_target_is_a_separate_namespace()`), prefixed
//! `objective-target:`. **Numeric parts go through `canonical_float`** (`target_ids_use_canonical_floats()`): `2` and `2.0`
//! share one ID (integers gain `.0`) while `0.1 + 0.2` and `0.3` differ, and `-0.0` differs from `0.0` (Rust prints `"-0"`) —
//! though a client **cannot express that last one** (`JSON.stringify(-0) === "0"`). **Blank IDs are refused with two different
//! messages** (`blank_ids_are_refused()`). **Only `kind` + `stable_id` together are unique** (`the_kind_completes_the_id()`).
//! And **the target is not an infeasibility-evidence member** (`the_target_is_not_a_member()`):
//! `as_infeasibility_member()` returns `None` for it and `Some` for the other five.

use ospf_rust_core::analysis::{
    BoundSide, DiagnosticSource, ObjectiveId, ObjectiveTarget, VariableBoundRef,
};
use ospf_rust_core::solver::report::{StableConstraintId, StableVariableId};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 一条来源的字面量。 / One source literal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourceSpec {
    /// `constraint` / `variableLowerBound` / `variableUpperBound` / `variableBound` / `sparseDomain` /
    /// `objectiveTarget`。 / The source kind.
    pub kind: String,
    /// 约束或变量 ID。 / The constraint or variable ID.
    #[serde(default)]
    pub id: Option<String>,
    /// `variableBound` 的侧：`lower` 或 `upper`。 / The side of `variableBound`.
    #[serde(default)]
    pub side: Option<String>,
    /// 目标 ID。 / The objective ID.
    #[serde(default)]
    pub objective: Option<String>,
    /// 目标关系：`at-least` 或 `at-most`。 / The target relation.
    #[serde(default)]
    pub relation: Option<String>,
    /// 目标值。 / The target value.
    #[serde(default)]
    pub value: Option<f64>,
}

/// 一条来源的身份结论。 / One source's identity verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourceIdentity {
    /// `kind()` 的返回值（来源建不起来时为 `null`）。 / The `kind()` value (`null` when the source cannot be built).
    #[serde(default)]
    pub kind: Option<String>,
    /// `stable_id()` 的返回值。 / The `stable_id()` value.
    #[serde(default)]
    pub stable_id: Option<String>,
    /// `as_infeasibility_member()` 是否有值。 / Whether `as_infeasibility_member()` has a value.
    #[serde(default)]
    pub member: Option<bool>,
    /// 失败消息。 / The failure message.
    #[serde(default)]
    pub error: Option<String>,
}

/// 来源身份请求。 / The source-identity request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourceIdentityRequest {
    #[serde(default)]
    pub sources: Vec<SourceSpec>,
}

/// 来源身份结论。 / The source-identity verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourceIdentityReport {
    /// 逐条结论。 / One verdict per source.
    pub sources: Vec<SourceIdentity>,
    /// 六种 `kind` 词表。 / The six-name `kind` vocabulary.
    pub kinds: Vec<String>,
    /// 下界与带侧边界给出同一个 ID（**是**）。 / Whether the lower bound and the sided bound share one ID (**yes**).
    pub stable_id_merges_lower_and_bound: bool,
    /// 目标另用命名空间（**是**）。 / Whether the target uses its own namespace (**yes**).
    pub the_target_is_a_separate_namespace: bool,
    /// 目标数值走 canonical float（**是**）。 / Whether target numbers go through `canonical_float` (**yes**).
    pub target_ids_use_canonical_floats: bool,
    /// 空白 ID 被拒（**是**）。 / Whether blank IDs are refused (**yes**).
    pub blank_ids_are_refused: bool,
    /// `kind` + `stable_id` 才唯一（**是**）。 / Whether only `kind` + `stable_id` is unique (**yes**).
    pub the_kind_completes_the_id: bool,
    /// 目标不是证据成员（**是**）。 / Whether the target is not an evidence member (**yes**).
    pub the_target_is_not_a_member: bool,
}

/// 来源条数上限（信任边界）。 / The source cap (a trust boundary).
const MAX_SOURCES: usize = 64;

/// 造一条来源。 / Build one source.
fn source(spec: &SourceSpec) -> Result<DiagnosticSource> {
    let id = spec.id.clone().unwrap_or_default();
    let failed = |message: String| BridgeError::new("InvalidInput",
        &format!("diagnostic source failed validation: {message}"));
    match spec.kind.as_str() {
        "constraint" => DiagnosticSource::constraint(StableConstraintId(id))
            .map_err(|error| failed(error.to_string())),
        "variableLowerBound" => DiagnosticSource::variable_lower_bound(StableVariableId(id))
            .map_err(|error| failed(error.to_string())),
        "variableUpperBound" => DiagnosticSource::variable_upper_bound(StableVariableId(id))
            .map_err(|error| failed(error.to_string())),
        "variableBound" => {
            let side = match spec.side.as_deref().unwrap_or("lower") {
                "lower" => BoundSide::Lower,
                "upper" => BoundSide::Upper,
                other => return Err(failed(format!("unknown bound side `{other}`"))),
            };
            DiagnosticSource::variable_bound(VariableBoundRef {
                variable_id: StableVariableId(id), side })
                .map_err(|error| failed(error.to_string()))
        }
        "sparseDomain" => DiagnosticSource::sparse_domain(StableVariableId(id))
            .map_err(|error| failed(error.to_string())),
        "objectiveTarget" => {
            let objective = ObjectiveId(spec.objective.clone().unwrap_or_default());
            let value = spec.value
                .ok_or_else(|| failed("an objective target needs a value".to_owned()))?;
            let target = match spec.relation.as_deref().unwrap_or("at-least") {
                "at-least" => ObjectiveTarget::at_least(objective, value),
                "at-most" => ObjectiveTarget::at_most(objective, value),
                other => return Err(failed(format!("unknown target relation `{other}`"))),
            }
            .map_err(|error| failed(error.to_string()))?;
            DiagnosticSource::objective_target(target).map_err(|error| failed(error.to_string()))
        }
        other => Err(failed(format!("unknown kind `{other}`"))),
    }
}

/// 走一遍来源身份。非法输入是**错误**。 / Walk the source identities.
pub fn verify_source_identity(request: &SourceIdentityRequest) -> Result<SourceIdentityReport> {
    if request.sources.len() > MAX_SOURCES {
        return Err(BridgeError::new("InvalidInput", &format!(
            "diagnostic source failed validation: at most {MAX_SOURCES} sources are accepted")));
    }
    let sources = request.sources.iter().map(|spec| match source(spec) {
        Ok(value) => SourceIdentity { kind: Some(value.kind().to_owned()),
            stable_id: Some(value.stable_id()),
            member: Some(value.as_infeasibility_member().is_some()), error: None },
        Err(error) => SourceIdentity { kind: None, stable_id: None, member: None,
            error: Some(error.message) },
    }).collect::<Vec<_>>();
    let findings = canonical_findings();
    Ok(SourceIdentityReport { sources, kinds: findings.kinds,
        stable_id_merges_lower_and_bound: findings.stable_id_merges_lower_and_bound,
        the_target_is_a_separate_namespace: findings.the_target_is_a_separate_namespace,
        target_ids_use_canonical_floats: findings.target_ids_use_canonical_floats,
        blank_ids_are_refused: findings.blank_ids_are_refused,
        the_kind_completes_the_id: findings.the_kind_completes_the_id,
        the_target_is_not_a_member: findings.the_target_is_not_a_member })
}

/// 七条规则属性的内部固定探针。 / The seven rule properties measured by fixed internal probes.
struct Findings {
    kinds: Vec<String>,
    stable_id_merges_lower_and_bound: bool,
    the_target_is_a_separate_namespace: bool,
    target_ids_use_canonical_floats: bool,
    blank_ids_are_refused: bool,
    the_kind_completes_the_id: bool,
    the_target_is_not_a_member: bool,
}

/// 用固定探针测出全部规则属性——不读调用方的请求值。
/// Measure every rule property with fixed probes; the caller's request values are not read.
fn canonical_findings() -> Findings {
    let constraint = DiagnosticSource::constraint(StableConstraintId("c".to_owned())).unwrap();
    let lower = DiagnosticSource::variable_lower_bound(StableVariableId("x".to_owned())).unwrap();
    let upper = DiagnosticSource::variable_upper_bound(StableVariableId("x".to_owned())).unwrap();
    let bound = DiagnosticSource::variable_bound(VariableBoundRef {
        variable_id: StableVariableId("x".to_owned()), side: BoundSide::Lower }).unwrap();
    let domain = DiagnosticSource::sparse_domain(StableVariableId("x".to_owned())).unwrap();
    let target = DiagnosticSource::objective_target(
        ObjectiveTarget::at_least(ObjectiveId("obj".to_owned()), 2.0).unwrap()).unwrap();
    let kinds = [&constraint, &lower, &upper, &bound, &domain, &target].iter()
        .map(|source| source.kind().to_owned()).collect::<Vec<_>>();
    // 1) 下界与带侧下界共享同一个 `stable_id`，但 `kind` 不同。 
    // 1) The lower bound and the sided lower bound share a `stable_id` while their `kind` differs.
    let stable_id_merges_lower_and_bound =
        lower.stable_id() == bound.stable_id() && lower.kind() != bound.kind();
    // 2) 目标另用一个命名空间。 / The target uses its own namespace.
    let the_target_is_a_separate_namespace = target.stable_id().starts_with("objective-target:")
        && !target.stable_id().starts_with("variable:")
        && !target.stable_id().starts_with("constraint:");
    // 3) 目标数值走 `canonical_float`：整数补 `.0`，`0.1+0.2 != 0.3`，`-0.0 != 0.0`。 
    // 3) Target numbers use `canonical_float`: integers gain `.0`, `0.1+0.2 != 0.3`, and `-0.0 != 0.0`.
    let target_ids_use_canonical_floats = {
        let id_of = |value: f64| DiagnosticSource::objective_target(
            ObjectiveTarget::at_least(ObjectiveId("obj".to_owned()), value).unwrap())
            .unwrap().stable_id();
        // 整数补 `.0`。 / Integers gain `.0`.
        id_of(2.0).ends_with(":2.0")
            // `0.1 + 0.2 != 0.3`。 / `0.1 + 0.2 != 0.3`.
            && id_of(0.1 + 0.2) != id_of(0.3)
            // `-0.0` 打印成 `"-0"`，与 `"0"` 不同。 / `-0.0` prints as `"-0"`, unlike `"0"`.
            && id_of(-0.0) != id_of(0.0)
    };
    // 4) 空白 ID 被拒，措辞分约束与变量两支。 / Blank IDs are refused with two different messages.
    let blank_ids_are_refused = {
        let constraint = DiagnosticSource::constraint(StableConstraintId("  ".to_owned()))
            .err().map(|error| error.to_string()).unwrap_or_default();
        let variable = DiagnosticSource::variable_lower_bound(StableVariableId("  ".to_owned()))
            .err().map(|error| error.to_string()).unwrap_or_default();
        constraint.contains("constraint ID must not be blank")
            && variable.contains("variable ID must not be blank")
    };
    // 5) `kind` + `stable_id` 的复合键才唯一。 / Only `kind` + `stable_id` is unique.
    let the_kind_completes_the_id = {
        let all = [&constraint, &lower, &upper, &bound, &domain, &target];
        let mut keys = std::collections::BTreeSet::new();
        all.iter().all(|source| keys.insert((source.kind(), source.stable_id())))
            && keys.len() == 6
            // 只用 `stable_id` 就只有 5 个键。 / Using only `stable_id` gives 5 keys.
            && all.iter().map(|source| source.stable_id())
                .collect::<std::collections::BTreeSet<_>>().len() == 5
    };
    // 6) 目标不是证据成员。 / The target is not an evidence member.
    let the_target_is_not_a_member = target.as_infeasibility_member().is_none()
        && [&constraint, &lower, &upper, &bound, &domain].iter()
            .all(|source| source.as_infeasibility_member().is_some());
    Findings { kinds, stable_id_merges_lower_and_bound, the_target_is_a_separate_namespace,
        target_ids_use_canonical_floats, blank_ids_are_refused, the_kind_completes_the_id,
        the_target_is_not_a_member }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> SourceIdentityRequest {
        SourceIdentityRequest { sources: vec![
            SourceSpec { kind: "constraint".to_owned(), id: Some("capacity".to_owned()),
                side: None, objective: None, relation: None, value: None },
            SourceSpec { kind: "variableBound".to_owned(), id: Some("x".to_owned()),
                side: Some("upper".to_owned()), objective: None, relation: None, value: None },
            SourceSpec { kind: "objectiveTarget".to_owned(), id: None, side: None,
                objective: Some("cost".to_owned()), relation: Some("at-most".to_owned()),
                value: Some(12.5) },
        ] }
    }

    #[test]
    fn six_kinds_are_a_fixed_vocabulary() {
        let report = verify_source_identity(&request()).unwrap();
        assert_eq!(report.kinds, vec!["constraint".to_owned(),
            "variable-lower-bound".to_owned(), "variable-upper-bound".to_owned(),
            "variable-bound".to_owned(), "sparse-domain".to_owned(),
            "objective-target".to_owned()]);
        assert_eq!(report.sources[0].kind.as_deref(), Some("constraint"));
        assert_eq!(report.sources[0].stable_id.as_deref(), Some("constraint:capacity"));
        assert_eq!(report.sources[1].kind.as_deref(), Some("variable-bound"));
        assert_eq!(report.sources[1].stable_id.as_deref(), Some("variable:x:upper"));
        assert!(report.sources[2].stable_id.as_deref().unwrap_or_default()
            .starts_with("objective-target:cost:at-most:12.5"));
        assert_eq!(report.sources[1].member, Some(true));
        assert_eq!(report.sources[2].member, Some(false));
    }

    #[test]
    fn a_lower_bound_and_a_sided_lower_bound_share_one_id() {
        let report = verify_source_identity(&request()).unwrap();
        assert!(report.stable_id_merges_lower_and_bound);
        assert!(report.the_kind_completes_the_id);
        // 实测：同一变量两种来源，ID 相同、kind 不同。 
        // Measured: two sources on one variable share an ID but differ in kind.
        let pair = SourceIdentityRequest { sources: vec![
            SourceSpec { kind: "variableLowerBound".to_owned(), id: Some("y".to_owned()),
                side: None, objective: None, relation: None, value: None },
            SourceSpec { kind: "variableBound".to_owned(), id: Some("y".to_owned()),
                side: Some("lower".to_owned()), objective: None, relation: None, value: None },
        ] };
        let report = verify_source_identity(&pair).unwrap();
        assert_eq!(report.sources[0].stable_id, report.sources[1].stable_id);
        assert_ne!(report.sources[0].kind, report.sources[1].kind);
    }

    #[test]
    fn the_target_has_its_own_namespace_and_canonical_numbers() {
        let report = verify_source_identity(&request()).unwrap();
        assert!(report.the_target_is_a_separate_namespace);
        assert!(report.target_ids_use_canonical_floats);
        assert!(report.the_target_is_not_a_member);
        // 整数补 `.0`：`2` 与 `2.0` 同一个 ID。 / Integers gain `.0`: `2` and `2.0` share one ID.
        let target = |value: f64| SourceIdentityRequest { sources: vec![SourceSpec {
            kind: "objectiveTarget".to_owned(), id: None, side: None,
            objective: Some("obj".to_owned()), relation: Some("at-least".to_owned()),
            value: Some(value) }] };
        let two = verify_source_identity(&target(2.0)).unwrap();
        assert!(two.sources[0].stable_id.as_deref().unwrap_or_default().ends_with(":2.0"));
        // `0.1 + 0.2` 与 `0.3` 不同。 / `0.1 + 0.2` differs from `0.3`.
        let sum = verify_source_identity(&target(0.1 + 0.2)).unwrap();
        let three = verify_source_identity(&target(0.3)).unwrap();
        assert_ne!(sum.sources[0].stable_id, three.sources[0].stable_id);
    }

    #[test]
    fn blank_ids_are_refused_with_two_messages() {
        let report = verify_source_identity(&request()).unwrap();
        assert!(report.blank_ids_are_refused);
        let blank = SourceIdentityRequest { sources: vec![
            SourceSpec { kind: "constraint".to_owned(), id: Some("  ".to_owned()),
                side: None, objective: None, relation: None, value: None },
            SourceSpec { kind: "sparseDomain".to_owned(), id: Some("  ".to_owned()),
                side: None, objective: None, relation: None, value: None },
        ] };
        let report = verify_source_identity(&blank).unwrap();
        assert!(report.sources[0].error.as_deref().unwrap_or_default()
            .contains("constraint ID must not be blank"));
        assert!(report.sources[1].error.as_deref().unwrap_or_default()
            .contains("variable ID must not be blank"));
        assert_eq!(report.sources[0].kind, None);
        assert_eq!(report.sources[0].stable_id, None);
    }

    /// **按实测纠正**：单条的构造失败**被记录**在 `error` 里，不打断整批——只有条数超限才是整体错误。
    /// Measured correction: a per-source build failure is **recorded** in `error` instead of aborting the batch; only the
    /// count cap is an overall error.
    #[test]
    fn per_source_failures_are_recorded_but_the_cap_is_overall() {
        let cases = vec![
            (SourceSpec { kind: "observation".to_owned(), id: Some("x".to_owned()), side: None,
                objective: None, relation: None, value: None }, "unknown kind"),
            (SourceSpec { kind: "variableBound".to_owned(), id: Some("x".to_owned()),
                side: Some("middle".to_owned()), objective: None, relation: None, value: None },
                "unknown bound side"),
            (SourceSpec { kind: "objectiveTarget".to_owned(), id: None, side: None,
                objective: Some("obj".to_owned()), relation: Some("around".to_owned()),
                value: Some(1.0) }, "unknown target relation"),
            (SourceSpec { kind: "objectiveTarget".to_owned(), id: None, side: None,
                objective: Some("obj".to_owned()), relation: None, value: None }, "needs a value"),
        ];
        for (spec, expected) in cases {
            let report = verify_source_identity(&SourceIdentityRequest { sources: vec![spec] })
                .unwrap();
            assert_eq!(report.sources.len(), 1);
            assert!(report.sources[0].error.as_deref().unwrap_or_default()
                .contains(expected), "{expected}");
            assert_eq!(report.sources[0].kind, None);
        }
        let too_many = SourceIdentityRequest { sources: (0..65).map(|index| SourceSpec {
            kind: "sparseDomain".to_owned(), id: Some(format!("v{index}")), side: None,
            objective: None, relation: None, value: None }).collect() };
        assert!(verify_source_identity(&too_many).unwrap_err().message
            .contains("at most 64 sources"));
    }

    /// 守门：DTO 必须是 camelCase 且拒绝未知字段。 / A guard: camelCase wire, unknown fields refused.
    #[test]
    fn the_report_wire_is_camel_case_and_rejects_unknown_fields() {
        let report = verify_source_identity(&request()).unwrap();
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"stableId\""), "{json}");
        assert!(json.contains("\"stableIdMergesLowerAndBound\""), "{json}");
        assert!(json.contains("\"theKindCompletesTheId\""), "{json}");
        assert!(!json.contains("stable_id"), "{json}");
        let mut copy: serde_json::Value = serde_json::from_str(&json).unwrap();
        copy["unexpected"] = serde_json::json!(1);
        assert!(serde_json::from_str::<SourceIdentityReport>(&copy.to_string()).is_err());
    }
}
