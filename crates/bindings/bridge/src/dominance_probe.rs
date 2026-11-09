//! 续挖第 81 单：**支配关系**——判据只看方向表长度、`NaN` 被当作"相等"而不是"毒药"、方向表短了会**截断**
//! （`solver/heuristic/operator/multi_objective/dominance.rs` 的 `Dominance` / `ParetoDominance` /
//! `MinimizationDominance`）
//! Continuation, delivery 81: the **dominance relation** — the predicate reads only the direction-list length, `NaN` is
//! treated as "equal" rather than as poison, and a short direction list **truncates** the comparison
//! (`Dominance` / `ParetoDominance` / `MinimizationDominance` in
//! `solver/heuristic/operator/multi_objective/dominance.rs`).
//!
//! 启发式层此前**整块没绑过**；这一单挑它最底层的偏序判据——一个求解器、一个算子都不需要，纯数据。八条实测：
//!
//! 1. **它是严格偏序**（`the_relation_is_a_strict_partial_order()`）：不自反、不对称、可传递。
//! 2. **方向表短了就截断**（`a_shorter_direction_list_truncates_the_comparison()`）：循环走
//!    `0..directions.len()`，**多出来的目标维度被静默忽略**——所以"声明了几个方向"决定比几维，不是向量长度。
//! 3. **方向表长了就一律拒绝**（`a_longer_direction_list_refuses_every_comparison()`）：索引取不到直接 `false`。
//! 4. **`NaN` 被当作"相等"而不是"毒药"**（`nan_counts_as_equal_not_as_poison()`）：`<` 与 `>` 同时为假，
//!    于是既不 `better` 也不 `worse`；只要另一维严格更好，带 `NaN` 的向量**照样能支配**。本单最值钱的一条。
//! 5. **无穷按普通数值比较**（`infinities_compare_normally()`）：`±inf` 没有特殊分支。
//! 6. **空方向表永不支配**（`an_empty_direction_list_never_dominates()`）：`strictly_better` 起手 `false`。
//! 7. **`MinimizationDominance` 是纯委托**（`minimization_is_a_pure_delegation()`）：与
//!    `ParetoDominance::minimization(n)` 逐项相同，且内部字段私有、无访问器。
//! 8. **混合方向让两个向量互不可比**（`mixed_directions_leave_vectors_incomparable()`）。
//!
//! The heuristic layer had never been bound; this delivery takes its bottom-most partial-order predicate — no solver and
//! no operator needed, pure data. **The relation is a strict partial order** (irreflexive, asymmetric, transitive).
//! **A short direction list truncates** — the loop walks `0..directions.len()`, so the extra objective dimensions are
//! **silently ignored**; how many directions you declare, not the vector length, decides how many axes are compared.
//! **A long direction list refuses every comparison.** **`NaN` counts as "equal" rather than as poison** — `<` and `>`
//! are both false, so it is neither better nor worse, and a vector carrying `NaN` **still dominates** as long as another
//! axis is strictly better. This is the most valuable finding here. **Infinities compare normally.** **An empty direction
//! list never dominates.** **`MinimizationDominance` is a pure delegation** and its inner field is private with no
//! accessor. And **mixed directions leave two vectors incomparable**.

use ospf_rust_core_plugin_heuristic::heuristic::operator::multi_objective::dominance::{
    Dominance, MinimizationDominance, ParetoDominance,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 一对目标向量。 / One pair of objective vectors.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DominancePairSpec {
    /// 左向量（候选）。 / The left vector (the candidate).
    #[serde(default)]
    pub lhs: Vec<f64>,
    /// 右向量（当前）。 / The right vector (the incumbent).
    #[serde(default)]
    pub rhs: Vec<f64>,
}

/// 支配关系探针请求。 / The dominance probe request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DominanceRequest {
    /// 构造方式：`directions`（逐目标方向）或 `minimization`（全最小化）。
    /// The construction mode: `directions` (per-objective direction) or `minimization` (all-minimisation).
    pub mode: String,
    /// 每个目标的优化方向，`true` 表示最小化（`directions` 模式用）。
    /// The optimisation direction per objective, `true` means minimisation (used by `directions`).
    #[serde(default)]
    pub directions: Vec<bool>,
    /// 目标个数（`minimization` 模式用）。 / The objective amount (used by `minimization`).
    #[serde(default)]
    pub objective_amount: Option<u32>,
    /// 要问的向量对。 / The vector pairs to ask about.
    #[serde(default)]
    pub pairs: Vec<DominancePairSpec>,
    /// 是否也问一次反向（`lhs` 与 `rhs` 对调）。 / Whether to also ask the reversed direction.
    #[serde(default)]
    pub with_reverse: Option<bool>,
}

