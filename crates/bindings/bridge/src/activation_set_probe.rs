//! 续挖第 78 单：诊断**激活集合**的语义——"缺席即未激活"、`insert` 覆盖并移位、两个变体共享一个稳定身份
//! （`analysis/activation.rs` 的 `DiagnosticActivation` / `ActivationState` / `DiagnosticActivationSet`）
//! Continuation, delivery 78: the semantics of the diagnostic **activation set** — "absent means inactive", `insert`
//! overrides and re-orders, and two distinct variants share one stable identity
//! (`DiagnosticActivation`, `ActivationState`, `DiagnosticActivationSet` in `analysis/activation.rs`).
//!
//! 这一单量的是"哪些原始证据参与了这次求解"这套账。八条实测：
//!
//! 1. **`insert` 覆盖并移位**（`insert_overrides_and_reorders()`）：同一来源再次插入时先删旧记录再追加，
//!    所以它既覆盖状态、又把它挪到末尾。
//! 2. **缺席即未激活**（`absence_means_inactive()`）：`is_active` 只查"有没有一条激活记录"，
//!    **不是**"没有被停用"——从未出现的来源是未激活。
//! 3. **`activate_source` / `deactivate_source` 校验来源，而 `insert` 不校验**
//!    （`the_typed_entry_points_validate_but_insert_does_not()`）：空白 ID 可以经 `insert` 进集合。
//! 4. **状态进身份**（`the_state_enters_the_stable_identity()`）：`activated:` / `deactivated:` 前缀，
//!    所以同一来源的两种状态是两个不同身份。
//! 5. **`VariableLowerBound{x}` 与 `VariableBound{x, Lower}` 身份相同但互不相等**
//!    （`two_variants_share_one_stable_identity()`）：两者 `stable_id` 都是 `variable:x:lower`，
//!    但 `PartialEq` 判不同，于是 `insert` 会**同时保留两条同身份记录**。本单最值钱的一条。
//! 6. **`to_assumptions` 永不为约束伪造 assumption**（`constraints_never_become_assumptions()`）：
//!    只处理变量边界与稀疏域，`Constraint` 与 `ObjectiveTarget` 一律跳过。
//! 7. **`active_constraints` 只收 `Constraint` 来源**（`active_constraints_only_reads_constraints()`）。
//! 8. **`from_snapshot` 的激活面由域形态决定**（`from_snapshot_activates_by_domain_shape()`）：
//!    每个约束一条、每个变量上下界两条，稀疏域**额外**一条 `SparseDomain`。
//!
//! This delivery measures the bookkeeping of "which original evidence took part in this solve". **`insert` overrides and
//! re-orders.** **Absence means inactive** — `is_active` asks "is there an active record", not "was it deactivated".
//! **The typed entry points validate while `insert` does not.** **The state enters the stable identity.**
//! **`VariableLowerBound{x}` and `VariableBound{x, Lower}` share one stable identity yet are not equal**, so `insert`
//! keeps **two records with the same identity**. **`to_assumptions` never fabricates assumptions for constraints.**
//! **`active_constraints` only reads constraint sources.** And **`from_snapshot`'s activation surface follows the domain
//! shape**.

use std::collections::BTreeSet;

use ospf_rust_core::analysis::{
    ActivationState, BoundSide, DiagnosticActivation, DiagnosticActivationSet, DiagnosticSource,
    VariableBoundRef,
};
use ospf_rust_core::model::constraint_programming::{
    ConstraintDefinition, ConstraintProgrammingConstraint, ConstraintProgrammingModel,
    ConstraintProgrammingSnapshot, IntegerDomain, IntegerExpression, IntegerVariable,
};
use ospf_rust_core::solver::{StableConstraintId, StableVariableId};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 一个变量的声明。 / One variable declaration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ActivationVariableSpec {
    /// 稳定变量 ID。 / The stable variable ID.
    pub id: String,
    /// 稀疏取值（非空即稀疏域，否则用 `0..=upper`）。 / Sparse values (non-empty means sparse, else `0..=upper`).
    #[serde(default)]
    pub values: Vec<i64>,
    /// 区间上界（默认 10）。 / The range upper bound (default 10).
    #[serde(default)]
    pub upper: Option<i64>,
}

