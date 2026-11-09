//! 续挖第 73 单：`validate_assignment` 的**求值顺序**——先覆盖、再域、再区间、最后按规范序逐约束
//! （`model/constraint_programming/snapshot.rs` 的 `validate_assignment` + `interval.rs` 的 `IntervalVariable::evaluate`）
//! Continuation, delivery 73: the **evaluation order** inside `validate_assignment` — coverage first, then domains, then
//! intervals, then constraints in canonical order (`validate_assignment` in `model/constraint_programming/snapshot.rs`
//! together with `IntervalVariable::evaluate` in `interval.rs`).
//!
//! 第 71 单量过 `validate_identity` 的读侧闸门；这一单量**同一份快照上的赋值**怎么被接受。八条实测：
//!
//! 1. **赋值必须恰好覆盖每个变量**（`the_assignment_must_cover_exactly_every_variable()`）：少给一个与多给一个
//!    报**同一句** "does not cover every registered variable"。
//! 2. **域检查先于一切区间求值**（`the_domain_check_runs_before_any_interval()`）：超域值先被拒，即使
//!    `end ≠ start + duration` 也会同时触发。
//! 3. **`end` 必须等于 `start + duration`**（`an_interval_end_must_equal_start_plus_duration()`）：这是
//!    **错误**而不是"不满足"——区间求值失败会让整份赋值非法。
//! 4. **缺席的可选区间被每条约束跳过**（`an_absent_optional_interval_is_skipped_by_every_constraint()`）：
//!    presence 为假时 `evaluate` 返回 `None`，`noOverlap` 与 `cumulative` 都 `continue`。
//! 5. **`cumulative` 在每个区间端点测负载**（`cumulative_measures_load_at_every_boundary()`）。
//! 6. **违规点名的是规范序里的第一条**（`a_violation_names_the_first_constraint_in_canonical_id_order()`）：
//!    `cCumulative` < `cNoOverlap`，所以两条都违规时报 `cCumulative`。
//! 7. **返回值是区间值映射**（`the_return_value_is_the_interval_value_map()`）：只含**在场**区间，且
//!    `duration` 字段就是固定时长。
//! 8. **presence 变量必须是稀疏布尔域**（`a_presence_variable_must_have_the_sparse_boolean_domain()`）：
//!    区间 `0..=1` 在 `freeze` 就被拒（"is not Boolean"）——与第 72 单的 `is_boolean` 口径一致。
//!
//! Delivery 71 measured the read-side gates of `validate_identity`; this one measures how an **assignment** on the same
//! snapshot is accepted. **The assignment must cover exactly every variable** — one too few and one too many share the
//! **same** message. **The domain check runs before any interval evaluation.** **`end` must equal `start + duration`**,
//! and a mismatch is an **error** rather than "unsatisfied". **An absent optional interval is skipped by every
//! constraint.** **`cumulative` measures load at every interval boundary.** **A violation names the first constraint in
//! canonical ID order.** **The return value is the interval-value map** with entries only for present intervals. And **a
//! presence variable must have the sparse Boolean domain**, consistent with delivery 72's `is_boolean`.

use std::collections::BTreeMap;

use ospf_rust_core::model::constraint_programming::{
    BooleanLiteral, ConstraintDefinition, ConstraintProgrammingConstraint,
    ConstraintProgrammingModel, ConstraintProgrammingSnapshot, CumulativeTask, IntegerDomain,
    IntegerExpression, IntegerVariable, IntervalDuration, IntervalVariable, IntervalVariableId,
};
use ospf_rust_core::solver::report::StableVariableId;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 一条赋值条目。 / One assignment entry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AssignmentValueSpec {
    /// 稳定变量 ID。 / The stable variable ID.
    pub id: String,
    /// 整数值。 / The integer value.
    pub value: i64,
}

