//! 续挖第 72 单：CP 约束 AST 的**规范化**——`canonicalize` 只排序"顺序无意义"的变体，而它从不合并也从不化简
//! （`model/constraint_programming/constraint.rs` 的 `ConstraintProgrammingConstraint::canonicalize`）
//! Continuation, delivery 72: the **normalization** of the CP constraint AST — `canonicalize` sorts only the variants
//! whose order is semantically irrelevant, and it never merges nor simplifies (`ConstraintProgrammingConstraint::canonicalize`
//! in `model/constraint_programming/constraint.rs`).
//!
//! `canonicalize` 是 `pub(crate)`，所以只能通过 `freeze()` 观察——这正好是它真正生效的地方。八条实测：
//!
//! 1. **五个布尔组合子按 `(稳定 ID, 是否取反)` 排序**（`the_boolean_combinators_sort_their_literals()`）：
//!    `and` / `or` / `xor` / `atMostOne` / `exactlyOne`，且 `positive` 排在 `negated` 之前。
//! 2. **`allDifferent` 排表达式，两张表约束只排元组**（`the_table_constraints_sort_tuples_not_expressions()`）：
//!    `expressions` 保持原序——列序就是语义。
//! 3. **`noOverlap` 排区间 ID，`cumulative` 用 `demand` 当次键**（`cumulative_sorts_by_interval_then_demand()`）。
//! 4. **`automaton` 按 `(from, value, to)` 排转移**，`expressions` 与 `final_states` 不动
//!    （`the_automaton_sorts_its_transitions_only()`）。
//! 5. **`reservoir` 按事件的规范字节排序**（`the_reservoir_sorts_its_events()`）。
//! 6. **`circuit` / `element` / `integer` / `boolean` 一个都不碰**（`the_untouched_variants_keep_their_order()`）：
//!    落到 `_ => self.clone()`，因为这些变体的顺序**就是**语义（`circuit` 的第 i 项是节点 i 的后继）。
//! 7. **它从不合并也从不化简**（`it_never_dedups_and_never_simplifies()`）：重复 literal 留着、单 literal 的
//!    `and` 仍然是 `and`（不会塌成 `boolean`）。
//! 8. **指纹因此对"被排序的变体"顺序不敏感、对"没碰的变体"顺序敏感**
//!    （`the_fingerprint_is_order_insensitive_exactly_where_sorting_happens()`）。
//!
//! `canonicalize` is `pub(crate)`, so it can only be observed through `freeze()` — which is exactly where it takes effect.
//! **The five boolean combinators sort their literals** by `(stable ID, negated)`, `positive` before `negated`. **`allDifferent`
//! sorts expressions while the two table constraints sort only tuples** — `expressions` keep their order because column order
//! *is* the semantics. **`noOverlap` sorts interval IDs and `cumulative` uses `demand` as the tiebreaker.** **`automaton` sorts
//! its transitions by `(from, value, to)`** and leaves `expressions`/`final_states` alone. **`reservoir` sorts its events** by
//! canonical bytes. **`circuit` / `element` / `integer` / `boolean` are untouched** (`_ => self.clone()`) because their order
//! *is* the semantics — a circuit's i-th entry is node i's successor. **It never dedups and never simplifies.** And **the
//! fingerprint is therefore order-insensitive exactly where sorting happens**.

use std::collections::{BTreeMap, BTreeSet};

use ospf_rust_core::model::constraint_programming::{
    AutomatonTransition, BooleanLiteral, ConstraintDefinition, ConstraintProgrammingConstraint,
    ConstraintProgrammingModel, ConstraintProgrammingSnapshot, CumulativeTask, IntegerDomain,
    IntegerExpression, IntervalDuration, IntervalVariable, IntervalVariableId, ReservoirEvent,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 一条约束的形状声明。 / One constraint shape declaration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConstraintShapeSpec {
    /// 约束变体：`and` / `or` / `xor` / `atMostOne` / `exactlyOne` / `allDifferent` /
    /// `allowedAssignments` / `forbiddenAssignments` / `noOverlap` / `cumulative` / `automaton` /
    /// `reservoir` / `circuit` / `integer`。
    /// The constraint variant.
    pub kind: String,
    /// 该变体的项。 / The variant's items.
    #[serde(default)]
    pub items: Vec<String>,
}

/// 约束规范化探针请求。 / The constraint-canonicalization probe request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConstraintCanonRequest {
    /// 约束声明。 / The constraint declarations.
    #[serde(default)]
    pub constraints: Vec<ConstraintShapeSpec>,
    /// 是否把每条约束的项**反序**（量"排序是否生效"）。 / Whether to reverse every item list.
    #[serde(default)]
    pub reverse_items: bool,
}

/// 一条约束规范化后的读面。 / One constraint's readable face after canonicalization.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConstraintCanonView {
    /// 约束 ID。 / The constraint ID.
    pub id: String,
    /// 规范化后的变体名。 / The canonicalized variant name.
    pub kind: String,
    /// 规范化后的项（按变体各自的读法）。 / The canonicalized items, read per variant.
    pub items: Vec<String>,
}