/// 一对向量的结论。 / One pair's verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DominancePairView {
    /// `lhs` 是否支配 `rhs`。 / Whether `lhs` dominates `rhs`.
    pub forward: bool,
    /// `rhs` 是否支配 `lhs`（未请求时为 `false`）。 / Whether `rhs` dominates `lhs` (`false` when not asked).
    pub reverse: bool,
}

/// 支配关系探针结论。 / The dominance probe verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DominanceReport {
    /// 构造方式回显。 / The construction mode echoed back.
    pub mode: String,
    /// 实际参与比较的目标维度数（`directions.len()` 或 `objective_amount`）。
    /// How many objective axes actually take part.
    pub compared_axes: u32,
    /// 每对向量的结论。 / Each pair's verdict.
    pub pairs: Vec<DominancePairView>,
    /// 八条规则属性的读数。 / The eight rule readings.
    pub the_relation_is_a_strict_partial_order: bool,
    pub a_shorter_direction_list_truncates_the_comparison: bool,
    pub a_longer_direction_list_refuses_every_comparison: bool,
    pub nan_counts_as_equal_not_as_poison: bool,
    pub infinities_compare_normally: bool,
    pub an_empty_direction_list_never_dominates: bool,
    pub minimization_is_a_pure_delegation: bool,
    pub mixed_directions_leave_vectors_incomparable: bool,
}

/// 造一个支配关系并逐对求值。 / Build the relation and evaluate every pair.
fn evaluate(request: &DominanceRequest) -> Result<(u32, Vec<DominancePairView>)> {
    let with_reverse = request.with_reverse.unwrap_or(false);
    let pairs = request.pairs.iter().map(|spec| {
        if spec.lhs.iter().chain(spec.rhs.iter()).any(|value| !value.is_finite()) {
            return Err(BridgeError::new("InvalidInput", "dominance failed validation: \
                non-finite objective values cannot cross the boundary"));
        }
        Ok(spec)
    }).collect::<Result<Vec<_>>>()?;
    let views = match request.mode.as_str() {
        "directions" => {
            let relation = ParetoDominance::new(request.directions.clone());
            let axes = u32::try_from(request.directions.len()).unwrap_or(u32::MAX);
            (axes, pairs.iter().map(|spec| DominancePairView {
                forward: relation.dominates(&spec.lhs, &spec.rhs),
                reverse: with_reverse && relation.dominates(&spec.rhs, &spec.lhs),
            }).collect::<Vec<_>>())
        }
        "minimization" => {
            let amount = usize::try_from(request.objective_amount.unwrap_or(0))
                .map_err(|_| BridgeError::new("InvalidInput",
                    "dominance failed validation: objective amount overflow"))?;
            let relation = MinimizationDominance::new(amount);
            // `MinimizationDominance` 没有访问器，维度数只能取请求声明的那一个。 
            // `MinimizationDominance` has no accessor, so the axis count is the declared amount.
            let axes = u32::try_from(amount).unwrap_or(u32::MAX);
            (axes, pairs.iter().map(|spec| DominancePairView {
                forward: relation.dominates(&spec.lhs, &spec.rhs),
                reverse: with_reverse && relation.dominates(&spec.rhs, &spec.lhs),
            }).collect::<Vec<_>>())
        }
        other => return Err(BridgeError::new("InvalidInput", &format!(
            "dominance failed validation: unknown mode `{other}`"))),
    };
    Ok(views)
}