/// 赋值求值探针请求。 / The assignment-evaluation probe request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AssignmentEvalRequest {
    /// `s1` 的取值。 / The value of `s1`.
    #[serde(default)]
    pub s1: Option<i64>,
    /// `s2` 的取值。 / The value of `s2`.
    #[serde(default)]
    pub s2: Option<i64>,
    /// 固定时长（默认 2）。 / The fixed duration (default 2).
    #[serde(default)]
    pub duration: Option<i64>,
    /// presence 变量的取值（`0`/`1`）。 / The presence variable's value (`0`/`1`).
    #[serde(default)]
    pub presence: Option<i64>,
    /// `cumulative` 的容量（默认 5）。 / The cumulative capacity (default 5).
    #[serde(default)]
    pub capacity: Option<i64>,
    /// 额外给一个未注册的变量（量"多给"）。 / Add an unregistered variable (to measure "too many").
    #[serde(default)]
    pub extra: bool,
    /// 少给一个变量（量"少给"）。 / Omit one variable (to measure "too few").
    #[serde(default)]
    pub drop: bool,
    /// 让 `e1 ≠ s1 + duration`（量区间闸门）。 / Make `e1 ≠ s1 + duration`.
    #[serde(default)]
    pub break_end: bool,
    /// presence 变量的域：`values`（稀疏，默认）或 `range`（区间 `0..=1`）。
    /// The presence variable's domain: `values` (sparse, default) or `range` (`0..=1`).
    #[serde(default)]
    pub presence_domain: Option<String>,
    /// 区间的 presence 是否为 `None`（mandatory）。 / Whether the interval's presence is `None` (mandatory).
    #[serde(default)]
    pub mandatory: bool,
}

/// 一个区间值的读面。 / One interval value's readable face.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct IntervalValueView {
    /// 区间 ID。 / The interval ID.
    pub id: String,
    /// 起点。 / Start.
    pub start: i64,
    /// 终点。 / End.
    pub end: i64,
    /// 时长。 / Duration.
    pub duration: i64,
}

/// 赋值求值探针结论。 / The assignment-evaluation probe verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AssignmentEvalReport {
    /// 是否被接受。 / Whether the assignment was accepted.
    pub accepted: bool,
    /// 被拒文案。 / The refusal message.
    #[serde(default)]
    pub refusal: Option<String>,
    /// 被拒发生在哪一层（`freeze` / `coverage` / `domain` / `interval` / `constraint`）。
    /// Which layer refused (`freeze` / `coverage` / `domain` / `interval` / `constraint`).
    #[serde(default)]
    pub refusal_layer: Option<String>,
    /// 返回的区间值（只含在场区间）。 / The returned interval values (present intervals only).
    pub intervals: Vec<IntervalValueView>,
    /// 八条规则属性的读数。 / The eight rule readings.
    pub the_assignment_must_cover_exactly_every_variable: bool,
    pub the_domain_check_runs_before_any_interval: bool,
    pub an_interval_end_must_equal_start_plus_duration: bool,
    pub an_absent_optional_interval_is_skipped_by_every_constraint: bool,
    pub cumulative_measures_load_at_every_boundary: bool,
    pub a_violation_names_the_first_constraint_in_canonical_id_order: bool,
    pub the_return_value_is_the_interval_value_map: bool,
    pub a_presence_variable_must_have_the_sparse_boolean_domain: bool,
}

/// 探针模型：两段区间 + `noOverlap` + `cumulative`。 / The probe model: two intervals plus two constraints.
struct ProbeModel {
    snapshot: ConstraintProgrammingSnapshot,
    presence_variable: IntegerVariable,
}

