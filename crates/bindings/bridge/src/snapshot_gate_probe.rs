//! 续挖第 71 单：CP 快照自己的**读侧闸门**——`validate_identity` 的七道检查、`validate_hint` 的部分赋值口径、
//! `objective_variables` 的引用范围、以及 artifact codec 的三道关
//! （`model/constraint_programming/snapshot.rs` 的 `ConstraintProgrammingSnapshot`）
//! Continuation, delivery 71: the CP snapshot's own **read-side gates** — the seven checks in `validate_identity`, the
//! partial-assignment semantics of `validate_hint`, the reference scope of `objective_variables`, and the artifact codec's
//! three checks (`ConstraintProgrammingSnapshot` in `model/constraint_programming/snapshot.rs`).
//!
//! 第 70 单量的是 builder 的**写侧**闸门；这一单量**读侧**：一份快照交回来之后，谁能证明它没被改过。八条实测：
//!
//! 1. **已冻结快照上 `validate_identity` 恒真且幂等**（`a_frozen_snapshot_always_validates()`）。
//! 2. **空白身份是第一道闸门**（`a_blank_identity_is_the_first_gate()`）：namespace 先于 schema 版本。
//! 3. **顺序检查有两道，前一道遮蔽后一道**（`the_collection_order_gate_shadows_the_sort_gate()`）：
//!    `canonicalized()` 的整结构比对排在逐集合 `windows(2)` 单调检查**之前**，而 `canonicalized()` 自己会排序，
//!    所以手工打乱顺序只会撞上 `"non-canonical collection order"`，那句 `"not in canonical stable-ID order"`
//!    **观察不到**。
//! 4. **模型名不在指纹里，约束名在**（`the_model_name_is_outside_the_fingerprint_but_constraint_names_are_inside()`）：
//!    改 `name` 后快照仍然合法；改任一约束的 `name` 立刻变成 `"fingerprint does not match canonical content"`。
//! 5. **空白组名在指纹比对之前被拒**（`a_blank_group_name_is_refused_before_the_fingerprint()`）。
//! 6. **`validate_hint` 是部分赋值闸门**（`a_hint_may_cover_less_than_every_variable()`）：不要求覆盖全部变量、
//!    不查约束、**空映射恒过**；只查"已注册 + 在域内"。
//! 7. **`objective_variables` 只看目标**（`objective_variables_ignore_constraints()`）：无目标时为空集，与约束引用
//!    无关。
//! 8. **artifact codec 三道关**（`the_artifact_codec_checks_three_things()`）：schema 版本、kind、digest；
//!    且解码后要求输入**逐字节等于**重新编码的结果。
//!
//! Delivery 70 measured the builder's **write-side** gates; this one measures the **read side**: once a snapshot comes
//! back, what proves it was not edited. **`validate_identity` is always true and idempotent on a frozen snapshot.** **A
//! blank identity is the first gate** (namespace before schema version). **The collection-order gate shadows the sort
//! gate** — `canonicalized()`'s whole-structure comparison runs *before* the per-collection `windows(2)` monotonicity
//! check and itself sorts, so shuffling by hand only ever hits `"non-canonical collection order"` and the
//! `"not in canonical stable-ID order"` message is **unobservable**. **The model name sits outside the fingerprint while
//! constraint names sit inside it.** **A blank group name is refused before the fingerprint comparison.** **`validate_hint`
//! is a partial-assignment gate** — it need not cover every variable, never evaluates constraints, and an **empty map always
//! passes**. **`objective_variables` only looks at the objective.** And **the artifact codec checks three things** (schema
//! version, kind, digest) and then requires the input to be **byte-for-byte** the re-encoding.

use std::collections::BTreeMap;

use ospf_rust_core::model::constraint_programming::{
    ConstraintDefinition, ConstraintProgrammingConstraint, ConstraintProgrammingModel,
    ConstraintProgrammingSnapshot, IntegerDomain, IntegerExpression, IntegerObjective,
    IntegerRelation, IntegerVariable,
};
use ospf_rust_core::solver::report::StableVariableId;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 一条赋值条目。 / One assignment entry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SnapshotValueSpec {
    /// 稳定变量 ID。 / The stable variable ID.
    pub id: String,
    /// 整数值。 / The integer value.
    pub value: i64,
}