/// 一条来源的声明。 / One source declaration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ActivationSourceSpec {
    /// 来源种类：`constraint` / `lowerBound` / `upperBound` / `variableBound` / `sparseDomain`。
    /// The source kind.
    pub kind: String,
    /// 稳定 ID。 / The stable ID.
    #[serde(default)]
    pub id: String,
    /// `variableBound` 的方向：`lower` / `upper`。 / The side for `variableBound`.
    #[serde(default)]
    pub side: Option<String>,
}

/// 一次集合操作。 / One set operation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ActivationOpSpec {
    /// 来源。 / The source.
    pub source: ActivationSourceSpec,
    /// 操作：`activate` / `deactivate` / `insertActivate` / `insertDeactivate`。
    /// The operation: `activate` / `deactivate` / `insertActivate` / `insertDeactivate`.
    pub op: String,
}

/// 激活集合探针请求。 / The activation-set probe request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ActivationSetRequest {
    /// 建快照用的变量。 / The variables used to build the snapshot.
    #[serde(default)]
    pub variables: Vec<ActivationVariableSpec>,
    /// 是否给快照加一条约束（默认加）。 / Whether the snapshot gets one constraint (default yes).
    #[serde(default)]
    pub with_constraint: Option<bool>,
    /// 是否从快照起手（默认是）。 / Whether to start from the snapshot (default yes).
    #[serde(default)]
    pub from_snapshot: Option<bool>,
    /// 依次施加的操作。 / The operations, applied in order.
    #[serde(default)]
    pub ops: Vec<ActivationOpSpec>,
    /// 要问 `is_active` 的来源。 / The sources to ask `is_active` about.
    #[serde(default)]
    pub probes: Vec<ActivationSourceSpec>,
}

/// 一条记录的读面。 / One record's readable face.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ActivationRecordView {
    /// 来源种类。 / The source kind.
    pub kind: String,
    /// 来源稳定身份。 / The source's stable identity.
    pub source_stable_id: String,
    /// 状态（`ACTIVATED` / `DEACTIVATED`）。 / The state.
    pub state: String,
    /// 记录稳定身份（含状态前缀）。 / The record's stable identity (with the state prefix).
    pub stable_id: String,
}

/// 激活集合探针结论。 / The activation-set probe verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ActivationSetReport {
    /// 每条记录的读面（按插入顺序）。 / Each record's readable face, in insertion order.
    pub records: Vec<ActivationRecordView>,
    /// 记录条数。 / The record count.
    pub len: u32,
    /// 是否为空。 / Whether the set is empty.
    pub is_empty: bool,
    /// 激活的原始约束（字典序）。 / The active original constraints (lexicographic).
    pub active_constraints: Vec<String>,
    /// `to_assumptions` 产出的身份（按顺序）。 / The identities produced by `to_assumptions`.
    pub assumptions: Vec<String>,
    /// 每个探针的 `is_active` 结果。 / Each probe's `is_active` result.
    pub probe_active: Vec<bool>,
    /// 记录稳定身份里是否有重复。 / Whether the record identities contain a duplicate.
    pub duplicate_stable_ids: Vec<String>,
    /// 八条规则属性的读数。 / The eight rule readings.
    pub insert_overrides_and_reorders: bool,
    pub absence_means_inactive: bool,
    pub the_typed_entry_points_validate_but_insert_does_not: bool,
    pub the_state_enters_the_stable_identity: bool,
    pub two_variants_share_one_stable_identity: bool,
    pub constraints_never_become_assumptions: bool,
    pub active_constraints_only_reads_constraints: bool,
    pub from_snapshot_activates_by_domain_shape: bool,
}