/// 造探针模型。 / Build the probe model.
fn probe_model(duration: i64, capacity: i64, presence_domain: &str,
    mandatory: bool, with_no_overlap: bool) -> Result<ProbeModel> {
    let s1 = IntegerVariable::new("s1");
    let e1 = IntegerVariable::new("e1");
    let s2 = IntegerVariable::new("s2");
    let e2 = IntegerVariable::new("e2");
    let p = IntegerVariable::new("p");
    let mut model = ConstraintProgrammingModel::new("assignment-eval");
    for variable in [&s1, &e1, &s2, &e2] {
        model.register_variable(variable.clone(), IntegerDomain::range(0, 20).expect("domain"))
            .map_err(|error| BridgeError::new("InvalidInput", &format!(
                "assignment evaluation failed validation: {error}")))?;
    }
    // **按实测**：presence 必须是稀疏 `{0,1}`；区间 `0..=1` 会在 `freeze` 被判"非布尔"。
    // **Measured**: the presence must be the sparse `{0,1}`; the range `0..=1` is refused as "not Boolean" at freeze.
    let domain = match presence_domain {
        "range" => IntegerDomain::range(0, 1).expect("domain"),
        "values" => IntegerDomain::values([0, 1]).expect("domain"),
        other => return Err(BridgeError::new("InvalidInput", &format!(
            "assignment evaluation failed validation: unknown presence domain `{other}`"))),
    };
    model.register_variable(p.clone(), domain).map_err(|error| BridgeError::new("InvalidInput",
        &format!("assignment evaluation failed validation: {error}")))?;

    let interval_a = IntervalVariable::new(
        IntervalVariableId("ivA".to_owned()),
        IntegerExpression::variable(s1),
        IntervalDuration::Fixed(duration),
        IntegerExpression::variable(e1),
        None,
    ).map_err(|error| BridgeError::new("InvalidInput", &format!(
        "assignment evaluation failed validation: {error}")))?;
    let interval_b = IntervalVariable::new(
        IntervalVariableId("ivB".to_owned()),
        IntegerExpression::variable(s2),
        IntervalDuration::Fixed(duration),
        IntegerExpression::variable(e2),
        if mandatory { None } else { Some(BooleanLiteral::positive(p.clone())) },
    ).map_err(|error| BridgeError::new("InvalidInput", &format!(
        "assignment evaluation failed validation: {error}")))?;
    model.register_interval(interval_a).map_err(|error| BridgeError::new("InvalidInput",
        &format!("assignment evaluation failed validation: {error}")))?;
    model.register_interval(interval_b).map_err(|error| BridgeError::new("InvalidInput",
        &format!("assignment evaluation failed validation: {error}")))?;

    // **按实测**：`noOverlap` 会先挡掉重叠赋值，所以量 `cumulative` 的负载必须能把它关掉。
    // **Measured**: `noOverlap` pre-empts an overlapping assignment, so measuring cumulative load must be able to drop it.
    if with_no_overlap {
        model.add_constraint(ConstraintDefinition::new("cNoOverlap",
            ConstraintProgrammingConstraint::NoOverlap {
                intervals: vec![IntervalVariableId("ivA".to_owned()),
                    IntervalVariableId("ivB".to_owned())] }))
            .map_err(|error| BridgeError::new("InvalidInput", &format!(
                "assignment evaluation failed validation: {error}")))?;
    }
    model.add_constraint(ConstraintDefinition::new("cCumulative",
        ConstraintProgrammingConstraint::Cumulative {
            tasks: vec![
                CumulativeTask { interval: IntervalVariableId("ivA".to_owned()), demand: 3 },
                CumulativeTask { interval: IntervalVariableId("ivB".to_owned()), demand: 3 }],
            capacity }))
        .map_err(|error| BridgeError::new("InvalidInput", &format!(
            "assignment evaluation failed validation: {error}")))?;
    let snapshot = model.freeze().map_err(|error| BridgeError::new("InvalidInput", &format!(
        "assignment evaluation failed validation: {error}")))?;
    Ok(ProbeModel { snapshot, presence_variable: p })
}