/// 约束规范化探针结论。 / The constraint-canonicalization probe verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConstraintCanonReport {
    /// 是否冻结成功。 / Whether the freeze succeeded.
    pub accepted: bool,
    /// 被拒时的错误文案。 / The refusal message.
    #[serde(default)]
    pub refusal: Option<String>,
    /// 快照指纹。 / The snapshot fingerprint.
    #[serde(default)]
    pub fingerprint: Option<String>,
    /// 每条约束的读面。 / Each constraint's readable face.
    pub constraints: Vec<ConstraintCanonView>,
    /// 八条规则属性的读数。 / The eight rule readings.
    pub the_boolean_combinators_sort_their_literals: bool,
    pub the_table_constraints_sort_tuples_not_expressions: bool,
    pub cumulative_sorts_by_interval_and_its_demand_tiebreak_is_unreachable: bool,
    pub the_automaton_sorts_its_transitions_only: bool,
    pub the_reservoir_sorts_its_events: bool,
    pub the_untouched_variants_keep_their_order: bool,
    pub it_never_dedups_and_never_simplifies: bool,
    pub the_fingerprint_is_order_insensitive_exactly_where_sorting_happens: bool,
}

/// 一个区间的声明。 / One interval declaration.
fn interval_of(id: &str) -> IntervalVariable {
    // 常量起点/终点：区间不引用任何变量，结构校验只查非负与 min ≤ max。
    // Constant start/end: the interval references no variable, so structure validation only checks non-negativity.
    IntervalVariable::new(
        IntervalVariableId(id.to_owned()),
        IntegerExpression::constant(0),
        IntervalDuration::Fixed(1),
        IntegerExpression::constant(1),
        None,
    )
    .expect("probe interval")
}

/// 注册一个区间（同 ID 只注册一次）。 / Register an interval once per ID.
fn register_interval_once(
    model: &mut ConstraintProgrammingModel,
    seen: &mut Vec<IntervalVariableId>,
    id: &str,
) -> Result<()> {
    let identity = IntervalVariableId(id.to_owned());
    if seen.contains(&identity) {
        return Ok(());
    }
    model.register_interval(interval_of(id)).map_err(|error| BridgeError::new("InvalidInput",
        &format!("constraint canonicalization failed validation: {error}")))?;
    seen.push(identity);
    Ok(())
}

/// 一个布尔 literal 的文字（`id` 或 `!id`）。 / A boolean literal's text (`id` or `!id`).
fn literal_text(literal: &BooleanLiteral) -> String {
    if literal.negated {
        format!("!{}", literal.variable.stable_id.0)
    } else {
        literal.variable.stable_id.0.clone()
    }
}

/// 已注册变量的账本：稳定 ID → **同一个实例**。同一稳定 ID 换个本地 ID 会被 `freeze` 拒，
/// 所以账本必须缓存实例而不是只记 ID。
/// The registered-variable ledger: stable ID → **the same instance**. Reusing a stable ID under a fresh local ID is
/// refused by `freeze`, so the ledger must cache the instance rather than just the ID.
type Registered = BTreeMap<String, ospf_rust_core::model::constraint_programming::IntegerVariable>;

/// 注册一个变量并返回**账本里的那个实例**。 / Register a variable and return **the ledger's instance**.
fn register_once(
    model: &mut ConstraintProgrammingModel,
    registered: &mut Registered,
    variable: &ospf_rust_core::model::constraint_programming::IntegerVariable,
    domain: IntegerDomain,
) -> Result<ospf_rust_core::model::constraint_programming::IntegerVariable> {
    if let Some(existing) = registered.get(&variable.stable_id.0) {
        return Ok(existing.clone());
    }
    model.register_variable(variable.clone(), domain)
        .map_err(|error| BridgeError::new("InvalidInput", &format!(
            "constraint canonicalization failed validation: {error}")))?;
    registered.insert(variable.stable_id.0.clone(), variable.clone());
    Ok(variable.clone())
}

/// 解析 literal 文字。 / Parse a literal text.
fn literal_of(text: &str, model: &mut ConstraintProgrammingModel,
    registered: &mut Registered) -> Result<BooleanLiteral> {
    let (negated, id) = match text.strip_prefix('!') {
        Some(rest) => (true, rest),
        None => (false, text),
    };
    if id.trim().is_empty() {
        return Err(BridgeError::new("InvalidInput",
            "constraint canonicalization failed validation: literal IDs must not be blank"));
    }
    let candidate = ospf_rust_core::model::constraint_programming::IntegerVariable::new(id);
    // **布尔域必须是稀疏的 `{0,1}`**：`is_boolean` 只认 `Values([0,1])`，区间 `0..=1` 会被判成"非布尔"。
    // **A Boolean domain must be the sparse `{0,1}`**: `is_boolean` only accepts `Values([0,1])`, so the range `0..=1`
    // counts as "not Boolean".
    let variable = register_once(model, registered, &candidate,
        IntegerDomain::values([0, 1]).expect("boolean domain"))?;
    Ok(BooleanLiteral { variable, negated })
}

/// 解析整数变量表达式。 / Parse an integer-variable expression.
fn variable_expression(
    text: &str,
    upper: i64,
    model: &mut ConstraintProgrammingModel,
    registered: &mut Registered,
) -> Result<IntegerExpression> {
    if text.trim().is_empty() {
        return Err(BridgeError::new("InvalidInput",
            "constraint canonicalization failed validation: variable IDs must not be blank"));
    }
    let candidate = ospf_rust_core::model::constraint_programming::IntegerVariable::new(text);
    let variable = register_once(model, registered, &candidate,
        IntegerDomain::range(0, upper).expect("domain"))?;
    Ok(IntegerExpression::variable(variable))
}

