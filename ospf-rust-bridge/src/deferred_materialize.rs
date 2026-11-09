//! 续挖第 39 单：延迟函数物化的**全有或全无**与"物化/丢弃"两侧互斥的索引契约
//! （`model/mechanism/basic_mechanism_model.rs` 的 `resolve_deferred_functions` /
//! `materialize_deferred_functions_at` / `apply_native_lowering`）
//! Continuation, delivery 39: the all-or-nothing materialization of deferred functions, and the mutually exclusive
//! materialize/drop index contract (`resolve_deferred_functions`, `materialize_deferred_functions_at`,
//! `apply_native_lowering`).
//!
//! 这是 §11.14 里"价值最高、需要真实结构"的那一块：本单用**可观测的假结构**把契约量出来（结构只记录
//! 自己被物化了几次、以及要不要失败），因此不需要真的去建一个完整 LP。实测六条：
//!
//! 1. **失败不写一行、不移除任何结构**（`failure_writes_nothing()`）：`materialize` 返回错误时
//!    `constraints().len()` 与 `deferred_functions().len()` 都**原样不变**。所以"先全部生成再一次性追加"
//!    不是风格问题，而是"失败不留半成品"的唯一实现方式。
//! 2. **索引会排序并去重**（`indices_are_sorted_and_deduplicated()`）：`[2, 0, 2]` 等价于 `[0, 2]`，
//!    所以**重复索引不是错误**，"物化了几个"按去重后的个数算。
//! 3. **越界是错误**（`out_of_range_is_refused()`）：`index >= pending_len` 直接报
//!    "is out of range for N pending structures"。
//! 4. **"同时出现在物化侧与丢弃侧"那句报错桥接**量不到**：它只在私有入口 `resolve_deferred_functions` 里，
//!    `materialize_deferred_functions_at` 的丢弃侧恒为空，`apply_native_lowering` 的两个下标集又互斥推导。
//!    这一层只量它能观测到的后果：一次调用里同一结构不会被物化两次
//!    （`oneStructureIsNeverMaterializedTwice()`）——**诚实标注，不假装量过**。
//! 5. **成功后只移除被处理的那些，其余保持原顺序**（`untouched_structures_keep_their_order()`）：
//!    物化 `[0, 2]` 后剩下的是原来第 1、3 个，顺序不变。
//! 6. **`materialize_deferred_functions()` 是幂等的**（`repeat_is_a_noop()`）：第二次调用时列表已空，
//!    直接返回 `Ok`，不新增约束。
//!
//! This is the "highest value, needs real structures" part of §11.14: this delivery measures the contract with an
//! **observable fake structure** (it only records how often it was materialized and whether it should fail), so no full LP is
//! needed. Six measured rules. **A failure writes no row and removes nothing** (`failure_writes_nothing()`): when
//! `materialize` returns an error both `constraints().len()` and `deferred_functions().len()` stay **unchanged**, so
//! "generate everything then append once" is not a style choice but the only way to avoid a half-built model. **Indices are
//! sorted and deduplicated** (`indices_are_sorted_and_deduplicated()`): `[2, 0, 2]` equals `[0, 2]`, so a **duplicate index
//! is not an error** and the count is the deduplicated one. **Out of range is an error** (`out_of_range_is_refused()`):
//! `index >= pending_len` reports "is out of range for N pending structures". The "both materialized and dropped" error is
//! **not reachable** from this layer — it lives in the private `resolve_deferred_functions`, the drop side is always empty
//! here and `apply_native_lowering` derives two disjoint sets — so only the observable consequence is measured:
//! **one structure is never materialized twice per call** (`one_structure_is_never_materialized_twice()`).
//! **On success only the handled structures are removed and the rest keep their order**
//! (`untouched_structures_keep_their_order()`): materializing `[0, 2]` leaves the original 1 and 3 in place. And
//! **`materialize_deferred_functions()` is idempotent** (`repeat_is_a_noop()`): the second call sees an empty list and returns
//! `Ok` without adding a single constraint.

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use ospf_rust_core::model::LinearConstraint;
use ospf_rust_core::model::intermediate::deferred_function::DeferredFunctionStructure;
use ospf_rust_core::model::mechanism::basic_mechanism_model::BasicMechanismModel;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 一个假结构：只记录物化次数，并可按请求失败。
/// A fake structure: it records how often it was materialized and can fail on request.
#[derive(Debug)]
struct ProbeStructure {
    name: String,
    rows: usize,
    fail: bool,
    calls: Arc<AtomicUsize>,
}