/// 构造赋值。 / Build the assignment.
fn assignment_of(request: &AssignmentEvalRequest, duration: i64,
    presence_variable: &IntegerVariable) -> BTreeMap<StableVariableId, i64> {
    let s1 = request.s1.unwrap_or(0);
    // 默认 `s2 = 2`：两段**不重叠**，所以默认容量 5 够用。 
    // Default `s2 = 2`: the two intervals are **disjoint**, so the default capacity 5 suffices.
    let s2 = request.s2.unwrap_or(2);
    let end_of = |start: i64| if request.break_end { start + duration + 1 } else { start + duration };
    let mut values = BTreeMap::from([
        (StableVariableId("s1".to_owned()), s1),
        (StableVariableId("e1".to_owned()), end_of(s1)),
        (StableVariableId("s2".to_owned()), s2),
        (StableVariableId("e2".to_owned()), end_of(s2)),
        (presence_variable.stable_id.clone(), request.presence.unwrap_or(1)),
    ]);
    if request.drop {
        values.remove(&StableVariableId("e2".to_owned()));
    }
    if request.extra {
        values.insert(StableVariableId("ghost".to_owned()), 0);
    }
    values
}

/// 判断拒绝发生在哪一层。 / Classify which layer refused.
fn layer_of(message: &str) -> &'static str {
    if message.contains("does not cover every registered variable") {
        "coverage"
    } else if message.contains("outside its domain") {
        "domain"
    } else if message.contains("interval") {
        "interval"
    } else if message.contains("violates constraint") {
        "constraint"
    } else {
        "freeze"
    }
}

/// 走一遍赋值求值探针。非法输入是**错误**。 / Walk the assignment-evaluation probe.
pub fn verify_assignment_evaluation(request: &AssignmentEvalRequest) -> Result<AssignmentEvalReport> {
    let duration = request.duration.unwrap_or(2);
    if duration < 0 {
        return Err(BridgeError::new("InvalidInput",
            "assignment evaluation failed validation: the duration must not be negative"));
    }
    let capacity = request.capacity.unwrap_or(5);
    let presence_domain = request.presence_domain.clone().unwrap_or_else(|| "values".to_owned());
    let findings = canonical_findings();
    let model = match probe_model(duration, capacity, &presence_domain, request.mandatory, true) {
        Ok(model) => model,
        Err(error) => return Ok(refused_report("freeze", error.message, findings)),
    };
    let values = assignment_of(request, duration, &model.presence_variable);
    match model.snapshot.validate_assignment(&values) {
        Ok(intervals) => Ok(AssignmentEvalReport {
            accepted: true,
            refusal: None,
            refusal_layer: None,
            intervals: intervals.iter().map(|(id, value)| IntervalValueView {
                id: id.0.clone(), start: value.start, end: value.end,
                duration: value.duration }).collect(),
            the_assignment_must_cover_exactly_every_variable:
                findings.the_assignment_must_cover_exactly_every_variable,
            the_domain_check_runs_before_any_interval:
                findings.the_domain_check_runs_before_any_interval,
            an_interval_end_must_equal_start_plus_duration:
                findings.an_interval_end_must_equal_start_plus_duration,
            an_absent_optional_interval_is_skipped_by_every_constraint:
                findings.an_absent_optional_interval_is_skipped_by_every_constraint,
            cumulative_measures_load_at_every_boundary:
                findings.cumulative_measures_load_at_every_boundary,
            a_violation_names_the_first_constraint_in_canonical_id_order:
                findings.a_violation_names_the_first_constraint_in_canonical_id_order,
            the_return_value_is_the_interval_value_map:
                findings.the_return_value_is_the_interval_value_map,
            a_presence_variable_must_have_the_sparse_boolean_domain:
                findings.a_presence_variable_must_have_the_sparse_boolean_domain,
        }),
        Err(error) => {
            let message = error.to_string();
            Ok(refused_report(layer_of(&message), message, findings))
        }
    }
}