/// 快照读侧闸门探针请求。 / The snapshot read-gate probe request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SnapshotGateRequest {
    /// 冻结后要施加的篡改（空表示原样）。 / Tampering applied after freezing (empty means as-is).
    #[serde(default)]
    pub mutations: Vec<String>,
    /// `validate_hint` 的输入。 / The input to `validate_hint`.
    #[serde(default)]
    pub hint: Vec<SnapshotValueSpec>,
    /// `objective_value` 的输入（空表示不调用）。 / The input to `objective_value` (empty means skip).
    #[serde(default)]
    pub evaluation: Vec<SnapshotValueSpec>,
}

/// 快照读侧闸门探针结论。 / The snapshot read-gate probe verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SnapshotGateReport {
    /// 施加的篡改。 / The tampering that was applied.
    pub mutations: Vec<String>,
    /// `validate_identity` 是否通过。 / Whether `validate_identity` passed.
    pub identity_accepted: bool,
    /// `validate_identity` 的拒绝文案。 / The refusal message.
    #[serde(default)]
    pub identity_refusal: Option<String>,
    /// 再校验一次是否仍给出同样结论（幂等）。 / Whether a second check agrees (idempotence).
    pub identity_is_idempotent: bool,
    /// 快照的模型名。 / The snapshot's model name.
    pub model_name: String,
    /// 快照指纹。 / The snapshot fingerprint.
    pub fingerprint: String,
    /// 变量稳定 ID（当前顺序）。 / The variable stable IDs in their current order.
    pub variable_ids: Vec<String>,
    /// 约束稳定 ID（当前顺序）。 / The constraint stable IDs in their current order.
    pub constraint_ids: Vec<String>,
    /// 约束显示名。 / The constraint display names.
    pub constraint_names: Vec<String>,
    /// 组元数据（`id:name`）。 / The group metadata (`id:name`).
    pub groups: Vec<String>,
    /// `validate_hint` 是否通过（未给 hint 时为空）。 / Whether `validate_hint` passed (absent when no hint was given).
    #[serde(default)]
    pub hint_accepted: Option<bool>,
    /// `validate_hint` 的拒绝文案。 / The `validate_hint` refusal message.
    #[serde(default)]
    pub hint_refusal: Option<String>,
    /// `objective_variables` 的稳定 ID。 / The stable IDs from `objective_variables`.
    pub objective_variables: Vec<String>,
    /// `objective_value` 的读数（未调用时为空）。 / The `objective_value` reading (absent when skipped).
    #[serde(default)]
    pub objective_value: Option<i64>,
    /// `objective_value` 的拒绝文案。 / The `objective_value` refusal message.
    #[serde(default)]
    pub objective_value_refusal: Option<String>,
    /// artifact 重新编码是否成功。 / Whether the artifact re-encoded.
    pub artifact_encoded: bool,
    /// artifact 的 digest。 / The artifact digest.
    #[serde(default)]
    pub artifact_digest: Option<String>,
    /// artifact 往返（编码→解码）是否一致。 / Whether the artifact round-trips.
    pub artifact_round_trips: bool,
    /// 八条规则属性的读数。 / The eight rule readings.
    pub a_frozen_snapshot_always_validates: bool,
    pub a_blank_identity_is_the_first_gate: bool,
    pub the_collection_order_gate_shadows_the_sort_gate: bool,
    pub the_model_name_is_outside_the_fingerprint_but_constraint_names_are_inside: bool,
    pub a_blank_group_name_is_refused_before_the_fingerprint: bool,
    pub a_hint_may_cover_less_than_every_variable: bool,
    pub objective_variables_ignore_constraints: bool,
    pub the_artifact_codec_checks_three_things: bool,
}