impl DeferredFunctionStructure<f64> for ProbeStructure {
    fn function_name(&self) -> &str { &self.name }

    fn materialize(
        &self,
        _symbol_to_index: &HashMap<usize, usize>,
    ) -> ospf_rust_core::Result<Vec<LinearConstraint<f64>>> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if self.fail {
            return Err(ospf_rust_core::error::CoreError::from(
                ospf_rust_core::error::ModelError::InvalidConstraint(format!(
                    "probe structure {} refuses to materialize", self.name))));
        }
        // 每一行都是同一个常数 0 约束：本单只数行数，不看内容。
        // Every row is the same constant-zero constraint: only the row count matters here.
        Ok((0..self.rows)
            .map(|_| LinearConstraint::new(
                ospf_rust_core::model::mechanism::constraint::LinearInequality::less_equal(
                    ospf_rust_core::model::flatten::Linear::zero(0.0), 0.0), ""))
            .collect())
    }

    fn as_any(&self) -> &dyn std::any::Any { self }
}

/// 一个结构的字面量。 / One structure literal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StructureSpec {
    /// 结构名。 / The structure name.
    pub name: String,
    /// 物化产出的行数。 / How many rows it produces.
    #[serde(default)]
    pub rows: u32,
    /// 是否拒绝物化。 / Whether it refuses to materialize.
    #[serde(default)]
    pub fail: bool,
}

/// 物化请求。 / The materialization request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MaterializeRequest {
    /// 待物化的结构。 / The pending structures.
    pub structures: Vec<StructureSpec>,
    /// 要物化的位置（**允许重复与乱序**）。 / The positions to materialize (**duplicates and any order allowed**).
    #[serde(default)]
    pub indices: Vec<u32>,
    /// 是否在成功后再调用一次全量物化（验幂等）。 / Whether to call the full materialization again afterwards.
    #[serde(default)]
    pub repeat_after_success: bool,
}

/// 每个结构的物化次数。 / One structure's materialization count.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StructureTrace {
    /// 结构名。 / The structure name.
    pub name: String,
    /// 被物化的次数。 / How often it was materialized.
    pub calls: u32,
}

/// 物化结论。 / The materialization verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MaterializeReport {
    /// 物化是否成功。 / Whether materialization succeeded.
    pub succeeded: bool,
    /// 失败时的错误消息。 / The error message when it failed.
    #[serde(default)]
    pub error: Option<String>,
    /// 最终约束行数。 / The final constraint row count.
    pub constraint_count: u64,
    /// 最终待物化结构数。 / The final pending count.
    pub pending_count: u64,
    /// 最终待物化的结构名（保持原顺序）。 / The remaining structure names (in their original order).
    pub remaining: Vec<String>,
    /// 每个结构的物化次数。 / Each structure's materialization count.
    pub traces: Vec<StructureTrace>,
    /// 失败不写一行（**是**）。 / Whether a failure writes nothing (**yes**).
    pub failure_writes_nothing: bool,
    /// 索引会排序去重（**是**）。 / Whether indices are sorted and deduplicated (**yes**).
    pub indices_are_sorted_and_deduplicated: bool,
    /// 越界是错误（**是**）。 / Whether out-of-range is an error (**yes**).
    pub out_of_range_is_refused: bool,
    /// 一次调用里同一结构不会被物化两次（**是**）。 / Whether one structure is never materialized twice per call
    /// (**yes**).
    pub one_structure_is_never_materialized_twice: bool,
    /// 未处理的结构保持原顺序（**是**）。 / Whether untouched structures keep their order (**yes**).
    pub untouched_structures_keep_their_order: bool,
    /// 重复调用是空操作（**是**）。 / Whether a repeat call is a no-op (**yes**).
    pub repeat_is_a_noop: bool,
}

/// 结构条数上限（信任边界）。 / The structure cap (a trust boundary).
const MAX_STRUCTURES: usize = 64;

/// 造一个带结构的模型。 / Build a model carrying the structures.
fn model_with(structures: &[StructureSpec]) -> (BasicMechanismModel<f64>, BTreeMap<String, Arc<AtomicUsize>>) {
    let mut model = BasicMechanismModel::<f64>::new("probe");
    let mut counters = BTreeMap::new();
    for spec in structures {
        let calls = Arc::new(AtomicUsize::new(0));
        counters.insert(spec.name.clone(), Arc::clone(&calls));
        model.add_deferred_function(Arc::new(ProbeStructure { name: spec.name.clone(),
            rows: spec.rows as usize, fail: spec.fail, calls }));
    }
    (model, counters)
}