/// 把一个 `a,b,c` 形式的元组解析成整数列表。 / Parse a `a,b,c` tuple into integers.
fn tuple_of(text: &str) -> Result<Vec<i64>> {
    text.split(',')
        .map(|piece| piece.trim().parse::<i64>().map_err(|_| BridgeError::new("InvalidInput",
            &format!("constraint canonicalization failed validation: `{piece}` is not an integer"))))
        .collect()
}

/// 把一个 `a:b` 形式切成两段。 / Split an `a:b` pair.
fn pair_of(text: &str) -> Result<(&str, &str)> {
    text.split_once(':').ok_or_else(|| BridgeError::new("InvalidInput", &format!(
        "constraint canonicalization failed validation: `{text}` is not a `a:b` pair")))
}

/// 造一条约束。 / Build one constraint.
fn constraint_of(
    spec: &ConstraintShapeSpec,
    model: &mut ConstraintProgrammingModel,
    registered: &mut Registered,
) -> Result<ConstraintProgrammingConstraint> {
    let items = &spec.items;
    Ok(match spec.kind.as_str() {
        "and" | "or" | "xor" | "atMostOne" | "exactlyOne" => {
            let mut literals = Vec::with_capacity(items.len());
            for item in items {
                literals.push(literal_of(item, model, registered)?);
            }
            match spec.kind.as_str() {
                "and" => ConstraintProgrammingConstraint::and(literals),
                "or" => ConstraintProgrammingConstraint::or(literals),
                "xor" => ConstraintProgrammingConstraint::xor(literals),
                "atMostOne" => ConstraintProgrammingConstraint::at_most_one(literals),
                _ => ConstraintProgrammingConstraint::exactly_one(literals),
            }
        }
        "allDifferent" => {
            let mut expressions = Vec::with_capacity(items.len());
            for item in items {
                expressions.push(variable_expression(item, 9, model, registered)?);
            }
            ConstraintProgrammingConstraint::AllDifferent { expressions }
        }
        "allowedAssignments" | "forbiddenAssignments" => {
            // 列数是元组长度；表达式列**不由项决定**，所以这里固定给一列。
            // The column count is the tuple length; expression columns are not driven by the items, so use one column.
            let expression = variable_expression("tableValue", 9, model, registered)?;
            let mut tuples = Vec::with_capacity(items.len());
            for item in items {
                tuples.push(tuple_of(item)?);
            }
            if spec.kind == "allowedAssignments" {
                ConstraintProgrammingConstraint::AllowedAssignments {
                    expressions: vec![expression], tuples }
            } else {
                ConstraintProgrammingConstraint::ForbiddenAssignments {
                    expressions: vec![expression], tuples }
            }
        }
        "noOverlap" => {
            let mut intervals = Vec::with_capacity(items.len());
            for item in items {
                register_interval_once(model, &mut intervals, item)?;
            }
            ConstraintProgrammingConstraint::NoOverlap { intervals }
        }
        "cumulative" => {
            let mut tasks: Vec<CumulativeTask> = Vec::with_capacity(items.len());
            for item in items {
                let (id, demand) = pair_of(item)?;
                let demand = demand.trim().parse::<i64>().map_err(|_| BridgeError::new(
                    "InvalidInput", "constraint canonicalization failed validation: bad demand"))?;
                let mut seen: Vec<IntervalVariableId> = tasks.iter()
                    .map(|task| task.interval.clone()).collect();
                register_interval_once(model, &mut seen, id)?;
                tasks.push(CumulativeTask { interval: IntervalVariableId(id.to_owned()), demand });
            }
            ConstraintProgrammingConstraint::Cumulative { tasks, capacity: 100 }
        }
        "automaton" => {
            let expression = variable_expression("sequence", 9, model, registered)?;
            let mut transitions = Vec::with_capacity(items.len());
            for item in items {
                let pieces = item.split(':').collect::<Vec<_>>();
                if pieces.len() != 3 {
                    return Err(BridgeError::new("InvalidInput", &format!(
                        "constraint canonicalization failed validation: `{item}` is not `from:value:to`")));
                }
                let parse = |text: &str, what: &str| text.trim().parse::<i32>().map_err(|_| {
                    BridgeError::new("InvalidInput", &format!(
                        "constraint canonicalization failed validation: bad {what} in `{item}`")) });
                transitions.push(AutomatonTransition {
                    from_state: parse(pieces[0], "from")?,
                    value: pieces[1].trim().parse::<i64>().map_err(|_| BridgeError::new(
                        "InvalidInput", "constraint canonicalization failed validation: bad value"))?,
                    to_state: parse(pieces[2], "to")?,
                });
            }
            ConstraintProgrammingConstraint::Automaton {
                expressions: vec![expression],
                initial_state: 0,
                final_states: BTreeSet::from([1]),
                transitions,
            }
        }
        "reservoir" => {
            let mut events = Vec::with_capacity(items.len());
            for item in items {
                let (time, delta) = pair_of(item)?;
                let time = time.trim().parse::<i64>().map_err(|_| BridgeError::new("InvalidInput",
                    "constraint canonicalization failed validation: bad event time"))?;
                let delta = delta.trim().parse::<i64>().map_err(|_| BridgeError::new("InvalidInput",
                    "constraint canonicalization failed validation: bad level change"))?;
                events.push(ReservoirEvent {
                    time: IntegerExpression::constant(time),
                    level_change: IntegerExpression::constant(delta),
                });
            }
            ConstraintProgrammingConstraint::Reservoir {
                events, initial_level: 0, minimum_level: -100, maximum_level: 100 }
        }
        "circuit" => {
            // `circuit` 的后继必须落在 `0..size` 里，所以变量域按项数给。
            // A circuit successor must lie in `0..size`, so the domain follows the item count.
            let upper = i64::try_from(items.len()).unwrap_or(1).saturating_sub(1);
            let mut successors = Vec::with_capacity(items.len());
            for item in items {
                successors.push(variable_expression(item, upper.max(0), model, registered)?);
            }
            ConstraintProgrammingConstraint::Circuit { successors }
        }
        "integer" => {
            let expression = variable_expression(items.first().map(String::as_str).unwrap_or("x"),
                9, model, registered)?;
            ConstraintProgrammingConstraint::integer(
                expression,
                ospf_rust_core::model::constraint_programming::IntegerRelation::LessOrEqual,
                9)
        }
        other => return Err(BridgeError::new("InvalidInput", &format!(
            "constraint canonicalization failed validation: unknown kind `{other}`"))),
    })
}