/// 造一份带目标的两变量快照。 / Build a two-variable snapshot with an objective.
fn base_snapshot() -> ConstraintProgrammingSnapshot {
    let x = IntegerVariable::new("x");
    let y = IntegerVariable::new("y");
    let mut model = ConstraintProgrammingModel::new("snapshot-gate");
    model.register_variable(x.clone(), IntegerDomain::range(0, 4).expect("domain"))
        .expect("variable");
    model.register_variable(y.clone(), IntegerDomain::range(0, 4).expect("domain"))
        .expect("variable");
    model.add_constraint(ConstraintDefinition::new("capacity",
        ConstraintProgrammingConstraint::integer(
            IntegerExpression::variable(x.clone()), IntegerRelation::LessOrEqual, 4))
        .with_group("capacity")).expect("constraint");
    model.add_constraint(ConstraintDefinition::new("floor",
        ConstraintProgrammingConstraint::integer(
            IntegerExpression::variable(y.clone()), IntegerRelation::GreaterOrEqual, 0))
        .with_group("capacity")).expect("constraint");
    model.ensure_constraint_group(1, "capacity").expect("group");
    model.set_objective(IntegerObjective::maximize_with_id("profit",
        IntegerExpression::variable(x)));
    model.freeze().expect("snapshot")
}

/// 施加一条篡改。 / Apply one tampering.
fn mutate(snapshot: &mut ConstraintProgrammingSnapshot, mutation: &str) -> Result<()> {
    match mutation {
        "blankNamespace" => snapshot.identity_namespace = "  ".to_owned(),
        "blankSchemaVersion" => snapshot.identity_schema_version = "  ".to_owned(),
        "reverseVariables" => snapshot.variables.reverse(),
        "reverseConstraints" => snapshot.constraints.reverse(),
        "blankGroupName" => {
            if let Some((_, name)) = snapshot.constraint_groups.iter_mut().next() {
                *name = "  ".to_owned();
            }
        }
        "renameModel" => snapshot.name = "renamed".to_owned(),
        "renameConstraint" => {
            if let Some(constraint) = snapshot.constraints.first_mut() {
                constraint.name = "renamed".to_owned();
            }
        }
        "duplicateVariableStableId" => {
            // 把第二个变量改成与第一个**同一个稳定 ID**，本地 ID 仍不同（所以两道去重里撞的是稳定那道）。
            // Give the second variable the **same stable ID** as the first while keeping a distinct local ID, so the
            // stable-ID duplicate is the one that fires.
            if snapshot.variables.len() >= 2 {
                let first_stable = snapshot.variables[0].variable.stable_id.clone();
                let local = snapshot.variables[1].variable.id;
                snapshot.variables[1].variable = IntegerVariable::with_ids(local, first_stable);
            }
        }
        "dropVariable" => {
            snapshot.variables.pop();
        }
        "blankStableId" => {
            if let Some(variable) = snapshot.variables.first_mut() {
                variable.variable = IntegerVariable::with_ids(variable.variable.id.clone(), "  ");
            }
        }
        other => return Err(BridgeError::new("InvalidInput", &format!(
            "snapshot gate failed validation: unknown mutation `{other}`"))),
    }
    Ok(())
}

/// 把赋值条目变成映射（重复 ID 拒）。 / Turn value specs into a map (duplicates refused).
fn values_of(entries: &[SnapshotValueSpec]) -> Result<BTreeMap<StableVariableId, i64>> {
    let mut values = BTreeMap::new();
    for entry in entries {
        if entry.id.trim().is_empty() {
            return Err(BridgeError::new("InvalidInput",
                "snapshot gate failed validation: assignment IDs must not be blank"));
        }
        if values.insert(StableVariableId(entry.id.clone()), entry.value).is_some() {
            return Err(BridgeError::new("InvalidInput",
                "snapshot gate failed validation: the assignment repeats a stable ID"));
        }
    }
    Ok(values)
}