/// 走一遍延迟物化。非法输入是**错误**。 / Walk the deferred materialization.
pub fn verify_materialize(request: &MaterializeRequest) -> Result<MaterializeReport> {
    if request.structures.is_empty() {
        return Err(BridgeError::new("InvalidInput",
            "deferred materialization failed validation: at least one structure is required"));
    }
    if request.structures.len() > MAX_STRUCTURES {
        return Err(BridgeError::new("InvalidInput", &format!(
            "deferred materialization failed validation: at most {MAX_STRUCTURES} structures are accepted")));
    }
    let mut names = std::collections::BTreeSet::new();
    for spec in &request.structures {
        if spec.name.trim().is_empty() {
            return Err(BridgeError::new("InvalidInput",
                "deferred materialization failed validation: structure names must not be blank"));
        }
        if !names.insert(spec.name.as_str()) {
            return Err(BridgeError::new("InvalidInput",
                "deferred materialization failed validation: the request repeats a structure"));
        }
    }
    let (mut model, counters) = model_with(&request.structures);
    let indices: Vec<usize> = request.indices.iter().map(|index| *index as usize).collect();
    let outcome = model.materialize_deferred_functions_at(&indices);
    let (succeeded, error) = match &outcome {
        Ok(()) => (true, None),
        Err(failure) => (false, Some(failure.to_string())),
    };
    // 幂等：成功后再全量物化一次。**按实测纠正**：第一次全量物化会把剩下的结构真的展开（行数会增加），
    // 所以"幂等"要在**列表已空之后**再调一次才测得出来——这才是源码注释说的"重复调用是空操作"。
    // Idempotence: run the full materialization once more after success. **Measured correction**: the first full pass really
    // expands the remaining structures (the row count grows), so idempotence must be measured **after the list is empty** —
    // that is what the source comment means by "repeated calls are no-ops".
    let mut repeat_leaked = false;
    if succeeded && request.repeat_after_success {
        let _ = model.materialize_deferred_functions();
        let before = model.constraints().len();
        let again = model.materialize_deferred_functions();
        repeat_leaked = again.is_err() || model.constraints().len() != before;
    }
    let findings = canonical_findings();
    Ok(MaterializeReport {
        succeeded,
        error,
        constraint_count: model.constraints().len() as u64,
        pending_count: model.deferred_functions().len() as u64,
        remaining: model.deferred_functions().iter().map(|structure| structure.function_name().to_owned()).collect(),
        traces: request.structures.iter().map(|spec| StructureTrace { name: spec.name.clone(),
            calls: counters.get(&spec.name).map(|counter| counter.load(Ordering::SeqCst) as u32)
                .unwrap_or(0) }).collect(),
        failure_writes_nothing: findings.failure_writes_nothing,
        indices_are_sorted_and_deduplicated: findings.indices_are_sorted_and_deduplicated,
        out_of_range_is_refused: findings.out_of_range_is_refused,
        one_structure_is_never_materialized_twice: findings.one_structure_is_never_materialized_twice,
        untouched_structures_keep_their_order: findings.untouched_structures_keep_their_order,
        repeat_is_a_noop: findings.repeat_is_a_noop && !repeat_leaked,
    })
}

/// 六条规则属性的内部固定探针。 / The six rule properties measured by fixed internal probes.
struct Findings {
    failure_writes_nothing: bool,
    indices_are_sorted_and_deduplicated: bool,
    out_of_range_is_refused: bool,
    one_structure_is_never_materialized_twice: bool,
    untouched_structures_keep_their_order: bool,
    repeat_is_a_noop: bool,
}

/// 四个探针结构。 / Four probe structures.
fn probe_specs() -> Vec<StructureSpec> {
    (0..4).map(|index| StructureSpec { name: format!("probe.{index}"), rows: 1, fail: false })
        .collect()
}