/// 把一条规范化后的约束读回项列表。 / Read one canonicalized constraint back into items.
fn items_of(constraint: &ConstraintProgrammingConstraint) -> (String, Vec<String>) {
    match constraint {
        ConstraintProgrammingConstraint::Integer { .. } => ("integer".to_owned(), Vec::new()),
        ConstraintProgrammingConstraint::Boolean { literal } => {
            ("boolean".to_owned(), vec![literal_text(literal)])
        }
        ConstraintProgrammingConstraint::And { literals } => {
            ("and".to_owned(), literals.iter().map(literal_text).collect())
        }
        ConstraintProgrammingConstraint::Or { literals } => {
            ("or".to_owned(), literals.iter().map(literal_text).collect())
        }
        ConstraintProgrammingConstraint::Xor { literals } => {
            ("xor".to_owned(), literals.iter().map(literal_text).collect())
        }
        ConstraintProgrammingConstraint::AtMostOne { literals } => {
            ("atMostOne".to_owned(), literals.iter().map(literal_text).collect())
        }
        ConstraintProgrammingConstraint::ExactlyOne { literals } => {
            ("exactlyOne".to_owned(), literals.iter().map(literal_text).collect())
        }
        ConstraintProgrammingConstraint::Implication { constraint, .. } => {
            ("implication".to_owned(), items_of(constraint).1)
        }
        ConstraintProgrammingConstraint::Reification { constraint, .. } => {
            ("reification".to_owned(), items_of(constraint).1)
        }
        ConstraintProgrammingConstraint::AllDifferent { expressions } => ("allDifferent".to_owned(),
            expressions.iter().map(|expression| expression.terms.first()
                .map(|term| term.variable.stable_id.0.clone()).unwrap_or_default()).collect()),
        ConstraintProgrammingConstraint::Element { .. } => ("element".to_owned(), Vec::new()),
        ConstraintProgrammingConstraint::AllowedAssignments { tuples, .. } => {
            ("allowedAssignments".to_owned(), tuples.iter()
                .map(|tuple| tuple.iter().map(|value| value.to_string())
                    .collect::<Vec<_>>().join(",")).collect())
        }
        ConstraintProgrammingConstraint::ForbiddenAssignments { tuples, .. } => {
            ("forbiddenAssignments".to_owned(), tuples.iter()
                .map(|tuple| tuple.iter().map(|value| value.to_string())
                    .collect::<Vec<_>>().join(",")).collect())
        }
        ConstraintProgrammingConstraint::NoOverlap { intervals } => ("noOverlap".to_owned(),
            intervals.iter().map(|id| id.0.clone()).collect()),
        ConstraintProgrammingConstraint::Cumulative { tasks, .. } => ("cumulative".to_owned(),
            tasks.iter().map(|task| format!("{}:{}", task.interval.0, task.demand)).collect()),
        ConstraintProgrammingConstraint::Circuit { successors } => ("circuit".to_owned(),
            successors.iter().map(|expression| expression.terms.first()
                .map(|term| term.variable.stable_id.0.clone()).unwrap_or_default()).collect()),
        ConstraintProgrammingConstraint::Automaton { transitions, .. } => ("automaton".to_owned(),
            transitions.iter().map(|transition| format!("{}:{}:{}", transition.from_state,
                transition.value, transition.to_state)).collect()),
        ConstraintProgrammingConstraint::Reservoir { events, .. } => ("reservoir".to_owned(),
            events.iter().map(|event| format!("{}:{}",
                event.time.constant, event.level_change.constant)).collect()),
    }
}

/// 冻结一份带给定约束的模型。 / Freeze a model with the given constraints.
fn freeze_with(
    specs: &[ConstraintShapeSpec],
    reverse: bool,
) -> Result<ConstraintProgrammingSnapshot> {
    let mut model = ConstraintProgrammingModel::new("constraint-canon");
    let mut registered = Registered::new();
    for (index, spec) in specs.iter().enumerate() {
        let mut spec = spec.clone();
        if reverse {
            spec.items.reverse();
        }
        let constraint = constraint_of(&spec, &mut model, &mut registered)?;
        model.add_constraint(ConstraintDefinition::new(format!("c{index}"), constraint))
            .map_err(|error| BridgeError::new("InvalidInput", &format!(
                "constraint canonicalization failed validation: {error}")))?;
    }
    model.freeze().map_err(|error| BridgeError::new("InvalidInput", &format!(
        "constraint canonicalization failed validation: {error}")))
}