/// 走一遍快照读侧闸门探针。非法输入是**错误**。 / Walk the snapshot read-gate probe.
pub fn verify_snapshot_gates(request: &SnapshotGateRequest) -> Result<SnapshotGateReport> {
    if request.mutations.len() > 16 || request.hint.len() > 16 || request.evaluation.len() > 16 {
        return Err(BridgeError::new("InvalidInput",
            "snapshot gate failed validation: at most 16 entries are accepted"));
    }
    let mut snapshot = base_snapshot();
    for mutation in &request.mutations {
        mutate(&mut snapshot, mutation)?;
    }
    let findings = canonical_findings();

    let first = snapshot.validate_identity();
    let identity_accepted = first.is_ok();
    let identity_refusal = first.as_ref().err().map(|error| error.to_string());
    // 幂等：再校验一次必须给出同样的结论与同样的文案。
    // Idempotence: a second check must give the same verdict and the same message.
    let second = snapshot.validate_identity();
    let identity_is_idempotent = second.is_ok() == identity_accepted
        && second.as_ref().err().map(|error| error.to_string()) == identity_refusal;

    let hint = if request.hint.is_empty() {
        None
    } else {
        let values = values_of(&request.hint)?;
        Some(snapshot.validate_hint(&values).map_err(|error| error.to_string()))
    };
    let (hint_accepted, hint_refusal) = match &hint {
        Some(Ok(())) => (Some(true), None),
        Some(Err(refusal)) => (Some(false), Some(refusal.clone())),
        None => (None, None),
    };
    let evaluation = if request.evaluation.is_empty() {
        None
    } else {
        Some(snapshot.objective_value(&values_of(&request.evaluation)?)
            .map_err(|error| error.to_string()))
    };
    let (objective_value, objective_value_refusal) = match &evaluation {
        Some(Ok(value)) => (*value, None),
        Some(Err(refusal)) => (None, Some(refusal.clone())),
        None => (None, None),
    };

    // artifact codec：只在快照本身合法时才有意义。 / The artifact codec only means something on a legal snapshot.
    let (artifact_encoded, artifact_digest, artifact_round_trips) =
        match snapshot.to_artifact() {
            Ok(artifact) => {
                let digest = artifact.artifact_digest.value.clone();
                let round_trips = artifact.to_canonical_json().is_ok_and(|bytes|
                    ConstraintProgrammingSnapshot::from_canonical_json(&bytes).is_ok());
                (true, Some(digest), round_trips)
            }
            Err(_) => (false, None, false),
        };

    Ok(SnapshotGateReport {
        mutations: request.mutations.clone(),
        identity_accepted,
        identity_refusal,
        identity_is_idempotent,
        model_name: snapshot.name.clone(),
        fingerprint: snapshot.fingerprint.value.clone(),
        variable_ids: snapshot.variables.iter()
            .map(|entry| entry.variable.stable_id.0.clone()).collect(),
        constraint_ids: snapshot.constraints.iter().map(|entry| entry.id.0.clone()).collect(),
        constraint_names: snapshot.constraints.iter().map(|entry| entry.name.clone()).collect(),
        groups: snapshot.constraint_groups.iter()
            .map(|(id, name)| format!("{id}:{name}")).collect(),
        hint_accepted,
        hint_refusal,
        objective_variables: snapshot.objective_variables().into_iter()
            .map(|id| id.0).collect(),
        objective_value,
        objective_value_refusal,
        artifact_encoded,
        artifact_digest,
        artifact_round_trips,
        a_frozen_snapshot_always_validates: findings.a_frozen_snapshot_always_validates,
        a_blank_identity_is_the_first_gate: findings.a_blank_identity_is_the_first_gate,
        the_collection_order_gate_shadows_the_sort_gate:
            findings.the_collection_order_gate_shadows_the_sort_gate,
        the_model_name_is_outside_the_fingerprint_but_constraint_names_are_inside:
            findings.the_model_name_is_outside_the_fingerprint_but_constraint_names_are_inside,
        a_blank_group_name_is_refused_before_the_fingerprint:
            findings.a_blank_group_name_is_refused_before_the_fingerprint,
        a_hint_may_cover_less_than_every_variable:
            findings.a_hint_may_cover_less_than_every_variable,
        objective_variables_ignore_constraints: findings.objective_variables_ignore_constraints,
        the_artifact_codec_checks_three_things: findings.the_artifact_codec_checks_three_things,
    })
}

/// 八条规则属性的内部固定探针。 / The eight rule properties measured by fixed internal probes.
struct Findings {
    a_frozen_snapshot_always_validates: bool,
    a_blank_identity_is_the_first_gate: bool,
    the_collection_order_gate_shadows_the_sort_gate: bool,
    the_model_name_is_outside_the_fingerprint_but_constraint_names_are_inside: bool,
    a_blank_group_name_is_refused_before_the_fingerprint: bool,
    a_hint_may_cover_less_than_every_variable: bool,
    objective_variables_ignore_constraints: bool,
    the_artifact_codec_checks_three_things: bool,
}