/// 一份被拒的结论。 / A refused verdict.
fn refused_report(layer: &str, message: String, findings: Findings) -> AssignmentEvalReport {
    AssignmentEvalReport {
        accepted: false,
        refusal: Some(message),
        refusal_layer: Some(layer.to_owned()),
        intervals: Vec::new(),
        the_assignment_must_cover_exactly_every_variable:
            findings.the_assignment_must_cover_exactly_every_variable,
        the_domain_check_runs_before_any_interval:
            findings.the_domain_check_runs_before_any_interval,
        an_interval_end_must_equal_start_plus_duration:
            findings.an_interval_end_must_equal_start_plus_duration,
        an_absent_optional_interval_is_skipped_by_every_constraint:
            findings.an_absent_optional_interval_is_skipped_by_every_constraint,
        cumulative_measures_load_at_every_boundary: findings.cumulative_measures_load_at_every_boundary,
        a_violation_names_the_first_constraint_in_canonical_id_order:
            findings.a_violation_names_the_first_constraint_in_canonical_id_order,
        the_return_value_is_the_interval_value_map: findings.the_return_value_is_the_interval_value_map,
        a_presence_variable_must_have_the_sparse_boolean_domain:
            findings.a_presence_variable_must_have_the_sparse_boolean_domain,
    }
}

/// 八条规则属性的内部固定探针。 / The eight rule properties measured by fixed internal probes.
struct Findings {
    the_assignment_must_cover_exactly_every_variable: bool,
    the_domain_check_runs_before_any_interval: bool,
    an_interval_end_must_equal_start_plus_duration: bool,
    an_absent_optional_interval_is_skipped_by_every_constraint: bool,
    cumulative_measures_load_at_every_boundary: bool,
    a_violation_names_the_first_constraint_in_canonical_id_order: bool,
    the_return_value_is_the_interval_value_map: bool,
    a_presence_variable_must_have_the_sparse_boolean_domain: bool,
}

/// 一份默认请求。 / A default request.
fn default_request() -> AssignmentEvalRequest {
    AssignmentEvalRequest { s1: None, s2: None, duration: None, presence: None, capacity: None,
        extra: false, drop: false, break_end: false, presence_domain: None, mandatory: false }
}

/// 用默认请求跑一遍并返回结果。 / Run the default request and return the outcome.
fn evaluate(request: &AssignmentEvalRequest) -> std::result::Result<Vec<IntervalValueView>, String> {
    evaluate_with(request, true)
}

/// 同上，但可关掉 `noOverlap`。 / As above, but `noOverlap` may be dropped.
fn evaluate_with(request: &AssignmentEvalRequest, with_no_overlap: bool)
    -> std::result::Result<Vec<IntervalValueView>, String> {
    let duration = request.duration.unwrap_or(2);
    let model = probe_model(duration, request.capacity.unwrap_or(5),
        request.presence_domain.as_deref().unwrap_or("values"), request.mandatory, with_no_overlap)
        .map_err(|error| error.message)?;
    let values = assignment_of(request, duration, &model.presence_variable);
    model.snapshot.validate_assignment(&values)
        .map(|intervals| intervals.into_iter().map(|(id, value)| IntervalValueView {
            id: id.0, start: value.start, end: value.end, duration: value.duration })
            .collect())
        .map_err(|error| error.to_string())
}