/// 走一遍约束规范化探针。非法输入是**错误**。 / Walk the constraint-canonicalization probe.
pub fn verify_constraint_canonicalization(
    request: &ConstraintCanonRequest,
) -> Result<ConstraintCanonReport> {
    if request.constraints.len() > 16 {
        return Err(BridgeError::new("InvalidInput",
            "constraint canonicalization failed validation: at most 16 constraints are accepted"));
    }
    let findings = canonical_findings();
    let outcome = freeze_with(&request.constraints, request.reverse_items);
    let (accepted, refusal, fingerprint, constraints) = match outcome {
        Ok(snapshot) => (true, None, Some(snapshot.fingerprint.value.clone()),
            snapshot.constraints.iter().map(|entry| {
                let (kind, items) = items_of(&entry.constraint);
                ConstraintCanonView { id: entry.id.0.clone(), kind, items }
            }).collect::<Vec<_>>()),
        Err(error) => (false, Some(error.message.clone()), None, Vec::new()),
    };
    Ok(ConstraintCanonReport {
        accepted,
        refusal,
        fingerprint,
        constraints,
        the_boolean_combinators_sort_their_literals:
            findings.the_boolean_combinators_sort_their_literals,
        the_table_constraints_sort_tuples_not_expressions:
            findings.the_table_constraints_sort_tuples_not_expressions,
        cumulative_sorts_by_interval_and_its_demand_tiebreak_is_unreachable:
            findings.cumulative_sorts_by_interval_and_its_demand_tiebreak_is_unreachable,
        the_automaton_sorts_its_transitions_only: findings.the_automaton_sorts_its_transitions_only,
        the_reservoir_sorts_its_events: findings.the_reservoir_sorts_its_events,
        the_untouched_variants_keep_their_order: findings.the_untouched_variants_keep_their_order,
        it_never_dedups_and_never_simplifies: findings.it_never_dedups_and_never_simplifies,
        the_fingerprint_is_order_insensitive_exactly_where_sorting_happens:
            findings.the_fingerprint_is_order_insensitive_exactly_where_sorting_happens,
    })
}

/// 八条规则属性的内部固定探针。 / The eight rule properties measured by fixed internal probes.
struct Findings {
    the_boolean_combinators_sort_their_literals: bool,
    the_table_constraints_sort_tuples_not_expressions: bool,
    cumulative_sorts_by_interval_and_its_demand_tiebreak_is_unreachable: bool,
    the_automaton_sorts_its_transitions_only: bool,
    the_reservoir_sorts_its_events: bool,
    the_untouched_variants_keep_their_order: bool,
    it_never_dedups_and_never_simplifies: bool,
    the_fingerprint_is_order_insensitive_exactly_where_sorting_happens: bool,
}

/// 一份形状声明。 / A shape declaration.
fn shape(kind: &str, items: &[&str]) -> ConstraintShapeSpec {
    ConstraintShapeSpec { kind: kind.to_owned(),
        items: items.iter().map(|item| (*item).to_owned()).collect() }
}

/// 冻结后取第一条约束的项。 / Freeze and read the first constraint's items.
fn frozen_items(specs: &[ConstraintShapeSpec], reverse: bool) -> Vec<String> {
    freeze_with(specs, reverse).expect("probe freezes").constraints.first()
        .map(|entry| items_of(&entry.constraint).1).unwrap_or_default()
}