/// 用固定探针测出全部规则属性——不读调用方的请求值。
/// Measure every rule property with fixed probes; the caller's request values are not read.
fn canonical_findings() -> Findings {
    // 1) 已冻结快照恒真且幂等。 / A frozen snapshot always validates, idempotently.
    let a_frozen_snapshot_always_validates = {
        let snapshot = base_snapshot();
        snapshot.validate_identity().is_ok()
            && snapshot.validate_identity().is_ok()
            && snapshot.validate_identity().is_ok()
    };
    // 2) 空白身份是第一道闸门（namespace 先于 schema）。 / A blank identity is the first gate.
    let a_blank_identity_is_the_first_gate = {
        let mut namespace = base_snapshot();
        namespace.identity_namespace = "  ".to_owned();
        namespace.identity_schema_version = "  ".to_owned();
        // 两者都空时先报 namespace。 / With both blank, the namespace message wins.
        let namespace_wins = namespace.validate_identity()
            .is_err_and(|error| error.to_string().contains("namespace must not be blank"));
        let mut schema = base_snapshot();
        schema.identity_schema_version = "  ".to_owned();
        let schema_alone = schema.validate_identity()
            .is_err_and(|error| error.to_string().contains("schema version must not be blank"));
        namespace_wins && schema_alone
    };
    // 3) 顺序闸门遮蔽排序闸门。 / The collection-order gate shadows the sort gate.
    let the_collection_order_gate_shadows_the_sort_gate = {
        let mut shuffled = base_snapshot();
        shuffled.variables.reverse();
        shuffled.constraints.reverse();
        let message = shuffled.validate_identity()
            .err().map(|error| error.to_string()).unwrap_or_default();
        // 前一道闸门报的是整结构不一致；逐集合的 "not in canonical stable-ID order" **观察不到**。
        // The first gate reports a whole-structure mismatch; the per-collection
        // "not in canonical stable-ID order" message is **unobservable**.
        message.contains("non-canonical collection order")
            && !message.contains("not in canonical stable-ID order")
    };
    // 4) 模型名在外，约束名在内。 / The model name is out; constraint names are in.
    let the_model_name_is_outside_the_fingerprint_but_constraint_names_are_inside = {
        let mut renamed_model = base_snapshot();
        renamed_model.name = "renamed".to_owned();
        let model_ok = renamed_model.validate_identity().is_ok();
        let mut renamed_constraint = base_snapshot();
        renamed_constraint.constraints[0].name = "renamed".to_owned();
        let constraint_refused = renamed_constraint.validate_identity()
            .is_err_and(|error| error.to_string().contains("fingerprint does not match"));
        model_ok && constraint_refused
    };
    // 5) 空白组名先于指纹。 / A blank group name precedes the fingerprint.
    let a_blank_group_name_is_refused_before_the_fingerprint = {
        let mut snapshot = base_snapshot();
        if let Some((_, name)) = snapshot.constraint_groups.iter_mut().next() {
            *name = "  ".to_owned();
        }
        // 改组名同时也会改指纹；报出来的必须是组名那条。
        // Changing the group name also changes the fingerprint; the group message must win.
        snapshot.validate_identity()
            .is_err_and(|error| error.to_string().contains("group name must not be blank"))
    };
    // 6) hint 可以只覆盖一部分。 / A hint may cover less than every variable.
    let a_hint_may_cover_less_than_every_variable = {
        let snapshot = base_snapshot();
        let empty = BTreeMap::new();
        let partial = BTreeMap::from([(StableVariableId("x".to_owned()), 2)]);
        let outside = BTreeMap::from([(StableVariableId("x".to_owned()), 9)]);
        let unknown = BTreeMap::from([(StableVariableId("ghost".to_owned()), 0)]);
        // 空映射恒过、单变量过、超域拒、未注册拒——但**都不查约束**。
        // An empty map passes, one variable passes, out-of-domain and unregistered are refused, and **constraints are
        // never evaluated**.
        snapshot.validate_hint(&empty).is_ok()
            && snapshot.validate_hint(&partial).is_ok()
            && snapshot.validate_hint(&outside).is_err()
            && snapshot.validate_hint(&unknown).is_err()
    };
    // 7) `objective_variables` 只看目标。 / `objective_variables` only looks at the objective.
    let objective_variables_ignore_constraints = {
        let snapshot = base_snapshot();
        // 约束引用了 x 和 y，目标只引用 x。 / The constraints reference x and y; the objective only x.
        let objective = snapshot.objective_variables();
        let mut without = snapshot.clone();
        without.objective = None;
        objective == BTreeMap::from([(StableVariableId("x".to_owned()), ())]).into_keys().collect()
            && without.objective_variables().is_empty()
    };
    // 8) artifact codec 三道关。 / The artifact codec checks three things.
    let the_artifact_codec_checks_three_things = {
        let snapshot = base_snapshot();
        let artifact = snapshot.to_artifact().expect("artifact");
        let bytes = artifact.to_canonical_json().expect("bytes");
        let round_trips = ConstraintProgrammingSnapshot::from_canonical_json(&bytes).is_ok();
        let mut wrong_schema = artifact.clone();
        wrong_schema.schema_version = "9.9".to_owned();
        let schema_refused = wrong_schema.validate()
            .is_err_and(|error| error.to_string().contains("unsupported CP snapshot codec schema"));
        let mut wrong_kind = artifact.clone();
        wrong_kind.kind = "something-else".to_owned();
        let kind_refused = wrong_kind.validate()
            .is_err_and(|error| error.to_string().contains("unexpected CP snapshot artifact kind"));
        let mut tampered = artifact;
        tampered.artifact_digest.value = "0".repeat(64);
        let digest_refused = tampered.validate()
            .is_err_and(|error| error.to_string().contains("artifact digest does not match"));
        round_trips && schema_refused && kind_refused && digest_refused
    };
    Findings {
        a_frozen_snapshot_always_validates,
        a_blank_identity_is_the_first_gate,
        the_collection_order_gate_shadows_the_sort_gate,
        the_model_name_is_outside_the_fingerprint_but_constraint_names_are_inside,
        a_blank_group_name_is_refused_before_the_fingerprint,
        a_hint_may_cover_less_than_every_variable,
        objective_variables_ignore_constraints,
        the_artifact_codec_checks_three_things,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> SnapshotGateRequest {
        SnapshotGateRequest { mutations: Vec::new(), hint: Vec::new(), evaluation: Vec::new() }
    }

    fn with_mutation(name: &str) -> SnapshotGateRequest {
        SnapshotGateRequest { mutations: vec![name.to_owned()], ..request() }
    }

    #[test]
    fn a_frozen_snapshot_validates_and_reports_its_shape() {
        let report = verify_snapshot_gates(&request()).unwrap();
        assert!(report.identity_accepted, "{:?}", report.identity_refusal);
        assert!(report.identity_is_idempotent);
        assert_eq!(report.model_name, "snapshot-gate");
        assert_eq!(report.variable_ids, vec!["x".to_owned(), "y".to_owned()]);
        assert_eq!(report.constraint_ids, vec!["capacity".to_owned(), "floor".to_owned()]);
        assert_eq!(report.constraint_names, vec!["capacity".to_owned(), "floor".to_owned()]);
        assert_eq!(report.groups, vec!["1:capacity".to_owned()]);
        // 目标只引用 x。 / The objective only references x.
        assert_eq!(report.objective_variables, vec!["x".to_owned()]);
        assert!(report.artifact_encoded);
        assert!(report.artifact_round_trips);
        assert_eq!(report.artifact_digest.as_deref().map(str::len), Some(64));
        assert!(report.a_frozen_snapshot_always_validates);
        assert!(report.a_blank_identity_is_the_first_gate);
        assert!(report.the_collection_order_gate_shadows_the_sort_gate);
        assert!(report.the_model_name_is_outside_the_fingerprint_but_constraint_names_are_inside);
        assert!(report.a_blank_group_name_is_refused_before_the_fingerprint);
        assert!(report.a_hint_may_cover_less_than_every_variable);
        assert!(report.objective_variables_ignore_constraints);
        assert!(report.the_artifact_codec_checks_three_things);
        // 没有 hint / evaluation 时两者都为空。 / Both are absent without a hint or an evaluation.
        assert_eq!(report.hint_accepted, None);
        assert_eq!(report.objective_value, None);
    }

    #[test]
    fn a_blank_identity_is_refused_with_the_namespace_first() {
        let both = verify_snapshot_gates(&with_mutation("blankNamespace")).unwrap();
        assert!(!both.identity_accepted);
        assert!(both.identity_refusal.as_deref().unwrap().contains("namespace must not be blank"));
        assert!(!both.artifact_encoded);
        let schema = verify_snapshot_gates(&with_mutation("blankSchemaVersion")).unwrap();
        assert!(schema.identity_refusal.as_deref().unwrap()
            .contains("schema version must not be blank"));
    }

    #[test]
    fn a_shuffled_collection_reports_the_whole_structure_gate() {
        let report = verify_snapshot_gates(&with_mutation("reverseVariables")).unwrap();
        assert!(!report.identity_accepted);
        let message = report.identity_refusal.unwrap();
        assert!(message.contains("non-canonical collection order"), "{message}");
        assert!(!message.contains("not in canonical stable-ID order"), "{message}");
        // 顺序被如实反映。 / The order is reported verbatim.
        assert_eq!(report.variable_ids, vec!["y".to_owned(), "x".to_owned()]);
    }

    #[test]
    fn renaming_the_model_is_legal_but_renaming_a_constraint_is_not() {
        let model = verify_snapshot_gates(&with_mutation("renameModel")).unwrap();
        assert!(model.identity_accepted);
        assert_eq!(model.model_name, "renamed");
        let constraint = verify_snapshot_gates(&with_mutation("renameConstraint")).unwrap();
        assert!(!constraint.identity_accepted);
        assert!(constraint.identity_refusal.as_deref().unwrap()
            .contains("fingerprint does not match canonical content"));
    }

    #[test]
    fn a_blank_group_name_wins_over_the_fingerprint() {
        let report = verify_snapshot_gates(&with_mutation("blankGroupName")).unwrap();
        assert!(!report.identity_accepted);
        assert!(report.identity_refusal.as_deref().unwrap()
            .contains("group name must not be blank"));
        assert_eq!(report.groups, vec!["1:  ".to_owned()]);
    }

    #[test]
    fn a_hint_is_a_partial_assignment_gate() {
        let empty = verify_snapshot_gates(&SnapshotGateRequest {
            hint: Vec::new(), ..request() }).unwrap();
        // 空 hint 列表 → 不调用；给一条合法值 → 通过。
        // An empty list means "skip"; one legal value passes.
        assert_eq!(empty.hint_accepted, None);
        let partial = verify_snapshot_gates(&SnapshotGateRequest {
            hint: vec![SnapshotValueSpec { id: "x".to_owned(), value: 2 }], ..request() }).unwrap();
        assert_eq!(partial.hint_accepted, Some(true));
        assert_eq!(partial.hint_refusal, None);
        // 超域与未注册都拒，且文案不同。 / Out-of-domain and unregistered are refused with different messages.
        let outside = verify_snapshot_gates(&SnapshotGateRequest {
            hint: vec![SnapshotValueSpec { id: "x".to_owned(), value: 9 }], ..request() }).unwrap();
        assert_eq!(outside.hint_accepted, Some(false));
        assert!(outside.hint_refusal.as_deref().unwrap().contains("outside the variable domain"));
        let unknown = verify_snapshot_gates(&SnapshotGateRequest {
            hint: vec![SnapshotValueSpec { id: "ghost".to_owned(), value: 0 }],
            ..request() }).unwrap();
        assert!(unknown.hint_refusal.as_deref().unwrap().contains("unknown variable"));
    }

    #[test]
    fn the_objective_value_reads_the_target_expression() {
        let report = verify_snapshot_gates(&SnapshotGateRequest {
            evaluation: vec![SnapshotValueSpec { id: "x".to_owned(), value: 3 },
                SnapshotValueSpec { id: "y".to_owned(), value: 1 }], ..request() }).unwrap();
        // 目标是 maximize x，所以读数就是 x。 / The objective maximizes x, so the reading is x.
        assert_eq!(report.objective_value, Some(3));
        assert_eq!(report.objective_value_refusal, None);
        // **按实测纠正**：`objective_value` 只求目标表达式，不做覆盖校验——少给 `y` 照样读得出 3。
        // **Corrected by measurement**: `objective_value` only evaluates the objective expression and never checks
        // coverage, so omitting `y` still reads 3.
        let partial = verify_snapshot_gates(&SnapshotGateRequest {
            evaluation: vec![SnapshotValueSpec { id: "x".to_owned(), value: 3 }],
            ..request() }).unwrap();
        assert_eq!(partial.objective_value, Some(3));
        assert_eq!(partial.objective_value_refusal, None);
        // 少给**目标引用**的变量才拒。 / Only omitting a variable the objective references is refused.
        let missing = verify_snapshot_gates(&SnapshotGateRequest {
            evaluation: vec![SnapshotValueSpec { id: "y".to_owned(), value: 1 }],
            ..request() }).unwrap();
        assert_eq!(missing.objective_value, None);
        assert!(missing.objective_value_refusal.as_deref().unwrap().contains("missing assignment"));
    }

    #[test]
    fn a_duplicate_stable_variable_id_is_refused() {
        let report = verify_snapshot_gates(&with_mutation("duplicateVariableStableId")).unwrap();
        assert!(!report.identity_accepted);
        // **按实测纠正**：重复稳定 ID 也先撞上整结构闸门——`canonicalized()` 会按索引重绑本地 ID，
        // 于是"重复"在到达去重检查之前就已经表现为结构不一致。
        // **Corrected by measurement**: a duplicate stable ID also hits the whole-structure gate first, because
        // `canonicalized()` rebinds local IDs by index, so the duplicate shows up as a structural mismatch before the
        // duplicate check runs.
        assert!(report.identity_refusal.as_deref().unwrap()
            .contains("non-canonical collection order"));
        let dropped = verify_snapshot_gates(&with_mutation("dropVariable")).unwrap();
        assert!(!dropped.identity_accepted);
        let blank = verify_snapshot_gates(&with_mutation("blankStableId")).unwrap();
        assert!(!blank.identity_accepted);
    }

    #[test]
    fn bad_input_is_refused() {
        assert!(verify_snapshot_gates(&with_mutation("nope")).unwrap_err().message
            .contains("unknown mutation"));
        assert!(verify_snapshot_gates(&SnapshotGateRequest {
            hint: vec![SnapshotValueSpec { id: "  ".to_owned(), value: 0 }], ..request() })
            .unwrap_err().message.contains("assignment IDs must not be blank"));
        assert!(verify_snapshot_gates(&SnapshotGateRequest {
            hint: vec![SnapshotValueSpec { id: "x".to_owned(), value: 0 },
                SnapshotValueSpec { id: "x".to_owned(), value: 1 }], ..request() })
            .unwrap_err().message.contains("repeats a stable ID"));
        assert!(verify_snapshot_gates(&SnapshotGateRequest {
            mutations: (0..17).map(|index| format!("m{index}")).collect(), ..request() })
            .unwrap_err().message.contains("at most 16 entries"));
    }

    /// 守门：DTO 必须是 camelCase 且拒绝未知字段。 / A guard: camelCase wire, unknown fields refused.
    #[test]
    fn the_report_wire_is_camel_case_and_rejects_unknown_fields() {
        let report = verify_snapshot_gates(&request()).unwrap();
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"identityAccepted\""), "{json}");
        assert!(json.contains("\"objectiveVariables\""), "{json}");
        assert!(json.contains("\"theArtifactCodecChecksThreeThings\""), "{json}");
        assert!(json.contains("\"artifactRoundTrips\""), "{json}");
        assert!(!json.contains("identity_accepted"), "{json}");
        let mut copy: serde_json::Value = serde_json::from_str(&json).unwrap();
        copy["unexpected"] = serde_json::json!(1);
        assert!(serde_json::from_str::<SnapshotGateReport>(&copy.to_string()).is_err());
    }
}