/// 造一个来源。 / Build a source.
fn source_of(spec: &ActivationSourceSpec) -> Result<DiagnosticSource> {
    let id = spec.id.trim();
    Ok(match spec.kind.as_str() {
        "constraint" => DiagnosticSource::Constraint { id: StableConstraintId(spec.id.clone()) },
        "lowerBound" => DiagnosticSource::VariableLowerBound {
            variable_id: StableVariableId(spec.id.clone()) },
        "upperBound" => DiagnosticSource::VariableUpperBound {
            variable_id: StableVariableId(spec.id.clone()) },
        "variableBound" => {
            let side = match spec.side.as_deref() {
                Some("lower") | None => BoundSide::Lower,
                Some("upper") => BoundSide::Upper,
                Some(other) => return Err(BridgeError::new("InvalidInput", &format!(
                    "activation set failed validation: unknown bound side `{other}`"))),
            };
            DiagnosticSource::VariableBound {
                bound: VariableBoundRef { variable_id: StableVariableId(spec.id.clone()),
                    side } }
        }
        "sparseDomain" => DiagnosticSource::SparseDomain {
            variable_id: StableVariableId(spec.id.clone()) },
        other => {
            let _ = id;
            return Err(BridgeError::new("InvalidInput", &format!(
                "activation set failed validation: unknown source kind `{other}`")));
        }
    })
}

/// 造快照。 / Build the snapshot.
fn snapshot_of(request: &ActivationSetRequest) -> Result<ConstraintProgrammingSnapshot> {
    let mut model = ConstraintProgrammingModel::new("activation-set");
    let mut first = None;
    for spec in &request.variables {
        if spec.id.trim().is_empty() {
            return Err(BridgeError::new("InvalidInput",
                "activation set failed validation: variable IDs must not be blank"));
        }
        let domain = if spec.values.is_empty() {
            IntegerDomain::range(0, spec.upper.unwrap_or(10))
        } else {
            IntegerDomain::values(spec.values.iter().copied())
        }
        .map_err(|error| BridgeError::new("InvalidInput", &format!(
            "activation set failed validation: {error}")))?;
        let variable = IntegerVariable::new(spec.id.as_str());
        model.register_variable(variable.clone(), domain).map_err(|error| BridgeError::new(
            "InvalidInput", &format!("activation set failed validation: {error}")))?;
        first.get_or_insert(variable);
    }
    if request.with_constraint.unwrap_or(true) {
        let expression = match first {
            Some(variable) => IntegerExpression::variable(variable),
            None => IntegerExpression::constant(0),
        };
        model.add_constraint(ConstraintDefinition::new("c0",
            ConstraintProgrammingConstraint::integer(
                expression,
                ospf_rust_core::model::constraint_programming::IntegerRelation::LessOrEqual, 10)))
            .map_err(|error| BridgeError::new("InvalidInput", &format!(
                "activation set failed validation: {error}")))?;
    }
    model.freeze().map_err(|error| BridgeError::new("InvalidInput", &format!(
        "activation set failed validation: {error}")))
}