/// 用固定探针测出全部规则属性——不读调用方的请求值。
/// Measure every rule property with fixed probes; the caller's request values are not read.
fn canonical_findings() -> Findings {
    // 1) 失败不写一行、不移除任何结构。 / A failure writes no row and removes nothing.
    let failing = vec![StructureSpec { name: "probe.fail".to_owned(), rows: 1, fail: true },
        StructureSpec { name: "probe.ok".to_owned(), rows: 1, fail: false }];
    let (mut model, _) = model_with(&failing);
    let before_constraints = model.constraints().len();
    let failed = model.materialize_deferred_functions();
    let failure_writes_nothing = failed.is_err() && model.constraints().len() == before_constraints
        && model.deferred_functions().len() == failing.len();
    // 2) 索引排序去重：`[2, 0, 2]` 只物化两个。 / Indices dedupe: `[2, 0, 2]` materializes two.
    let (mut model, counters) = model_with(&probe_specs());
    let ok = model.materialize_deferred_functions_at(&[2, 0, 2]).is_ok();
    let materialized = counters.values().filter(|counter| counter.load(Ordering::SeqCst) == 1).count();
    let touched_twice = counters.values().any(|counter| counter.load(Ordering::SeqCst) > 1);
    let indices_are_sorted_and_deduplicated = ok && materialized == 2 && !touched_twice
        && model.deferred_functions().len() == 2;
    // 3) 未处理的结构保持原顺序。 / Untouched structures keep their order.
    let remaining: Vec<String> = model.deferred_functions().iter()
        .map(|structure| structure.function_name().to_owned()).collect();
    let untouched_structures_keep_their_order = remaining == vec!["probe.1".to_owned(),
        "probe.3".to_owned()];
    // 4) 越界是错误。 / Out of range is an error.
    let (mut model, _) = model_with(&probe_specs());
    let out_of_range_is_refused = model.materialize_deferred_functions_at(&[9]).is_err()
        && model.deferred_functions().len() == 4;
    // 5) **诚实标注**："同时出现在物化侧与丢弃侧"那句报错只有私有入口 `resolve_deferred_functions` 能触发，
    //    `materialize_deferred_functions_at` 的丢弃侧恒为空，`apply_native_lowering` 的两个下标集又互斥推导出来，
    //    所以**桥接这一层量不到那句错误**。这里只量它能观测到的后果：一次调用里同一结构不会被物化两次。
    // 5) **Honest note**: the "both materialized and dropped" error is only reachable through the private
    //    `resolve_deferred_functions` (the drop side is always empty here, and `apply_native_lowering` derives two disjoint
    //    sets), so this layer **cannot** measure that message. Only its observable consequence is measured: one structure is
    //    never materialized twice in a single call.
    let one_structure_is_never_materialized_twice = !touched_twice && indices_are_sorted_and_deduplicated;
    // 6) 重复调用是空操作。 / A repeat call is a no-op.
    let (mut model, _) = model_with(&probe_specs());
    let _ = model.materialize_deferred_functions();
    let after_first = model.constraints().len();
    let repeat = model.materialize_deferred_functions();
    let repeat_is_a_noop = repeat.is_ok() && model.constraints().len() == after_first
        && model.deferred_functions().is_empty();
    Findings { failure_writes_nothing, indices_are_sorted_and_deduplicated,
        out_of_range_is_refused, one_structure_is_never_materialized_twice,
        untouched_structures_keep_their_order, repeat_is_a_noop }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> MaterializeRequest {
        MaterializeRequest {
            structures: (0..4).map(|index| StructureSpec { name: format!("s.{index}"),
                rows: 1, fail: false }).collect(),
            indices: vec![0, 2],
            // **按实测纠正**：`repeat_after_success` 会把**剩下的**也物化（那正是"全量物化"的含义），
            // 所以想只观察子集效果就必须关掉它。
            // Measured correction: `repeat_after_success` materializes the **remaining** structures too (that is what
            // "materialize everything" means), so observing a subset requires turning it off.
            repeat_after_success: false,
        }
    }

    /// 成功后全量再物化一次：剩下的也会被展开，但仍不重复写行。
    /// A full re-materialization after success expands the rest too, without duplicating rows.
    #[test]
    fn a_repeat_call_expands_the_rest_and_stays_a_noop_afterwards() {
        let mut document = request();
        document.repeat_after_success = true;
        let report = verify_materialize(&document).unwrap();
        assert!(report.succeeded);
        assert_eq!(report.constraint_count, 4);
        assert_eq!(report.pending_count, 0);
        assert!(report.remaining.is_empty());
        assert!(report.repeat_is_a_noop);
    }

    #[test]
    fn materializing_a_subset_keeps_the_rest_in_order() {
        let report = verify_materialize(&request()).unwrap();
        assert!(report.succeeded);
        assert_eq!(report.constraint_count, 2);
        assert_eq!(report.pending_count, 2);
        // 未处理的保持原顺序。 / Untouched structures keep their order.
        assert_eq!(report.remaining, vec!["s.1".to_owned(), "s.3".to_owned()]);
        assert!(report.untouched_structures_keep_their_order);
        // 每个结构只被物化一次。 / Each structure is materialized at most once.
        assert_eq!(report.traces.iter().filter(|trace| trace.calls == 1).count(), 2);
        assert!(report.traces.iter().all(|trace| trace.calls <= 1));
        assert!(report.repeat_is_a_noop);
    }

    #[test]
    fn duplicate_and_unsorted_indices_are_merged() {
        let mut document = request();
        document.indices = vec![2, 0, 2];
        let report = verify_materialize(&document).unwrap();
        assert_eq!(report.constraint_count, 2);
        assert_eq!(report.pending_count, 2);
        assert!(report.indices_are_sorted_and_deduplicated);
    }

    #[test]
    fn a_failing_structure_writes_nothing() {
        let mut document = request();
        document.structures[2].fail = true;
        document.repeat_after_success = false;
        let report = verify_materialize(&document).unwrap();
        assert!(!report.succeeded);
        // 错误消息原样带出。 / The error message comes through verbatim.
        assert!(report.error.as_deref().unwrap_or_default().contains("refuses to materialize"));
        // 一行都没写、一个都没移除。 / No row written, nothing removed.
        assert_eq!(report.constraint_count, 0);
        assert_eq!(report.pending_count, 4);
        assert!(report.failure_writes_nothing);
    }

    #[test]
    fn out_of_range_is_refused_without_side_effects() {
        let mut document = request();
        document.indices = vec![9];
        document.repeat_after_success = false;
        let report = verify_materialize(&document).unwrap();
        assert!(!report.succeeded);
        assert!(report.error.as_deref().unwrap_or_default().contains("out of range"));
        assert_eq!(report.constraint_count, 0);
        assert_eq!(report.pending_count, 4);
        assert!(report.out_of_range_is_refused);
        assert!(report.one_structure_is_never_materialized_twice);
    }

    #[test]
    fn the_rule_properties_hold_regardless_of_the_request() {
        let mut other = request();
        other.structures = vec![StructureSpec { name: "only".to_owned(), rows: 3, fail: false }];
        other.indices = vec![0];
        let report = verify_materialize(&other).unwrap();
        assert_eq!(report.constraint_count, 3);
        assert_eq!(report.pending_count, 0);
        assert!(report.remaining.is_empty());
        assert!(report.failure_writes_nothing);
        assert!(report.indices_are_sorted_and_deduplicated);
        assert!(report.out_of_range_is_refused);
        assert!(report.one_structure_is_never_materialized_twice);
        assert!(report.untouched_structures_keep_their_order);
        assert!(report.repeat_is_a_noop);
    }

    #[test]
    fn bad_input_is_refused() {
        let mut empty = request();
        empty.structures = Vec::new();
        assert!(verify_materialize(&empty).unwrap_err().message
            .contains("at least one structure"));
        let mut blank = request();
        blank.structures[0].name = "  ".to_owned();
        assert!(verify_materialize(&blank).unwrap_err().message
            .contains("structure names must not be blank"));
        let mut duplicate = request();
        duplicate.structures[1].name = "s.0".to_owned();
        assert!(verify_materialize(&duplicate).unwrap_err().message
            .contains("repeats a structure"));
    }

    /// 守门：DTO 必须是 camelCase 且拒绝未知字段。 / A guard: camelCase wire, unknown fields refused.
    #[test]
    fn the_report_wire_is_camel_case_and_rejects_unknown_fields() {
        let report = verify_materialize(&request()).unwrap();
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"failureWritesNothing\""), "{json}");
        assert!(json.contains("\"indicesAreSortedAndDeduplicated\""), "{json}");
        assert!(json.contains("\"constraintCount\""), "{json}");
        assert!(!json.contains("failure_writes"), "{json}");
        let mut copy: serde_json::Value = serde_json::from_str(&json).unwrap();
        copy["unexpected"] = serde_json::json!(1);
        assert!(serde_json::from_str::<MaterializeReport>(&copy.to_string()).is_err());
    }
}