/// 用固定探针测出全部规则属性——不读调用方的请求值。
/// Measure every rule property with fixed probes; the caller's request values are not read.
fn canonical_findings() -> Findings {
    // 1) 覆盖必须恰好相等。 / Coverage must be exact.
    let the_assignment_must_cover_exactly_every_variable = {
        let missing = evaluate(&AssignmentEvalRequest { drop: true, ..default_request() })
            .err().unwrap_or_default();
        let extra = evaluate(&AssignmentEvalRequest { extra: true, ..default_request() })
            .err().unwrap_or_default();
        // 两种错法给**同一句**文案。 / Both mistakes share **one** message.
        missing.contains("does not cover every registered variable")
            && extra.contains("does not cover every registered variable")
            && missing == extra
    };
    // 2) 域检查先于区间求值。 / The domain check precedes interval evaluation.
    let the_domain_check_runs_before_any_interval = {
        // 超域**且** end 不匹配：报的必须是域那条。 / Out of domain **and** a broken end: the domain message must win.
        let both = evaluate(&AssignmentEvalRequest { s1: Some(99), break_end: true,
            ..default_request() }).err().unwrap_or_default();
        both.contains("outside its domain") && !both.contains("does not equal start plus duration")
    };
    // 3) `end` 必须等于 `start + duration`，且是错误。 / `end` must equal `start + duration`, as an error.
    let an_interval_end_must_equal_start_plus_duration = {
        let broken = evaluate(&AssignmentEvalRequest { break_end: true, ..default_request() })
            .err().unwrap_or_default();
        broken.contains("end does not equal start plus duration")
            && evaluate(&default_request()).is_ok()
    };
    // 4) 缺席的可选区间被跳过。 / An absent optional interval is skipped.
    let an_absent_optional_interval_is_skipped_by_every_constraint = {
        // presence=0 且两段**重叠**：若 ivB 被计入，`noOverlap` 会拒；实际通过。 
        // presence=0 with two **overlapping** intervals: if ivB counted, `noOverlap` would refuse; it passes.
        let absent = evaluate(&AssignmentEvalRequest { presence: Some(0), s1: Some(0), s2: Some(0),
            ..default_request() });
        // 对照：presence=1 时同一份赋值被拒。 / Control: with presence=1 the same assignment is refused.
        let present = evaluate(&AssignmentEvalRequest { presence: Some(1), s1: Some(0), s2: Some(0),
            ..default_request() }).err().unwrap_or_default();
        absent.is_ok_and(|intervals| intervals.len() == 1 && intervals[0].id == "ivA")
            && present.contains("violates constraint")
    };
    // 5) `cumulative` 在每个端点测负载。 / `cumulative` measures load at every boundary.
    let cumulative_measures_load_at_every_boundary = {
        // 关掉 `noOverlap` 才能看见负载本身：两段各 demand 3，容量 5 不够、6 够。
        // Dropping `noOverlap` exposes the load itself: two demands of 3, capacity 5 short and 6 enough.
        let tight = evaluate_with(&AssignmentEvalRequest { s1: Some(0), s2: Some(0),
            capacity: Some(5), ..default_request() }, false).err().unwrap_or_default();
        let loose = evaluate_with(&AssignmentEvalRequest { s1: Some(0), s2: Some(0),
            capacity: Some(6), ..default_request() }, false).is_ok();
        // 不重叠时容量 5 就够——负载只在重叠处累加。
        // Without overlap capacity 5 suffices: load only accumulates where intervals overlap.
        let disjoint = evaluate_with(&AssignmentEvalRequest { s1: Some(0), s2: Some(5),
            capacity: Some(5), ..default_request() }, false).is_ok();
        tight.contains("violates constraint") && loose && disjoint
    };
    // 6) 违规点名规范序第一条。 / A violation names the first constraint in canonical order.
    let a_violation_names_the_first_constraint_in_canonical_id_order = {
        // 两段完全重叠 → `noOverlap` 与 `cumulative` **都**违规；`cCumulative` < `cNoOverlap`。
        // Two fully overlapping intervals violate **both**; `cCumulative` < `cNoOverlap`.
        let message = evaluate(&AssignmentEvalRequest { s1: Some(0), s2: Some(0), capacity: Some(1),
            ..default_request() }).err().unwrap_or_default();
        message.contains("violates constraint cCumulative")
            && !message.contains("cNoOverlap")
    };
    // 7) 返回值就是区间值映射。 / The return value is the interval-value map.
    let the_return_value_is_the_interval_value_map = {
        let intervals = evaluate(&default_request()).unwrap_or_default();
        // 两个在场区间，`duration` 字段就是固定时长 2。 / Two present intervals whose `duration` is the fixed 2.
        intervals.len() == 2
            && intervals.iter().all(|value| value.duration == 2)
            && intervals.iter().all(|value| value.end == value.start + value.duration)
            && intervals.iter().map(|value| value.id.clone()).collect::<Vec<_>>()
                == vec!["ivA".to_owned(), "ivB".to_owned()]
    };
    // 8) presence 必须是稀疏布尔域。 / The presence must be the sparse Boolean domain.
    let a_presence_variable_must_have_the_sparse_boolean_domain = {
        let range = evaluate(&AssignmentEvalRequest { presence_domain: Some("range".to_owned()),
            ..default_request() }).err().unwrap_or_default();
        let values = evaluate(&AssignmentEvalRequest {
            presence_domain: Some("values".to_owned()), ..default_request() }).is_ok();
        range.contains("is not Boolean") && values
    };
    Findings {
        the_assignment_must_cover_exactly_every_variable,
        the_domain_check_runs_before_any_interval,
        an_interval_end_must_equal_start_plus_duration,
        an_absent_optional_interval_is_skipped_by_every_constraint,
        cumulative_measures_load_at_every_boundary,
        a_violation_names_the_first_constraint_in_canonical_id_order,
        the_return_value_is_the_interval_value_map,
        a_presence_variable_must_have_the_sparse_boolean_domain,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> AssignmentEvalRequest {
        default_request()
    }

    #[test]
    fn the_default_assignment_is_accepted_and_returns_both_intervals() {
        let report = verify_assignment_evaluation(&request()).unwrap();
        assert!(report.accepted, "{:?}", report.refusal);
        assert_eq!(report.refusal_layer, None);
        assert_eq!(report.intervals.len(), 2);
        assert_eq!(report.intervals[0].id, "ivA");
        assert_eq!(report.intervals[0].start, 0);
        assert_eq!(report.intervals[0].end, 2);
        assert_eq!(report.intervals[0].duration, 2);
        assert_eq!(report.intervals[1].id, "ivB");
        assert_eq!(report.intervals[1].start, 2);
        assert!(report.the_assignment_must_cover_exactly_every_variable);
        assert!(report.the_domain_check_runs_before_any_interval);
        assert!(report.an_interval_end_must_equal_start_plus_duration);
        assert!(report.an_absent_optional_interval_is_skipped_by_every_constraint);
        assert!(report.cumulative_measures_load_at_every_boundary);
        assert!(report.a_violation_names_the_first_constraint_in_canonical_id_order);
        assert!(report.the_return_value_is_the_interval_value_map);
        assert!(report.a_presence_variable_must_have_the_sparse_boolean_domain);
    }

    #[test]
    fn coverage_must_be_exact_and_both_mistakes_share_one_message() {
        let missing = verify_assignment_evaluation(&AssignmentEvalRequest { drop: true,
            ..request() }).unwrap();
        assert!(!missing.accepted);
        assert_eq!(missing.refusal_layer.as_deref(), Some("coverage"));
        let extra = verify_assignment_evaluation(&AssignmentEvalRequest { extra: true,
            ..request() }).unwrap();
        assert_eq!(extra.refusal_layer.as_deref(), Some("coverage"));
        assert_eq!(missing.refusal, extra.refusal);
        assert!(missing.refusal.unwrap().contains("does not cover every registered variable"));
    }

    #[test]
    fn the_domain_check_runs_before_the_interval_check() {
        let report = verify_assignment_evaluation(&AssignmentEvalRequest { s1: Some(99),
            break_end: true, ..request() }).unwrap();
        assert!(!report.accepted);
        assert_eq!(report.refusal_layer.as_deref(), Some("domain"));
        assert!(report.refusal.as_deref().unwrap().contains("outside its domain"));
    }

    #[test]
    fn a_broken_interval_end_is_an_error_not_a_violation() {
        let report = verify_assignment_evaluation(&AssignmentEvalRequest { break_end: true,
            ..request() }).unwrap();
        assert!(!report.accepted);
        assert_eq!(report.refusal_layer.as_deref(), Some("interval"));
        assert!(report.refusal.as_deref().unwrap()
            .contains("end does not equal start plus duration"));
        assert!(report.intervals.is_empty());
    }

    #[test]
    fn an_absent_optional_interval_is_skipped() {
        // presence=0 + 完全重叠：通过，且只返回 ivA。 / presence=0 with full overlap passes and returns only ivA.
        let absent = verify_assignment_evaluation(&AssignmentEvalRequest { presence: Some(0),
            s1: Some(0), s2: Some(0), ..request() }).unwrap();
        assert!(absent.accepted, "{:?}", absent.refusal);
        assert_eq!(absent.intervals.len(), 1);
        assert_eq!(absent.intervals[0].id, "ivA");
        // presence=1 时同一份赋值被拒。 / With presence=1 the same assignment is refused.
        let present = verify_assignment_evaluation(&AssignmentEvalRequest { presence: Some(1),
            s1: Some(0), s2: Some(0), ..request() }).unwrap();
        assert!(!present.accepted);
        assert_eq!(present.refusal_layer.as_deref(), Some("constraint"));
    }

    #[test]
    fn cumulative_measures_load_only_where_intervals_overlap() {
        // 请求路径带 `noOverlap`：容量 6 时负载本身够，**被点名的是 `cNoOverlap`**——这本身也是实测结论。
        // The request path carries `noOverlap`: at capacity 6 the load itself fits and **`cNoOverlap` is the one named** —
        // itself a measured result.
        let overlapping = verify_assignment_evaluation(&AssignmentEvalRequest { s1: Some(0),
            s2: Some(0), capacity: Some(6), ..request() }).unwrap();
        assert!(!overlapping.accepted);
        assert!(overlapping.refusal.as_deref().unwrap().contains("cNoOverlap"),
            "{:?}", overlapping.refusal);
        // 不重叠时容量 5 就够。 / Without overlap capacity 5 suffices.
        let disjoint = verify_assignment_evaluation(&AssignmentEvalRequest { s1: Some(0),
            s2: Some(5), capacity: Some(5), ..request() }).unwrap();
        assert!(disjoint.accepted, "{:?}", disjoint.refusal);
    }

    #[test]
    fn a_violation_names_the_first_constraint_in_canonical_order() {
        let report = verify_assignment_evaluation(&AssignmentEvalRequest { s1: Some(0),
            s2: Some(0), capacity: Some(1), ..request() }).unwrap();
        assert!(!report.accepted);
        let message = report.refusal.unwrap();
        assert!(message.contains("violates constraint cCumulative"), "{message}");
        assert!(!message.contains("cNoOverlap"), "{message}");
    }

    #[test]
    fn a_range_presence_domain_is_refused_at_freeze() {
        let report = verify_assignment_evaluation(&AssignmentEvalRequest {
            presence_domain: Some("range".to_owned()), ..request() }).unwrap();
        assert!(!report.accepted);
        assert_eq!(report.refusal_layer.as_deref(), Some("freeze"));
        assert!(report.refusal.as_deref().unwrap().contains("is not Boolean"));
    }

    #[test]
    fn a_mandatory_interval_is_always_present() {
        // mandatory 时 presence 变量仍在，但区间不看它。 / With a mandatory interval the presence variable exists but
        // the interval ignores it.
        let report = verify_assignment_evaluation(&AssignmentEvalRequest { mandatory: true,
            presence: Some(0), s1: Some(0), s2: Some(0), ..request() }).unwrap();
        assert!(!report.accepted);
        assert_eq!(report.refusal_layer.as_deref(), Some("constraint"));
    }

    #[test]
    fn a_negative_duration_is_refused_as_input() {
        assert!(verify_assignment_evaluation(&AssignmentEvalRequest { duration: Some(-1),
            ..request() }).unwrap_err().message.contains("must not be negative"));
    }

    /// 守门：DTO 必须是 camelCase 且拒绝未知字段。 / A guard: camelCase wire, unknown fields refused.
    #[test]
    fn the_report_wire_is_camel_case_and_rejects_unknown_fields() {
        let report = verify_assignment_evaluation(&request()).unwrap();
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"refusalLayer\""), "{json}");
        assert!(json.contains("\"theAssignmentMustCoverExactlyEveryVariable\""), "{json}");
        assert!(json.contains("\"cumulativeMeasuresLoadAtEveryBoundary\""), "{json}");
        assert!(!json.contains("refusal_layer"), "{json}");
        let mut copy: serde_json::Value = serde_json::from_str(&json).unwrap();
        copy["unexpected"] = serde_json::json!(1);
        assert!(serde_json::from_str::<AssignmentEvalReport>(&copy.to_string()).is_err());
    }
}