/// 走一遍支配关系探针。非法输入是**错误**。 / Walk the dominance probe.
pub fn verify_dominance(request: &DominanceRequest) -> Result<DominanceReport> {
    if request.pairs.len() > 64 {
        return Err(BridgeError::new("InvalidInput",
            "dominance failed validation: at most 64 pairs are accepted"));
    }
    if request.directions.len() > 64 {
        return Err(BridgeError::new("InvalidInput",
            "dominance failed validation: at most 64 directions are accepted"));
    }
    let findings = canonical_findings();
    let (compared_axes, pairs) = evaluate(request)?;
    Ok(DominanceReport {
        mode: request.mode.clone(),
        compared_axes,
        pairs,
        the_relation_is_a_strict_partial_order: findings.the_relation_is_a_strict_partial_order,
        a_shorter_direction_list_truncates_the_comparison:
            findings.a_shorter_direction_list_truncates_the_comparison,
        a_longer_direction_list_refuses_every_comparison:
            findings.a_longer_direction_list_refuses_every_comparison,
        nan_counts_as_equal_not_as_poison: findings.nan_counts_as_equal_not_as_poison,
        infinities_compare_normally: findings.infinities_compare_normally,
        an_empty_direction_list_never_dominates: findings.an_empty_direction_list_never_dominates,
        minimization_is_a_pure_delegation: findings.minimization_is_a_pure_delegation,
        mixed_directions_leave_vectors_incomparable:
            findings.mixed_directions_leave_vectors_incomparable,
    })
}

/// 八条规则属性的内部固定探针。 / The eight rule properties measured by fixed internal probes.
struct Findings {
    the_relation_is_a_strict_partial_order: bool,
    a_shorter_direction_list_truncates_the_comparison: bool,
    a_longer_direction_list_refuses_every_comparison: bool,
    nan_counts_as_equal_not_as_poison: bool,
    infinities_compare_normally: bool,
    an_empty_direction_list_never_dominates: bool,
    minimization_is_a_pure_delegation: bool,
    mixed_directions_leave_vectors_incomparable: bool,
}