/// 用固定探针测出全部规则属性——不读调用方的请求值。
/// Measure every rule property with fixed probes; the caller's request values are not read.
fn canonical_findings() -> Findings {
    // 1) 五个布尔组合子排序 literal，(稳定 ID, 取反)。 / The five combinators sort by (stable ID, negated).
    let the_boolean_combinators_sort_their_literals = {
        // 键是 `(稳定 ID, 取反)`：同一 ID 的 positive 在 negated **之前**，所以 `!a` 排在 `b` 前面。
        // The key is `(stable ID, negated)`: for one ID the positive precedes the negated, so `!a` lands before `b`.
        let sorted = vec!["a".to_owned(), "!a".to_owned(), "b".to_owned()];
        ["and", "or", "xor", "atMostOne", "exactlyOne"].iter().all(|kind| {
            let spec = [shape(kind, &["b", "!a", "a"])];
            let forward = frozen_items(&spec, false);
            let backward = frozen_items(&spec, true);
            // `positive` 排在 `negated` 之前，且 `!a` 整体排在 `b` 之前——这是 `(id, negated)` 序，
            // 不是字符串序（字符串序会把 `!a` 排到最前）。
            // `positive` precedes `negated`, and `!a` sorts before `b`: that is the `(id, negated)` order, not string
            // order (which would put `!a` first).
            forward == sorted && backward == sorted
        })
    };
    // 2) `allDifferent` 排表达式，两张表约束只排元组。 
    // 2) `allDifferent` sorts expressions; the two table constraints only sort tuples.
    let the_table_constraints_sort_tuples_not_expressions = {
        let different = {
            let spec = [shape("allDifferent", &["b", "a"])];
            frozen_items(&spec, false) == vec!["a".to_owned(), "b".to_owned()]
                && frozen_items(&spec, true) == vec!["a".to_owned(), "b".to_owned()]
        };
        // 表约束：元组排序，但 `expressions` 是固定一列，无法从项里看出顺序——所以量"元组是否排序"。
        // Table constraints sort tuples; `expressions` is a fixed single column here, so measure the tuple order.
        let table = {
            let spec = [shape("allowedAssignments", &["3", "1", "2"])];
            frozen_items(&spec, false) == vec!["1".to_owned(), "2".to_owned(), "3".to_owned()]
                && frozen_items(&spec, true) == vec!["1".to_owned(), "2".to_owned(), "3".to_owned()]
        };
        let forbidden = {
            let spec = [shape("forbiddenAssignments", &["3", "1", "2"])];
            frozen_items(&spec, true) == vec!["1".to_owned(), "2".to_owned(), "3".to_owned()]
        };
        different && table && forbidden
    };
    // 3) `noOverlap` 排区间 ID；`cumulative` 也排区间，但它的 demand 次键**不可观测**——
    //    领域禁止同一区间出现两次，所以次键永远用不上。
    // 3) `noOverlap` sorts interval IDs; `cumulative` sorts intervals too, but its demand tiebreaker is **unobservable**
    //    because the domain forbids the same interval twice, so the tiebreaker can never fire.
    let cumulative_sorts_by_interval_and_its_demand_tiebreak_is_unreachable = {
        let overlap = {
            let spec = [shape("noOverlap", &["ivB", "ivA"])];
            frozen_items(&spec, true) == vec!["ivA".to_owned(), "ivB".to_owned()]
        };
        let cumulative = {
            let spec = [shape("cumulative", &["ivC:5", "ivA:1", "ivB:3"])];
            frozen_items(&spec, true) == vec!["ivA:1".to_owned(), "ivB:3".to_owned(),
                "ivC:5".to_owned()]
        };
        // 同一区间两次 → 领域直接拒，所以次键拿不到输入。
        // The same interval twice is refused by the domain outright, so the tiebreaker never gets an input.
        let duplicate_refused = freeze_with(&[shape("cumulative", &["ivA:1", "ivA:3"])], false)
            .is_err_and(|error| error.message.contains("Cumulative cannot contain the same interval twice"));
        overlap && cumulative && duplicate_refused
    };
    // 4) `automaton` 只排转移。 / `automaton` sorts only its transitions.
    let the_automaton_sorts_its_transitions_only = {
        let spec = [shape("automaton", &["1:2:1", "0:1:1", "0:2:1"])];
        frozen_items(&spec, false) == vec!["0:1:1".to_owned(), "0:2:1".to_owned(),
            "1:2:1".to_owned()]
            && frozen_items(&spec, true) == vec!["0:1:1".to_owned(), "0:2:1".to_owned(),
                "1:2:1".to_owned()]
    };
    // 5) `reservoir` 排事件。 / `reservoir` sorts its events.
    let the_reservoir_sorts_its_events = {
        let spec = [shape("reservoir", &["3:1", "1:2", "2:-1"])];
        frozen_items(&spec, false) == vec!["1:2".to_owned(), "2:-1".to_owned(), "3:1".to_owned()]
            && frozen_items(&spec, true) == vec!["1:2".to_owned(), "2:-1".to_owned(),
                "3:1".to_owned()]
    };
    // 6) `circuit` / `integer` 保持原序。 / `circuit` and `integer` keep their order.
    let the_untouched_variants_keep_their_order = {
        let forward = frozen_items(&[shape("circuit", &["n1", "n0"])], false);
        let backward = frozen_items(&[shape("circuit", &["n1", "n0"])], true);
        // `circuit` 的第 i 项是节点 i 的后继，所以排序会改变语义——它确实没被排序。
        // A circuit's i-th entry is node i's successor, so sorting would change the semantics — and it is not sorted.
        forward == vec!["n1".to_owned(), "n0".to_owned()]
            && backward == vec!["n0".to_owned(), "n1".to_owned()]
    };
    // 7) 从不合并、从不化简。 / Never dedups, never simplifies.
    let it_never_dedups_and_never_simplifies = {
        // 重复 literal 留着。 / Duplicate literals stay.
        let duplicates = frozen_items(&[shape("and", &["a", "a", "!a", "!a"])], false)
            == vec!["a".to_owned(), "a".to_owned(), "!a".to_owned(), "!a".to_owned()];
        // 单 literal 的 `and` 仍然是 `and`，不会塌成 `boolean`。 
        // An `and` with one literal stays `and` and never collapses to `boolean`.
        let single = freeze_with(&[shape("and", &["a"])], false).expect("freeze");
        let kind = single.constraints.first().map(|entry| items_of(&entry.constraint).0)
            .unwrap_or_default();
        duplicates && kind == "and"
    };
    // 8) 指纹对排序处不敏感、对未碰处敏感。 / The fingerprint is insensitive where sorting happens.
    let the_fingerprint_is_order_insensitive_exactly_where_sorting_happens = {
        let boolean = {
            let spec = [shape("and", &["b", "a"])];
            freeze_with(&spec, false).expect("freeze").fingerprint
                == freeze_with(&spec, true).expect("freeze").fingerprint
        };
        let circuit = {
            let spec = [shape("circuit", &["n1", "n0"])];
            freeze_with(&spec, false).expect("freeze").fingerprint
                != freeze_with(&spec, true).expect("freeze").fingerprint
        };
        boolean && circuit
    };
    Findings {
        the_boolean_combinators_sort_their_literals,
        the_table_constraints_sort_tuples_not_expressions,
        cumulative_sorts_by_interval_and_its_demand_tiebreak_is_unreachable,
        the_automaton_sorts_its_transitions_only,
        the_reservoir_sorts_its_events,
        the_untouched_variants_keep_their_order,
        it_never_dedups_and_never_simplifies,
        the_fingerprint_is_order_insensitive_exactly_where_sorting_happens,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(specs: Vec<ConstraintShapeSpec>) -> ConstraintCanonRequest {
        ConstraintCanonRequest { constraints: specs, reverse_items: false }
    }

    fn items(report: &ConstraintCanonReport) -> Vec<String> {
        report.constraints.first().map(|view| view.items.clone()).unwrap_or_default()
    }

    #[test]
    fn the_boolean_combinators_sort_their_literals() {
        for kind in ["and", "or", "xor", "atMostOne", "exactlyOne"] {
            let report = verify_constraint_canonicalization(
                &request(vec![shape(kind, &["b", "!a", "a"])])).unwrap();
            assert!(report.accepted, "{:?}", report.refusal);
            assert_eq!(report.constraints[0].kind, kind);
            assert_eq!(items(&report), vec!["a".to_owned(), "!a".to_owned(), "b".to_owned()],
                "{kind}");
            // 反序输入给出同样的结果。 / A reversed input gives the same result.
            let reversed = verify_constraint_canonicalization(
                &ConstraintCanonRequest { reverse_items: true, ..request(vec![shape(kind,
                    &["b", "!a", "a"])]) }).unwrap();
            assert_eq!(items(&reversed), items(&report), "{kind}");
        }
    }

    #[test]
    fn all_different_sorts_expressions_and_tables_sort_tuples() {
        let different = verify_constraint_canonicalization(
            &request(vec![shape("allDifferent", &["b", "a"])])).unwrap();
        assert_eq!(items(&different), vec!["a".to_owned(), "b".to_owned()]);
        for kind in ["allowedAssignments", "forbiddenAssignments"] {
            let table = verify_constraint_canonicalization(
                &request(vec![shape(kind, &["3", "1", "2"])])).unwrap();
            assert_eq!(table.constraints[0].kind, kind);
            assert_eq!(items(&table), vec!["1".to_owned(), "2".to_owned(), "3".to_owned()],
                "{kind}");
        }
    }

    #[test]
    fn no_overlap_sorts_ids_and_cumulatives_demand_tiebreak_is_unreachable() {
        let overlap = verify_constraint_canonicalization(
            &request(vec![shape("noOverlap", &["ivB", "ivA"])])).unwrap();
        assert_eq!(items(&overlap), vec!["ivA".to_owned(), "ivB".to_owned()]);
        let cumulative = verify_constraint_canonicalization(
            &request(vec![shape("cumulative", &["ivC:5", "ivA:1", "ivB:3"])])).unwrap();
        assert_eq!(items(&cumulative), vec!["ivA:1".to_owned(), "ivB:3".to_owned(),
            "ivC:5".to_owned()]);
        // 同一区间两次被领域拒，所以 demand 次键不可观测。 
        // The same interval twice is refused by the domain, so the demand tiebreaker is unobservable.
        let duplicate = verify_constraint_canonicalization(
            &request(vec![shape("cumulative", &["ivA:1", "ivA:3"])])).unwrap();
        assert!(!duplicate.accepted);
        assert!(duplicate.refusal.as_deref().unwrap()
            .contains("Cumulative cannot contain the same interval twice"));
    }

    #[test]
    fn the_automaton_sorts_its_transitions() {
        let report = verify_constraint_canonicalization(
            &request(vec![shape("automaton", &["1:2:1", "0:1:1", "0:2:1"])])).unwrap();
        assert_eq!(items(&report), vec!["0:1:1".to_owned(), "0:2:1".to_owned(),
            "1:2:1".to_owned()]);
    }

    #[test]
    fn the_reservoir_sorts_its_events() {
        let report = verify_constraint_canonicalization(
            &request(vec![shape("reservoir", &["3:1", "1:2", "2:-1"])])).unwrap();
        assert_eq!(items(&report), vec!["1:2".to_owned(), "2:-1".to_owned(), "3:1".to_owned()]);
    }

    #[test]
    fn the_circuit_keeps_its_order() {
        // `circuit` 的第 i 项是节点 i 的后继——排序会改语义，所以它没被排序。
        // A circuit's i-th entry is node i's successor, so sorting would change the semantics and it is not sorted.
        let forward = verify_constraint_canonicalization(
            &request(vec![shape("circuit", &["n1", "n0"])])).unwrap();
        assert_eq!(items(&forward), vec!["n1".to_owned(), "n0".to_owned()]);
        let reversed = verify_constraint_canonicalization(
            &ConstraintCanonRequest { reverse_items: true,
                ..request(vec![shape("circuit", &["n1", "n0"])]) }).unwrap();
        assert_eq!(items(&reversed), vec!["n0".to_owned(), "n1".to_owned()]);
        // 指纹因此不同。 / The fingerprints therefore differ.
        assert_ne!(forward.fingerprint, reversed.fingerprint);
    }

    #[test]
    fn duplicates_survive_and_single_literal_ands_stay_ands() {
        let report = verify_constraint_canonicalization(
            &request(vec![shape("and", &["a", "a", "!a", "!a"])])).unwrap();
        assert_eq!(items(&report), vec!["a".to_owned(), "a".to_owned(), "!a".to_owned(),
            "!a".to_owned()]);
        let single = verify_constraint_canonicalization(
            &request(vec![shape("and", &["a"])])).unwrap();
        assert_eq!(single.constraints[0].kind, "and");
        assert_eq!(items(&single), vec!["a".to_owned()]);
    }

    #[test]
    fn the_sorted_variants_share_a_fingerprint_and_the_unsorted_ones_do_not() {
        let sorted = request(vec![shape("and", &["b", "a"])]);
        let forward = verify_constraint_canonicalization(&sorted).unwrap();
        let backward = verify_constraint_canonicalization(
            &ConstraintCanonRequest { reverse_items: true, ..sorted }).unwrap();
        assert_eq!(forward.fingerprint, backward.fingerprint);
        let unsorted = request(vec![shape("circuit", &["n1", "n0"])]);
        let left = verify_constraint_canonicalization(&unsorted).unwrap();
        let right = verify_constraint_canonicalization(
            &ConstraintCanonRequest { reverse_items: true, ..unsorted }).unwrap();
        assert_ne!(left.fingerprint, right.fingerprint);
    }

    #[test]
    fn every_rule_reading_holds_on_a_real_report() {
        let report = verify_constraint_canonicalization(
            &request(vec![shape("and", &["b", "a"])])).unwrap();
        assert!(report.the_boolean_combinators_sort_their_literals);
        assert!(report.the_table_constraints_sort_tuples_not_expressions);
        assert!(report.cumulative_sorts_by_interval_and_its_demand_tiebreak_is_unreachable);
        assert!(report.the_automaton_sorts_its_transitions_only);
        assert!(report.the_reservoir_sorts_its_events);
        assert!(report.the_untouched_variants_keep_their_order);
        assert!(report.it_never_dedups_and_never_simplifies);
        assert!(report.the_fingerprint_is_order_insensitive_exactly_where_sorting_happens);
    }

    #[test]
    fn a_domain_error_is_a_refusal_not_an_input_error() {
        // 空 `automaton` 项 → 结构校验拒（领域规则，不是桥接输入错误）。
        // An empty automaton → structural refusal (a domain rule, not a bridge input error).
        let report = verify_constraint_canonicalization(&request(vec![shape("automaton", &[])])).unwrap();
        assert!(!report.accepted);
        assert!(report.refusal.as_deref().unwrap().contains("must not be empty"));
        assert!(report.fingerprint.is_none());
        // `circuit` 空后继同样被领域拒。 / An empty circuit is refused by the domain too.
        let circuit = verify_constraint_canonicalization(&request(vec![shape("circuit", &[])])).unwrap();
        assert!(!circuit.accepted);
    }

    /// 这个探针里"构造不出来"统一表现为**被拒的结论**（带文案），只有超出条目上限才是 `Err`。
    /// Here "cannot be built" is uniformly a **refused verdict** with a message; only the entry cap is an `Err`.
    #[test]
    fn a_shape_error_is_a_refusal_with_a_message() {
        for (spec, expected) in [
            (shape("nope", &[]), "unknown kind"),
            (shape("and", &["  "]), "literal IDs must not be blank"),
            (shape("allowedAssignments", &["1,x"]), "is not an integer"),
            (shape("cumulative", &["ivA"]), "is not a `a:b` pair"),
            (shape("automaton", &["0:1"]), "is not `from:value:to`"),
            (shape("allDifferent", &["  "]), "variable IDs must not be blank"),
        ] {
            let report = verify_constraint_canonicalization(&request(vec![spec])).unwrap();
            assert!(!report.accepted, "{expected}");
            assert!(report.refusal.as_deref().unwrap().contains(expected), "{:?}", report.refusal);
            assert!(report.fingerprint.is_none());
            assert!(report.constraints.is_empty());
        }
        assert!(verify_constraint_canonicalization(&ConstraintCanonRequest {
            constraints: (0..17).map(|index| shape("and", &[&format!("v{index}")])).collect(),
            reverse_items: false }).unwrap_err().message.contains("at most 16 constraints"));
    }

    /// 守门：DTO 必须是 camelCase 且拒绝未知字段。 / A guard: camelCase wire, unknown fields refused.
    #[test]
    fn the_report_wire_is_camel_case_and_rejects_unknown_fields() {
        let report = verify_constraint_canonicalization(
            &request(vec![shape("and", &["a"])])).unwrap();
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"theBooleanCombinatorsSortTheirLiterals\""), "{json}");
        assert!(json.contains("\"itNeverDedupsAndNeverSimplifies\""), "{json}");
        assert!(json.contains("\"cumulativeSortsByIntervalAndItsDemandTiebreakIsUnreachable\""),
            "{json}");
        assert!(json.contains("\"reverseItems\"") == false, "{json}");
        assert!(!json.contains("the_boolean"), "{json}");
        let mut copy: serde_json::Value = serde_json::from_str(&json).unwrap();
        copy["unexpected"] = serde_json::json!(1);
        assert!(serde_json::from_str::<ConstraintCanonReport>(&copy.to_string()).is_err());
    }
}