/// 走一遍激活集合探针。非法输入是**错误**。 / Walk the activation-set probe.
pub fn verify_activation_set(request: &ActivationSetRequest) -> Result<ActivationSetReport> {
    if request.ops.len() > 32 {
        return Err(BridgeError::new("InvalidInput",
            "activation set failed validation: at most 32 operations are accepted"));
    }
    let findings = canonical_findings();
    let snapshot = snapshot_of(request)?;
    let mut set = if request.from_snapshot.unwrap_or(true) {
        DiagnosticActivationSet::from_snapshot(&snapshot)
    } else {
        DiagnosticActivationSet::new()
    };
    for op in &request.ops {
        let source = source_of(&op.source)?;
        match op.op.as_str() {
            "activate" => {
                set.activate_source(source).map_err(|error| BridgeError::new("InvalidInput",
                    &format!("activation set failed validation: {error}")))?;
            }
            "deactivate" => {
                set.deactivate_source(source).map_err(|error| BridgeError::new("InvalidInput",
                    &format!("activation set failed validation: {error}")))?;
            }
            // **不校验**的两个入口：空 ID 也能进集合。 / The two **unvalidated** entry points: a blank ID gets in.
            "insertActivate" => set.insert(DiagnosticActivation::activate(source)),
            "insertDeactivate" => set.insert(DiagnosticActivation::deactivate(source)),
            other => return Err(BridgeError::new("InvalidInput", &format!(
                "activation set failed validation: unknown operation `{other}`"))),
        }
    }
    let mut probes = Vec::with_capacity(request.probes.len());
    for probe in &request.probes {
        probes.push(set.is_active(&source_of(probe)?));
    }
    let records = set.entries().iter().map(|entry| ActivationRecordView {
        kind: entry.source.kind().to_owned(),
        source_stable_id: entry.source.stable_id(),
        state: match entry.state {
            ActivationState::Activated => "ACTIVATED".to_owned(),
            ActivationState::Deactivated => "DEACTIVATED".to_owned(),
        },
        stable_id: entry.stable_id(),
    }).collect::<Vec<_>>();
    let mut seen = BTreeSet::new();
    let duplicate_stable_ids = records.iter()
        .filter(|record| !seen.insert(record.stable_id.clone()))
        .map(|record| record.stable_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    Ok(ActivationSetReport {
        records,
        len: u32::try_from(set.len()).unwrap_or(u32::MAX),
        is_empty: set.is_empty(),
        active_constraints: set.active_constraints().into_iter()
            .map(|id| id.0).collect::<Vec<_>>(),
        assumptions: set.to_assumptions(&snapshot).into_iter()
            .map(|assumption| assumption.stable_id().0).collect::<Vec<_>>(),
        probe_active: probes,
        duplicate_stable_ids,
        insert_overrides_and_reorders: findings.insert_overrides_and_reorders,
        absence_means_inactive: findings.absence_means_inactive,
        the_typed_entry_points_validate_but_insert_does_not:
            findings.the_typed_entry_points_validate_but_insert_does_not,
        the_state_enters_the_stable_identity: findings.the_state_enters_the_stable_identity,
        two_variants_share_one_stable_identity: findings.two_variants_share_one_stable_identity,
        constraints_never_become_assumptions: findings.constraints_never_become_assumptions,
        active_constraints_only_reads_constraints: findings.active_constraints_only_reads_constraints,
        from_snapshot_activates_by_domain_shape: findings.from_snapshot_activates_by_domain_shape,
    })
}

/// 八条规则属性的内部固定探针。 / The eight rule properties measured by fixed internal probes.
struct Findings {
    insert_overrides_and_reorders: bool,
    absence_means_inactive: bool,
    the_typed_entry_points_validate_but_insert_does_not: bool,
    the_state_enters_the_stable_identity: bool,
    two_variants_share_one_stable_identity: bool,
    constraints_never_become_assumptions: bool,
    active_constraints_only_reads_constraints: bool,
    from_snapshot_activates_by_domain_shape: bool,
}

/// 造一份最小快照（一个区间域变量 + 一个稀疏域变量 + 一条约束）。
/// Build a minimal snapshot (one range variable, one sparse variable, one constraint).
fn probe_snapshot() -> ConstraintProgrammingSnapshot {
    let mut model = ConstraintProgrammingModel::new("activation-probe");
    let x = IntegerVariable::new("x");
    let y = IntegerVariable::new("y");
    model.register_variable(x.clone(), IntegerDomain::range(0, 5).expect("domain"))
        .expect("register");
    model.register_variable(y, IntegerDomain::values([1, 4, 9]).expect("domain"))
        .expect("register");
    model.add_constraint(ConstraintDefinition::new("c0",
        ConstraintProgrammingConstraint::integer(IntegerExpression::variable(x),
            ospf_rust_core::model::constraint_programming::IntegerRelation::LessOrEqual, 5)))
        .expect("constraint");
    model.freeze().expect("freeze")
}

/// 用固定探针测出全部规则属性——不读调用方的请求值。
/// Measure every rule property with fixed probes; the caller's request values are not read.
fn canonical_findings() -> Findings {
    let snapshot = probe_snapshot();
    // 1) `insert` 覆盖并移位。 / `insert` overrides and re-orders.
    let insert_overrides_and_reorders = {
        let mut set = DiagnosticActivationSet::new();
        set.insert(DiagnosticActivation::activate(DiagnosticSource::VariableLowerBound {
            variable_id: StableVariableId("a".to_owned()) }));
        set.insert(DiagnosticActivation::activate(DiagnosticSource::VariableLowerBound {
            variable_id: StableVariableId("b".to_owned()) }));
        // 再次插入 `a` 的**停用**记录：条数不变、顺序变成 `b, a`、状态变停用。 
        // Re-inserting `a` as deactivated: the count stays, the order becomes `b, a` and the state flips.
        set.insert(DiagnosticActivation::deactivate(DiagnosticSource::VariableLowerBound {
            variable_id: StableVariableId("a".to_owned()) }));
        let entries = set.entries();
        entries.len() == 2
            && entries[0].source == DiagnosticSource::VariableLowerBound {
                variable_id: StableVariableId("b".to_owned()) }
            && entries[1].source == DiagnosticSource::VariableLowerBound {
                variable_id: StableVariableId("a".to_owned()) }
            && entries[1].state == ActivationState::Deactivated
    };
    // 2) 缺席即未激活。 / Absence means inactive.
    let absence_means_inactive = {
        let mut set = DiagnosticActivationSet::new();
        let source = DiagnosticSource::Constraint { id: StableConstraintId("c0".to_owned()) };
        // 从未出现 → 未激活。 / Never present → inactive.
        let absent = !set.is_active(&source);
        // 停用记录也算"出现过"，但仍然是未激活。 / A deactivated record counts as present yet is still inactive.
        set.insert(DiagnosticActivation::deactivate(source.clone()));
        absent && !set.is_active(&source) && set.len() == 1
    };
    // 3) 带类型的入口校验，`insert` 不校验。 / The typed entry points validate, `insert` does not.
    let the_typed_entry_points_validate_but_insert_does_not = {
        let mut set = DiagnosticActivationSet::new();
        let blank = DiagnosticSource::Constraint { id: StableConstraintId("  ".to_owned()) };
        let typed = set.activate_source(blank.clone()).is_err();
        set.insert(DiagnosticActivation::activate(blank));
        // 经 `insert` 进来之后条数变了，`to_assumptions` 也不会替它伪造 assumption。 
        // After entering through `insert` the count changes and `to_assumptions` still fabricates nothing for it.
        typed && set.len() == 1 && set.to_assumptions(&snapshot).is_empty()
    };
    // 4) 状态进身份。 / The state enters the identity.
    let the_state_enters_the_stable_identity = {
        let source = DiagnosticSource::VariableUpperBound {
            variable_id: StableVariableId("x".to_owned()) };
        let activated = DiagnosticActivation::activate(source.clone()).stable_id();
        let deactivated = DiagnosticActivation::deactivate(source).stable_id();
        activated == "activated:variable:x:upper"
            && deactivated == "deactivated:variable:x:upper"
            && activated != deactivated
    };
    // 5) 两个变体共享一个稳定身份。 / Two variants share one stable identity.
    let two_variants_share_one_stable_identity = {
        let lower = DiagnosticSource::VariableLowerBound {
            variable_id: StableVariableId("x".to_owned()) };
        let bound = DiagnosticSource::VariableBound {
            bound: VariableBoundRef { variable_id: StableVariableId("x".to_owned()),
                side: BoundSide::Lower } };
        // 身份相同、值不等、不可互相覆盖。 
        // Same identity, unequal values, no mutual override.
        let same_identity = lower.stable_id() == bound.stable_id()
            && lower.stable_id() == "variable:x:lower";
        let mut set = DiagnosticActivationSet::new();
        set.insert(DiagnosticActivation::activate(lower.clone()));
        set.insert(DiagnosticActivation::activate(bound.clone()));
        same_identity && lower != bound && set.len() == 2
            && set.entries()[0].stable_id() == set.entries()[1].stable_id()
    };
    // 6) 约束永不变成 assumption。 / Constraints never become assumptions.
    let constraints_never_become_assumptions = {
        let set = DiagnosticActivationSet::from_snapshot(&snapshot);
        let assumptions = set.to_assumptions(&snapshot);
        // 快照里有一条约束、两个变量（其中一个稀疏域），assumption 只来自变量边界与稀疏域。 
        // The snapshot has one constraint and two variables (one sparse); assumptions come only from bounds and the
        // sparse domain.
        !assumptions.is_empty()
            && assumptions.iter().all(|assumption| {
                let id = assumption.stable_id().0;
                id.starts_with("lower-bound/") || id.starts_with("upper-bound/")
                    || id.starts_with("sparse-domain/")
            })
    };
    // 7) `active_constraints` 只收约束。 / `active_constraints` only reads constraints.
    let active_constraints_only_reads_constraints = {
        let set = DiagnosticActivationSet::from_snapshot(&snapshot);
        set.active_constraints().into_iter().map(|id| id.0).collect::<Vec<_>>()
            == vec!["c0".to_owned()]
    };
    // 8) `from_snapshot` 的激活面由域形态决定。 / `from_snapshot`'s surface follows the domain shape.
    let from_snapshot_activates_by_domain_shape = {
        let set = DiagnosticActivationSet::from_snapshot(&snapshot);
        let kinds = set.entries().iter().map(|entry| entry.source.kind())
            .collect::<Vec<_>>();
        // 一条约束 + 每个变量两条边界 + 稀疏域变量额外一条 = 1 + 4 + 1 = 6。 
        // One constraint plus two bounds per variable plus one extra for the sparse variable: 1 + 4 + 1 = 6.
        kinds == vec!["constraint", "variable-lower-bound", "variable-upper-bound",
            "variable-lower-bound", "variable-upper-bound", "sparse-domain"]
            && set.entries().iter().all(|entry| entry.state == ActivationState::Activated)
    };
    Findings {
        insert_overrides_and_reorders,
        absence_means_inactive,
        the_typed_entry_points_validate_but_insert_does_not,
        the_state_enters_the_stable_identity,
        two_variants_share_one_stable_identity,
        constraints_never_become_assumptions,
        active_constraints_only_reads_constraints,
        from_snapshot_activates_by_domain_shape,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source(kind: &str, id: &str) -> ActivationSourceSpec {
        ActivationSourceSpec { kind: kind.to_owned(), id: id.to_owned(), side: None }
    }

    fn op(source: ActivationSourceSpec, op: &str) -> ActivationOpSpec {
        ActivationOpSpec { source, op: op.to_owned() }
    }

    fn variable(id: &str) -> ActivationVariableSpec {
        ActivationVariableSpec { id: id.to_owned(), values: Vec::new(), upper: None }
    }

    fn request() -> ActivationSetRequest {
        ActivationSetRequest { variables: vec![variable("x")], with_constraint: Some(true),
            from_snapshot: Some(false), ops: Vec::new(), probes: Vec::new() }
    }

    #[test]
    fn an_empty_set_is_empty_and_activates_nothing() {
        let report = verify_activation_set(&request()).unwrap();
        assert_eq!(report.len, 0);
        assert!(report.is_empty);
        assert!(report.records.is_empty());
        assert!(report.active_constraints.is_empty());
        assert!(report.assumptions.is_empty());
        assert!(report.insert_overrides_and_reorders);
        assert!(report.absence_means_inactive);
        assert!(report.the_typed_entry_points_validate_but_insert_does_not);
        assert!(report.the_state_enters_the_stable_identity);
        assert!(report.two_variants_share_one_stable_identity);
        assert!(report.constraints_never_become_assumptions);
        assert!(report.active_constraints_only_reads_constraints);
        assert!(report.from_snapshot_activates_by_domain_shape);
    }

    #[test]
    fn from_snapshot_activates_by_domain_shape() {
        // 两个变量（第二个是稀疏域）+ 一条约束 → 6 条记录。 
        // Two variables (the second sparse) plus one constraint give six records.
        let report = verify_activation_set(&ActivationSetRequest {
            variables: vec![variable("x"), ActivationVariableSpec { id: "y".to_owned(),
                values: vec![1, 4, 9], upper: None }],
            with_constraint: Some(true), from_snapshot: Some(true), ops: Vec::new(),
            probes: Vec::new() }).unwrap();
        assert_eq!(report.len, 6);
        assert_eq!(report.records.iter().map(|record| record.kind.clone()).collect::<Vec<_>>(),
            vec!["constraint", "variable-lower-bound", "variable-upper-bound",
                "variable-lower-bound", "variable-upper-bound", "sparse-domain"]);
        assert!(report.records.iter().all(|record| record.state == "ACTIVATED"));
        assert_eq!(report.active_constraints, vec!["c0".to_owned()]);
        assert!(!report.assumptions.is_empty());
    }

    #[test]
    fn insert_overrides_the_state_and_moves_the_record() {
        let report = verify_activation_set(&ActivationSetRequest {
            ops: vec![op(source("lowerBound", "a"), "insertActivate"),
                op(source("lowerBound", "b"), "insertActivate"),
                op(source("lowerBound", "a"), "insertDeactivate")], ..request() }).unwrap();
        assert_eq!(report.len, 2);
        assert_eq!(report.records[0].source_stable_id, "variable:b:lower");
        assert_eq!(report.records[1].source_stable_id, "variable:a:lower");
        assert_eq!(report.records[1].state, "DEACTIVATED");
        assert_eq!(report.records[1].stable_id, "deactivated:variable:a:lower");
    }

    #[test]
    fn absence_means_inactive() {
        let report = verify_activation_set(&ActivationSetRequest {
            ops: vec![op(source("constraint", "c0"), "insertDeactivate")],
            probes: vec![source("constraint", "c0"), source("constraint", "c9")],
            ..request() }).unwrap();
        // 停用记录也算出现过，但两个探针都不是激活。 
        // The deactivated record counts as present while neither probe is active.
        assert_eq!(report.probe_active, vec![false, false]);
        assert_eq!(report.len, 1);
    }

    #[test]
    fn the_typed_entry_points_validate_while_insert_does_not() {
        // 空白 ID 经 `activate` 被拒。 / A blank ID is refused through `activate`.
        assert!(verify_activation_set(&ActivationSetRequest {
            ops: vec![op(source("constraint", "  "), "activate")], ..request() }).unwrap_err()
            .message.contains("constraint ID must not be blank"));
        // 同一份空白 ID 经 `insertActivate` 进来了。 
        // The same blank ID gets in through `insertActivate`.
        let inserted = verify_activation_set(&ActivationSetRequest {
            ops: vec![op(source("constraint", "  "), "insertActivate")], ..request() }).unwrap();
        assert_eq!(inserted.len, 1);
        assert!(inserted.assumptions.is_empty());
    }

    #[test]
    fn two_variants_share_one_stable_identity() {
        let report = verify_activation_set(&ActivationSetRequest {
            ops: vec![op(source("lowerBound", "x"), "insertActivate"),
                op(ActivationSourceSpec { kind: "variableBound".to_owned(), id: "x".to_owned(),
                    side: Some("lower".to_owned()) }, "insertActivate")], ..request() }).unwrap();
        // 两条记录、同一个稳定身份——`insert` 无法用身份去重。 
        // Two records with one stable identity: `insert` cannot dedup by identity.
        assert_eq!(report.len, 2);
        assert_eq!(report.records[0].stable_id, report.records[1].stable_id);
        assert_eq!(report.records[0].kind, "variable-lower-bound");
        assert_eq!(report.records[1].kind, "variable-bound");
        assert_eq!(report.duplicate_stable_ids, vec!["activated:variable:x:lower".to_owned()]);
    }

    #[test]
    fn assumptions_never_come_from_constraints() {
        let report = verify_activation_set(&ActivationSetRequest {
            variables: vec![variable("x"), ActivationVariableSpec { id: "y".to_owned(),
                values: vec![2, 8], upper: None }],
            with_constraint: Some(true), from_snapshot: Some(true),
            ops: vec![op(source("constraint", "c0"), "insertActivate")],
            probes: Vec::new() }).unwrap();
        // 约束被激活两次也只是约束，不进 assumption。 
        // Activating the constraint twice keeps it a constraint and adds no assumption.
        assert!(report.assumptions.iter().all(|id| !id.starts_with("constraint/")), "{:?}",
            report.assumptions);
        assert!(report.assumptions.iter().any(|id| id.starts_with("sparse-domain/")));
        assert!(report.assumptions.iter().any(|id| id.starts_with("lower-bound/")));
        assert!(report.assumptions.iter().any(|id| id.starts_with("upper-bound/")));
    }

    #[test]
    fn active_constraints_only_reads_constraint_sources() {
        let report = verify_activation_set(&ActivationSetRequest {
            ops: vec![op(source("lowerBound", "x"), "activate")], ..request() }).unwrap();
        // 没有激活任何约束 → 空集，尽管有一条下界记录。 
        // No constraint is active, so the set is empty even though a lower-bound record exists.
        assert!(report.active_constraints.is_empty());
        assert_eq!(report.len, 1);
    }

    #[test]
    fn bad_input_is_refused() {
        assert!(verify_activation_set(&ActivationSetRequest {
            ops: vec![op(source("nope", "x"), "activate")], ..request() }).unwrap_err()
            .message.contains("unknown source kind"));
        assert!(verify_activation_set(&ActivationSetRequest {
            ops: vec![op(ActivationSourceSpec { kind: "variableBound".to_owned(),
                id: "x".to_owned(), side: Some("middle".to_owned()) }, "activate")],
            ..request() }).unwrap_err().message.contains("unknown bound side"));
        assert!(verify_activation_set(&ActivationSetRequest {
            ops: vec![op(source("constraint", "c0"), "nope")], ..request() }).unwrap_err()
            .message.contains("unknown operation"));
        assert!(verify_activation_set(&ActivationSetRequest {
            variables: vec![variable("  ")], ..request() }).unwrap_err().message
            .contains("variable IDs must not be blank"));
        assert!(verify_activation_set(&ActivationSetRequest {
            ops: (0..33).map(|index| op(source("constraint", &format!("c{index}")), "insertActivate"))
                .collect(), ..request() }).unwrap_err().message.contains("at most 32 operations"));
    }

    /// 守门：DTO 必须是 camelCase 且拒绝未知字段。 / A guard: camelCase wire, unknown fields refused.
    #[test]
    fn the_report_wire_is_camel_case_and_rejects_unknown_fields() {
        let report = verify_activation_set(&request()).unwrap();
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"activeConstraints\""), "{json}");
        assert!(json.contains("\"duplicateStableIds\""), "{json}");
        assert!(json.contains("\"twoVariantsShareOneStableIdentity\""), "{json}");
        assert!(!json.contains("active_constraints"), "{json}");
        let mut copy: serde_json::Value = serde_json::from_str(&json).unwrap();
        copy["unexpected"] = serde_json::json!(1);
        assert!(serde_json::from_str::<ActivationSetReport>(&copy.to_string()).is_err());
    }
}