/// 用固定探针测出全部规则属性——不读调用方的请求值。
/// Measure every rule property with fixed probes; the caller's request values are not read.
fn canonical_findings() -> Findings {
    // 1) 严格偏序：不自反、不对称、可传递。 / A strict partial order: irreflexive, asymmetric, transitive.
    let the_relation_is_a_strict_partial_order = {
        let relation = ParetoDominance::minimization(3);
        let vectors: Vec<[f64; 3]> = vec![
            [0.0, 0.0, 0.0], [1.0, 1.0, 1.0], [2.0, 2.0, 2.0], [1.0, 0.0, 2.0],
            [0.0, 1.0, 1.0], [1.0, 1.0, 0.0], [0.5, 0.5, 0.5], [3.0, 0.0, 0.0]];
        let slice = |value: &[f64; 3]| value.to_vec();
        // 不自反。 / Irreflexive.
        let irreflexive = vectors.iter().all(|value| !relation.dominates(&slice(value), &slice(value)));
        // 不对称 + 可传递。 / Asymmetric and transitive.
        let asymmetric = vectors.iter().all(|left| vectors.iter().all(|right| {
            !(relation.dominates(&slice(left), &slice(right))
                && relation.dominates(&slice(right), &slice(left)))
        }));
        let transitive = vectors.iter().all(|a| vectors.iter().all(|b| vectors.iter().all(|c| {
            !(relation.dominates(&slice(a), &slice(b))
                && relation.dominates(&slice(b), &slice(c)))
                || relation.dominates(&slice(a), &slice(c))
        })));
        // 至少有一条成立，免得空关系让上面三条平凡为真。 
        // At least one pair holds, so the three checks above cannot pass trivially.
        irreflexive && asymmetric && transitive
            && relation.dominates(&[0.0, 0.0, 0.0], &[1.0, 1.0, 1.0])
    };
    // 2) 方向表短了就截断。 / A short direction list truncates.
    let a_shorter_direction_list_truncates_the_comparison = {
        let short = ParetoDominance::minimization(1);
        let full = ParetoDominance::minimization(2);
        // 第一维更好、第二维更差：一维判据接受，二维判据拒绝。 
        // Better on axis 0 and worse on axis 1: a one-axis predicate accepts while a two-axis one refuses.
        short.dominates(&[1.0, 10.0], &[2.0, 0.0])
            && !full.dominates(&[1.0, 10.0], &[2.0, 0.0])
    };
    // 3) 方向表长了就一律拒绝。 / A long direction list refuses everything.
    let a_longer_direction_list_refuses_every_comparison = {
        let long = ParetoDominance::minimization(3);
        // 第三维取不到 → 直接 `false`，哪怕前两维都更好。 
        // The third axis is missing, which returns `false` even though the first two are better.
        !long.dominates(&[0.0, 0.0], &[1.0, 1.0])
            // 补齐第三维才恢复支配。 / Supplying the third axis restores domination.
            && long.dominates(&[0.0, 0.0, 0.0], &[1.0, 1.0, 1.0])
    };
    // 4) `NaN` 被当作相等。 / `NaN` counts as equal.
    let nan_counts_as_equal_not_as_poison = {
        let relation = ParetoDominance::minimization(2);
        // 另一维更好 → 带 `NaN` 的向量照样支配。 / Another axis is better, so the `NaN` vector still dominates.
        relation.dominates(&[1.0, f64::NAN], &[2.0, f64::NAN])
            // 只差 `NaN` 这一维 → 不支配（也和"相等即不支配"一致）。 
            // Differing only on the `NaN` axis → no domination, consistent with "equal never dominates".
            && !relation.dominates(&[1.0, f64::NAN], &[1.0, 0.0])
            // 反向也不支配。 / The reverse does not dominate either.
            && !relation.dominates(&[2.0, f64::NAN], &[1.0, f64::NAN])
            // `NaN` 不会因为"更差"而拒绝：另一维更好时仍成立。 
            // `NaN` never refuses by counting as worse: it still holds when another axis is better.
            && relation.dominates(&[0.0, f64::NAN], &[1.0, 0.0])
    };
    // 5) 无穷按普通数值比较。 / Infinities compare normally.
    let infinities_compare_normally = {
        let relation = ParetoDominance::minimization(2);
        relation.dominates(&[f64::NEG_INFINITY, 0.0], &[f64::INFINITY, 0.0])
            && relation.dominates(&[0.0, f64::NEG_INFINITY], &[0.0, 5.0])
            && !relation.dominates(&[f64::INFINITY, 0.0], &[f64::NEG_INFINITY, 0.0])
            // 与有限值同序。 / Ordered with finite values.
            && relation.dominates(&[f64::NEG_INFINITY, 0.0], &[1.0, 0.0])
    };
    // 6) 空方向表永不支配。 / An empty direction list never dominates.
    let an_empty_direction_list_never_dominates = {
        let relation = ParetoDominance::minimization(0);
        !relation.dominates(&[], &[])
            && !relation.dominates(&[1.0], &[2.0])
            && !relation.dominates(&[1.0], &[1.0])
    };
    // 7) `MinimizationDominance` 是纯委托。 / `MinimizationDominance` is a pure delegation.
    let minimization_is_a_pure_delegation = {
        let delegation = MinimizationDominance::new(2);
        let equivalent = ParetoDominance::minimization(2);
        let samples = [([1.0, 2.0], [2.0, 3.0]), ([1.0, 2.0], [1.0, 2.0]),
            ([1.0, 4.0], [2.0, 3.0]), ([2.0, 3.0], [1.0, 2.0]), ([0.0, 0.0], [0.0, 1.0])];
        // 逐项相同（`ParetoDominance` 读方向，`MinimizationDominance` 内部看不见）。 
        // Identical entry by entry (`ParetoDominance` reads directions while the delegation keeps them private).
        samples.iter().all(|(left, right): &([f64; 2], [f64; 2])|
            delegation.dominates(left, right) == equivalent.dominates(left, right))
            // 与 `new(vec![true; n])` 等价。 / Equivalent to `new(vec![true; n])`.
            && samples.iter().all(|(left, right): &([f64; 2], [f64; 2])|
                delegation.dominates(left, right)
                    == ParetoDominance::new(vec![true, true]).dominates(left, right))
    };
    // 8) 混合方向让两边互不可比。 / Mixed directions leave both sides incomparable.
    let mixed_directions_leave_vectors_incomparable = {
        // 第一维最小化、第二维最大化。 / Minimise axis 0 and maximise axis 1.
        let relation = ParetoDominance::new(vec![true, false]);
        // `lhs` 第一维好、第二维差；`rhs` 反之 → 双向都不支配。 
        // `lhs` wins axis 0 and loses axis 1 while `rhs` does the opposite, so neither dominates.
        !relation.dominates(&[1.0, 1.0], &[2.0, 2.0])
            && !relation.dominates(&[2.0, 2.0], &[1.0, 1.0])
            // 两个方向同时更好才支配。 / Domination needs both directions to win.
            && relation.dominates(&[1.0, 2.0], &[2.0, 1.0])
    };
    Findings {
        the_relation_is_a_strict_partial_order,
        a_shorter_direction_list_truncates_the_comparison,
        a_longer_direction_list_refuses_every_comparison,
        nan_counts_as_equal_not_as_poison,
        infinities_compare_normally,
        an_empty_direction_list_never_dominates,
        minimization_is_a_pure_delegation,
        mixed_directions_leave_vectors_incomparable,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pair(lhs: &[f64], rhs: &[f64]) -> DominancePairSpec {
        DominancePairSpec { lhs: lhs.to_vec(), rhs: rhs.to_vec() }
    }

    fn request() -> DominanceRequest {
        DominanceRequest { mode: "directions".to_owned(), directions: vec![true], 
            objective_amount: None, pairs: Vec::new(), with_reverse: None }
    }

    #[test]
    fn an_empty_pair_list_still_reports_the_eight_rules() {
        let report = verify_dominance(&request()).unwrap();
        assert!(report.pairs.is_empty());
        assert_eq!(report.compared_axes, 1);
        assert_eq!(report.mode, "directions");
        assert!(report.the_relation_is_a_strict_partial_order);
        assert!(report.a_shorter_direction_list_truncates_the_comparison);
        assert!(report.a_longer_direction_list_refuses_every_comparison);
        assert!(report.nan_counts_as_equal_not_as_poison);
        assert!(report.infinities_compare_normally);
        assert!(report.an_empty_direction_list_never_dominates);
        assert!(report.minimization_is_a_pure_delegation);
        assert!(report.mixed_directions_leave_vectors_incomparable);
    }

    #[test]
    fn a_short_direction_list_truncates_the_vector() {
        // 方向表只有一维：第二维"更差"被忽略。 / One direction only: the worse second axis is ignored.
        let report = verify_dominance(&DominanceRequest { directions: vec![true],
            pairs: vec![pair(&[1.0, 10.0], &[2.0, 0.0])], with_reverse: Some(true),
            ..request() }).unwrap();
        assert!(report.pairs[0].forward);
        assert!(!report.pairs[0].reverse);
        assert_eq!(report.compared_axes, 1);
        // 两维方向表就拒绝。 / Two directions refuse it.
        let full = verify_dominance(&DominanceRequest { directions: vec![true, true],
            pairs: vec![pair(&[1.0, 10.0], &[2.0, 0.0])], ..request() }).unwrap();
        assert!(!full.pairs[0].forward);
    }

    #[test]
    fn a_long_direction_list_refuses_every_comparison() {
        let report = verify_dominance(&DominanceRequest { directions: vec![true, true, true],
            pairs: vec![pair(&[0.0, 0.0], &[1.0, 1.0])], with_reverse: Some(true), ..request() })
            .unwrap();
        assert!(!report.pairs[0].forward && !report.pairs[0].reverse);
        assert_eq!(report.compared_axes, 3);
    }

    #[test]
    fn nan_neither_dominates_nor_blocks() {
        // 有限值这边无法表达 NaN，所以只能问"NaN 不算更差"的反面：二维都更好才支配。 
        // The finite side cannot express NaN, so this asks the positive case instead.
        let report = verify_dominance(&DominanceRequest { directions: vec![true, true],
            pairs: vec![pair(&[0.0, 0.0], &[1.0, 1.0])], ..request() }).unwrap();
        assert!(report.pairs[0].forward);
        assert!(report.nan_counts_as_equal_not_as_poison);
    }

    #[test]
    fn a_non_finite_objective_is_refused_at_the_boundary() {
        // JSON 表达不了 `inf`/`NaN`；从结构体直接喂进来必须被拒。 
        // JSON cannot express `inf`/`NaN`, so feeding them through the struct must be refused.
        let report = verify_dominance(&DominanceRequest { directions: vec![true],
            pairs: vec![pair(&[f64::INFINITY], &[1.0])], ..request() });
        assert!(report.unwrap_err().message.contains("non-finite objective values"));
        assert!(verify_dominance(&DominanceRequest { directions: vec![true],
            pairs: vec![pair(&[f64::NAN], &[1.0])], ..request() }).is_err());
    }

    #[test]
    fn the_minimization_mode_matches_the_directions_mode() {
        let samples = vec![pair(&[1.0, 2.0], &[2.0, 3.0]), pair(&[1.0, 2.0], &[1.0, 2.0]),
            pair(&[1.0, 4.0], &[2.0, 3.0]), pair(&[2.0, 3.0], &[1.0, 2.0])];
        let by_directions = verify_dominance(&DominanceRequest { mode: "directions".to_owned(),
            directions: vec![true, true], pairs: samples.clone(), with_reverse: Some(true),
            objective_amount: None }).unwrap();
        let by_minimization = verify_dominance(&DominanceRequest {
            mode: "minimization".to_owned(), directions: Vec::new(),
            objective_amount: Some(2), pairs: samples, with_reverse: Some(true) }).unwrap();
        assert_eq!(by_directions.pairs, by_minimization.pairs);
        assert_eq!(by_minimization.compared_axes, 2);
        assert_eq!(by_minimization.mode, "minimization");
    }

    #[test]
    fn mixed_directions_leave_two_vectors_incomparable() {
        // 最小化第一维、最大化第二维。 / Minimise axis 0 and maximise axis 1.
        let report = verify_dominance(&DominanceRequest { directions: vec![true, false],
            pairs: vec![pair(&[1.0, 1.0], &[2.0, 2.0]), pair(&[1.0, 2.0], &[2.0, 1.0])],
            with_reverse: Some(true), ..request() }).unwrap();
        assert!(!report.pairs[0].forward && !report.pairs[0].reverse);
        assert!(report.pairs[1].forward && !report.pairs[1].reverse);
    }

    #[test]
    fn an_empty_direction_list_never_dominates() {
        let report = verify_dominance(&DominanceRequest { directions: Vec::new(),
            pairs: vec![pair(&[1.0], &[2.0])], with_reverse: Some(true), ..request() }).unwrap();
        assert!(!report.pairs[0].forward && !report.pairs[0].reverse);
        assert_eq!(report.compared_axes, 0);
    }

    #[test]
    fn bad_input_is_refused() {
        assert!(verify_dominance(&DominanceRequest { mode: "nope".to_owned(), ..request() })
            .unwrap_err().message.contains("unknown mode"));
        assert!(verify_dominance(&DominanceRequest { directions: vec![true],
            pairs: (0..65).map(|_| pair(&[0.0], &[1.0])).collect(), ..request() }).unwrap_err()
            .message.contains("at most 64 pairs"));
        assert!(verify_dominance(&DominanceRequest { directions: vec![true; 65],
            ..request() }).unwrap_err().message.contains("at most 64 directions"));
    }

    /// 守门：DTO 必须是 camelCase 且拒绝未知字段。 / A guard: camelCase wire, unknown fields refused.
    #[test]
    fn the_report_wire_is_camel_case_and_rejects_unknown_fields() {
        let report = verify_dominance(&request()).unwrap();
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"comparedAxes\""), "{json}");
        assert!(json.contains("\"aShorterDirectionListTruncatesTheComparison\""), "{json}");
        assert!(json.contains("\"nanCountsAsEqualNotAsPoison\""), "{json}");
        assert!(!json.contains("compared_axes"), "{json}");
        let mut copy: serde_json::Value = serde_json::from_str(&json).unwrap();
        copy["unexpected"] = serde_json::json!(1);
        assert!(serde_json::from_str::<DominanceReport>(&copy.to_string()).is_err());
    }
}
