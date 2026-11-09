//! 续挖第 85 单：**约束评估的两种排序**——epsilon 排序在容差内让目标说了算（于是**不可行的能排到可行之前**）、
//! 收紧到零才回到"违约优先"，而随机排序用调用方种子可复现、且可行对**必**按目标比
//! （`solver/heuristic/operator/fitness/constraint_ranking.rs` 的 `ConstraintAssessment` /
//! `EpsilonConstraintEvaluator` / `StochasticRanking`）
//! Continuation, delivery 85: the **two rankings over constraint assessments** — the epsilon ranker lets the objective
//! decide inside the tolerance (so an **infeasible entry can precede a feasible one**), only tightening to zero returns to
//! "violation first", while the stochastic ranker is reproducible from a caller-owned seed and **must** compare feasible
//! pairs by objective
//! (`ConstraintAssessment` / `EpsilonConstraintEvaluator` / `StochasticRanking` in
//! `solver/heuristic/operator/fitness/constraint_ranking.rs`).
//!
//! 第 84 单量的是档案；这一单量**已计费评估的排序器**。八条实测：
//!
//! 1. **容差内目标说了算**（`within_the_tolerance_the_objective_decides()`）：两条都 ≤ epsilon 时比目标，
//!    所以**违约更大但目标更好的条目可以排到违约为零的条目之前**。本单最值钱的一条。
//! 2. **收紧到零才回到违约优先**（`tightening_to_zero_restores_a_violation_first_ranking()`）：
//!    `epsilon = initial * ((1 - step/horizon).clamp(0,1))^exponent`，`step == horizon` 时 `epsilon == 0`。
//! 3. **超过 horizon 与恰好等于 horizon 相同**（`beyond_the_horizon_behaves_like_the_horizon()`）：
//!    `clamp(0.0, 1.0)` 把负值截到 0。
//! 4. **违约相等时回落到目标比较**（`an_equal_violation_falls_back_to_the_objective()`）：即使**都**在容差外。
//! 5. **排序是稳定的**（`the_sort_is_stable_so_ties_keep_the_input_order()`）：目标与违约都相等时保持输入顺序。
//! 6. **非法评估在排序前就被拒**（`an_invalid_assessment_is_refused()`）：违约度为负、目标或违约非有限。
//! 7. **随机排序对同一 seed 可复现**（`the_stochastic_ranker_is_reproducible_for_a_fixed_seed()`）：
//!    两条同 seed 的流给出同一结果，且消耗后状态相同。
//! 8. **可行对必按目标比**（`the_stochastic_ranker_compares_feasible_pairs_by_objective()`）：
//!    `violation == 0.0` 的对**绕过**概率门，`objective_probability = 0.0` 时仍按目标排。
//!
//! Delivery 84 measured the archive; this one measures **the rankers over already-metered assessments**. **Inside the
//! tolerance the objective decides** — with both violations at or below epsilon the objective is compared, so **an entry
//! with a larger violation but a better objective can precede a feasible entry**; this is the most valuable finding here.
//! **Only tightening to zero returns to a violation-first ranking** because `step == horizon` makes `epsilon == 0`.
//! **Beyond the horizon behaves like the horizon** thanks to `clamp(0.0, 1.0)`. **An equal violation falls back to the
//! objective** even when both sit outside the tolerance. **The sort is stable**, so full ties keep the input order. **An
//! invalid assessment is refused before ranking** (negative violation, non-finite objective or violation). **The stochastic
//! ranker is reproducible for a fixed seed**, and **feasible pairs are always compared by objective** because the
//! `violation == 0.0` bypass skips the probability gate.

use ospf_rust_core_plugin_heuristic::heuristic::entropy::Entropy;
use ospf_rust_core_plugin_heuristic::heuristic::operator::fitness::constraint_ranking::{
    ConstraintAssessment, EpsilonConstraintEvaluator, StochasticRanking,
};
use ospf_rust_core_plugin_heuristic::heuristic::session::SearchRandom;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 一条已计费的约束评估。 / One already-metered constraint assessment.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RankingAssessmentSpec {
    /// 业务值（用于在读面里认出来）。 / The domain value, used to recognise the entry in the result.
    pub value: u32,
    /// 显式最小化标量。 / The explicit minimisation scalar.
    pub objective: f64,
    /// 归一化非负违约度。 / The normalised nonnegative violation.
    pub violation: f64,
}

/// 约束评估排序探针请求。 / The constraint-ranking probe request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConstraintRankingRequest {
    /// `epsilon` 或 `stochastic`。 / Either `epsilon` or `stochastic`.
    pub mode: String,
    /// 初始违约容差（`epsilon` 模式，默认 0）。 / The initial violation tolerance (epsilon mode, default 0).
    #[serde(default)]
    pub initial_epsilon: Option<f64>,
    /// 收紧步数（`epsilon` 模式，默认 1000）。 / The tightening horizon (epsilon mode, default 1000).
    #[serde(default)]
    pub horizon: Option<u64>,
    /// 收紧指数（`epsilon` 模式，默认 2）。 / The tightening exponent (epsilon mode, default 2).
    #[serde(default)]
    pub exponent: Option<f64>,
    /// 当前步（`epsilon` 模式，默认 0）。 / The current step (epsilon mode, default 0).
    #[serde(default)]
    pub step: Option<u64>,
    /// 随机排序的目标比较概率（`stochastic` 模式，默认 0.45）。 / The objective-comparison probability.
    #[serde(default)]
    pub objective_probability: Option<f64>,
    /// 随机流种子（`stochastic` 模式，默认 0）。 / The random-stream seed (stochastic mode, default 0).
    #[serde(default)]
    pub seed: Option<u64>,
    /// 待排序的评估。 / The assessments to rank.
    #[serde(default)]
    pub entries: Vec<RankingAssessmentSpec>,
}

/// 排序结果里的一条（原样回显评估的三元组）。 / One ranked entry, echoing the assessment triple.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RankingEntryView {
    /// 业务值。 / The domain value.
    pub value: u32,
    /// 显式最小化标量。 / The explicit minimisation scalar.
    pub objective: f64,
    /// 归一化非负违约度。 / The normalised nonnegative violation.
    pub violation: f64,
}

/// 约束评估排序探针结论。 / The constraint-ranking probe verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConstraintRankingReport {
    /// 模式回显。 / The mode echoed back.
    pub mode: String,
    /// 排序后的条目（按名次）。 / The ranked entries, in order.
    pub ranked: Vec<RankingEntryView>,
    /// 随机流在排序后的状态（仅 `stochastic` 模式给出）。 / The random-stream state after ranking (stochastic only).
    #[serde(default)]
    pub random_state: Option<u64>,
    /// 八条规则属性的读数。 / The eight rule readings.
    pub within_the_tolerance_the_objective_decides: bool,
    pub tightening_to_zero_restores_a_violation_first_ranking: bool,
    pub beyond_the_horizon_behaves_like_the_horizon: bool,
    pub an_equal_violation_falls_back_to_the_objective: bool,
    pub the_sort_is_stable_so_ties_keep_the_input_order: bool,
    pub an_invalid_assessment_is_refused: bool,
    pub the_stochastic_ranker_is_reproducible_for_a_fixed_seed: bool,
    pub the_stochastic_ranker_compares_feasible_pairs_by_objective: bool,
}

/// 造评估表。 / Build the assessment table.
fn assessments(request: &ConstraintRankingRequest) -> Result<Vec<ConstraintAssessment<u32>>> {
    if request.entries.len() > 32 {
        return Err(BridgeError::new("InvalidInput",
            "constraint ranking failed validation: at most 32 assessments are accepted"));
    }
    if request.step.unwrap_or(0) > 1_000_000 || request.horizon.unwrap_or(1000) > 1_000_000 {
        return Err(BridgeError::new("InvalidInput",
            "constraint ranking failed validation: step and horizon must stay below one million"));
    }
    let mut entries = Vec::with_capacity(request.entries.len());
    for spec in &request.entries {
        if !spec.objective.is_finite() || !spec.violation.is_finite() {
            return Err(BridgeError::new("InvalidInput", "constraint ranking failed validation: \
                non-finite assessment values cannot cross the boundary"));
        }
        entries.push(ConstraintAssessment { value: spec.value, objective: spec.objective,
            violation: spec.violation });
    }
    Ok(entries)
}

/// 把条目转成读面。 / Turn entries into the readable face.
fn views<T: Clone>(entries: Vec<ConstraintAssessment<T>>,
    value_of: impl Fn(&T) -> u32) -> Vec<RankingEntryView> {
    entries.into_iter().map(|entry| RankingEntryView { value: value_of(&entry.value),
        objective: entry.objective, violation: entry.violation }).collect()
}

/// 一个评估表的业务值序列。 / The sequence of domain values.
fn values<T: Clone>(entries: &[ConstraintAssessment<T>], value_of: impl Fn(&T) -> u32)
    -> Vec<u32> {
    entries.iter().map(|entry| value_of(&entry.value)).collect()
}

/// 走一遍排序探针。非法输入是**错误**。 / Walk the ranking probe.
pub fn verify_constraint_ranking(request: &ConstraintRankingRequest)
    -> Result<ConstraintRankingReport>
{
    let entries = assessments(request)?;
    let findings = canonical_findings();
    let identity = |value: &u32| *value;
    let (ranked, random_state) = match request.mode.as_str() {
        "epsilon" => {
            let evaluator = EpsilonConstraintEvaluator {
                initial_epsilon: request.initial_epsilon.unwrap_or(0.0),
                horizon: request.horizon.unwrap_or(1000),
                exponent: request.exponent.unwrap_or(2.0),
            };
            let ranked = evaluator.rank(&entries, request.step.unwrap_or(0))
                .map_err(|error| BridgeError::new("InvalidInput", &format!(
                    "constraint ranking failed validation: {error}")))?;
            (views(ranked, identity), None)
        }
        "stochastic" => {
            let ranker = StochasticRanking {
                objective_probability: request.objective_probability.unwrap_or(0.45) };
            let mut random = SearchRandom::new(request.seed.unwrap_or(0));
            let ranked = ranker.rank(&entries, &mut random)
                .map_err(|error| BridgeError::new("InvalidInput", &format!(
                    "constraint ranking failed validation: {error}")))?;
            (views(ranked, identity), Some(random.state()))
        }
        other => return Err(BridgeError::new("InvalidInput", &format!(
            "constraint ranking failed validation: unknown mode `{other}`"))),
    };
    Ok(ConstraintRankingReport {
        mode: request.mode.clone(),
        ranked,
        random_state,
        within_the_tolerance_the_objective_decides:
            findings.within_the_tolerance_the_objective_decides,
        tightening_to_zero_restores_a_violation_first_ranking:
            findings.tightening_to_zero_restores_a_violation_first_ranking,
        beyond_the_horizon_behaves_like_the_horizon:
            findings.beyond_the_horizon_behaves_like_the_horizon,
        an_equal_violation_falls_back_to_the_objective:
            findings.an_equal_violation_falls_back_to_the_objective,
        the_sort_is_stable_so_ties_keep_the_input_order:
            findings.the_sort_is_stable_so_ties_keep_the_input_order,
        an_invalid_assessment_is_refused: findings.an_invalid_assessment_is_refused,
        the_stochastic_ranker_is_reproducible_for_a_fixed_seed:
            findings.the_stochastic_ranker_is_reproducible_for_a_fixed_seed,
        the_stochastic_ranker_compares_feasible_pairs_by_objective:
            findings.the_stochastic_ranker_compares_feasible_pairs_by_objective,
    })
}

/// 八条规则属性的内部固定探针。 / The eight rule properties measured by fixed internal probes.
struct Findings {
    within_the_tolerance_the_objective_decides: bool,
    tightening_to_zero_restores_a_violation_first_ranking: bool,
    beyond_the_horizon_behaves_like_the_horizon: bool,
    an_equal_violation_falls_back_to_the_objective: bool,
    the_sort_is_stable_so_ties_keep_the_input_order: bool,
    an_invalid_assessment_is_refused: bool,
    the_stochastic_ranker_is_reproducible_for_a_fixed_seed: bool,
    the_stochastic_ranker_compares_feasible_pairs_by_objective: bool,
}

/// 造一条评估。 / Build one assessment.
fn entry(value: u32, objective: f64, violation: f64) -> ConstraintAssessment<u32> {
    ConstraintAssessment { value, objective, violation }
}

/// 用固定探针测出全部规则属性——不读调用方的请求值。
/// Measure every rule property with fixed probes; the caller's request values are not read.
fn canonical_findings() -> Findings {
    let identity = |value: &u32| *value;
    let evaluator = EpsilonConstraintEvaluator { initial_epsilon: 2.0, horizon: 2, exponent: 2.0 };
    // 1) 容差内目标说了算：违约 2.0 但目标 1.0 的条目排在违约 0 但目标 3.0 的条目之前。 
    // Inside the tolerance the objective decides: a violation-2.0 entry with objective 1.0 precedes a feasible entry with
    // objective 3.0.
    let within_the_tolerance_the_objective_decides = {
        let entries = vec![entry(0, 3.0, 0.0), entry(1, 1.0, 2.0)];
        // `step = 0` → `epsilon = 2.0`，两条都在容差内 → 比目标。 
        // At step 0 the epsilon is 2.0, both sit inside it and the objective decides.
        values(&evaluator.rank(&entries, 0).expect("rank"), identity) == vec![1, 0]
    };
    // 2) 收紧到零才回到违约优先。 / Only tightening to zero returns to violation first.
    let tightening_to_zero_restores_a_violation_first_ranking = {
        let entries = vec![entry(0, 3.0, 0.0), entry(1, 1.0, 2.0)];
        // `step = horizon = 2` → `epsilon == 0`，只有违约 0 的那条算"可行"，于是违约优先。 
        // With step == horizon == 2 the epsilon is zero, only the zero-violation entry counts as feasible, and the
        // violation decides.
        values(&evaluator.rank(&entries, 2).expect("rank"), identity) == vec![0, 1]
    };
    // 3) 超过 horizon 与恰好等于 horizon 相同。 / Beyond the horizon behaves like the horizon.
    let beyond_the_horizon_behaves_like_the_horizon = {
        let entries = vec![entry(0, 3.0, 0.0), entry(1, 1.0, 2.0), entry(2, 2.0, 1.0)];
        let at = values(&evaluator.rank(&entries, 2).expect("rank"), identity);
        let beyond = values(&evaluator.rank(&entries, 5).expect("rank"), identity);
        let far = values(&evaluator.rank(&entries, 99).expect("rank"), identity);
        at == beyond && at == far
    };
    // 4) 违约相等时回落到目标比较，即使都在容差外。 
    // An equal violation falls back to the objective even outside the tolerance.
    let an_equal_violation_falls_back_to_the_objective = {
        // `epsilon == 0`：两条违约都是 1.5（都 > 容差），但数值相等 → 比目标。 
        // With epsilon 0 both violations are 1.5, outside the tolerance, yet equal, so the objective decides.
        let entries = vec![entry(0, 5.0, 1.5), entry(1, 2.0, 1.5)];
        values(&evaluator.rank(&entries, 2).expect("rank"), identity) == vec![1, 0]
    };
    // 5) 排序是稳定的。 / The sort is stable.
    let the_sort_is_stable_so_ties_keep_the_input_order = {
        // 目标与违约都相等 → 输入顺序不变。 / Full ties keep the input order.
        let entries = vec![entry(7, 1.0, 1.0), entry(8, 1.0, 1.0), entry(9, 1.0, 1.0)];
        values(&evaluator.rank(&entries, 0).expect("rank"), identity) == vec![7, 8, 9]
            // 也验证"不相等的部分确实排了"。 / It also shows the non-tied part really does sort.
            && values(&evaluator.rank(&vec![entry(1, 3.0, 0.0), entry(2, 2.0, 0.0), entry(3, 1.0, 0.0)],
                0).expect("rank"), identity) == vec![3, 2, 1]
    };
    // 6) 非法评估在排序前就被拒。 / An invalid assessment is refused before ranking.
    let an_invalid_assessment_is_refused = {
        // 违约度为负、目标非有限、违约非有限——三种都拒。 
        // A negative violation, a non-finite objective and a non-finite violation are all refused.
        let negative = evaluator.rank(&vec![entry(0, 1.0, -1.0)], 0).is_err();
        let infinite = evaluator.rank(&vec![entry(0, f64::INFINITY, 0.0)], 0).is_err();
        let nan = evaluator.rank(&vec![entry(0, f64::NAN, 0.0)], 0).is_err();
        let nan_violation = evaluator.rank(&vec![entry(0, 1.0, f64::NAN)], 0).is_err();
        negative && infinite && nan && nan_violation
            // 对照：合法输入照常排序。 / Control: a legal input still ranks.
            && evaluator.rank(&vec![entry(0, 1.0, 0.0)], 0).is_ok()
    };
    // 7) 随机排序对同一 seed 可复现。 / The stochastic ranker is reproducible for a fixed seed.
    let the_stochastic_ranker_is_reproducible_for_a_fixed_seed = {
        let ranker = StochasticRanking { objective_probability: 0.45 };
        let entries = vec![entry(0, 1.0, 2.0), entry(1, 3.0, 0.0), entry(2, 2.0, 1.0)];
        let mut first = SearchRandom::new(42);
        let mut second = SearchRandom::new(42);
        let left = ranker.rank(&entries, &mut first).expect("rank");
        let right = ranker.rank(&entries, &mut second).expect("rank");
        // 同一 seed 两趟完全相同，且消耗后的流状态相同。 
        // Two runs from one seed agree exactly and consume the stream to the same state.
        left == right && first.state() == second.state()
            // 不同 seed 会消耗到不同状态（流确实被用到了）。 
            // A different seed lands on a different state, which shows the stream really is consumed.
            && { let mut other = SearchRandom::new(43);
                let _ = ranker.rank(&entries, &mut other).expect("rank");
                other.state() != first.state() }
    };
    // 8) 可行对必按目标比。 / Feasible pairs are always compared by objective.
    let the_stochastic_ranker_compares_feasible_pairs_by_objective = {
        // `objective_probability = 0.0`：概率门永不挑目标，但可行对**绕过**门。 
        // With a zero probability the gate never picks the objective, yet feasible pairs **bypass** it.
        let ranker = StochasticRanking { objective_probability: 0.0 };
        let entries = vec![entry(0, 3.0, 0.0), entry(1, 1.0, 0.0), entry(2, 2.0, 0.0)];
        let mut random = SearchRandom::new(7);
        values(&ranker.rank(&entries, &mut random).expect("rank"), identity) == vec![1, 2, 0]
            // 对照：概率 1.0 时全按目标比，同样得到升序。 
            // Control: with probability 1.0 every pair compares objectives and the same ascending order appears.
            && { let mut other = SearchRandom::new(7);
                let full = StochasticRanking { objective_probability: 1.0 };
                values(&full.rank(&entries, &mut other).expect("rank"), identity) == vec![1, 2, 0] }
    };
    Findings {
        within_the_tolerance_the_objective_decides,
        tightening_to_zero_restores_a_violation_first_ranking,
        beyond_the_horizon_behaves_like_the_horizon,
        an_equal_violation_falls_back_to_the_objective,
        the_sort_is_stable_so_ties_keep_the_input_order,
        an_invalid_assessment_is_refused,
        the_stochastic_ranker_is_reproducible_for_a_fixed_seed,
        the_stochastic_ranker_compares_feasible_pairs_by_objective,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> ConstraintRankingRequest {
        ConstraintRankingRequest { mode: "epsilon".to_owned(), initial_epsilon: Some(0.0),
            horizon: Some(1000), exponent: Some(2.0), step: Some(0), objective_probability: None,
            seed: None, entries: Vec::new() }
    }

    fn spec(value: u32, objective: f64, violation: f64) -> RankingAssessmentSpec {
        RankingAssessmentSpec { value, objective, violation }
    }

    #[test]
    fn an_empty_ranking_keeps_the_eight_rules_readable() {
        let report = verify_constraint_ranking(&request()).unwrap();
        assert!(report.ranked.is_empty());
        assert_eq!(report.mode, "epsilon");
        assert_eq!(report.random_state, None);
        assert!(report.within_the_tolerance_the_objective_decides);
        assert!(report.tightening_to_zero_restores_a_violation_first_ranking);
        assert!(report.beyond_the_horizon_behaves_like_the_horizon);
        assert!(report.an_equal_violation_falls_back_to_the_objective);
        assert!(report.the_sort_is_stable_so_ties_keep_the_input_order);
        assert!(report.an_invalid_assessment_is_refused);
        assert!(report.the_stochastic_ranker_is_reproducible_for_a_fixed_seed);
        assert!(report.the_stochastic_ranker_compares_feasible_pairs_by_objective);
    }

    #[test]
    fn the_tolerance_lets_the_objective_win() {
        // 容差 2、`step = 0`：违约 2.0 目标 1.0 的条目排到违约 0 目标 3.0 的条目之前。 
        // With a tolerance of 2 at step 0 the violation-2.0 entry with objective 1.0 precedes the feasible entry with
        // objective 3.0.
        let mut probe = request();
        probe.initial_epsilon = Some(2.0);
        probe.horizon = Some(2);
        probe.entries = vec![spec(0, 3.0, 0.0), spec(1, 1.0, 2.0)];
        let report = verify_constraint_ranking(&probe).unwrap();
        assert_eq!(report.ranked.iter().map(|entry| entry.value).collect::<Vec<_>>(), vec![1, 0]);
        // `step = horizon`：容差归零，违约优先。 / At step == horizon the tolerance is zero and the violation decides.
        probe.step = Some(2);
        let tightened = verify_constraint_ranking(&probe).unwrap();
        assert_eq!(tightened.ranked.iter().map(|entry| entry.value).collect::<Vec<_>>(),
            vec![0, 1]);
        // 超过 horizon 与等于 horizon 相同。 / Beyond the horizon is the same as at the horizon.
        probe.step = Some(5);
        assert_eq!(verify_constraint_ranking(&probe).unwrap(), tightened);
    }

    #[test]
    fn an_equal_violation_falls_back_to_the_objective() {
        let mut probe = request();
        probe.initial_epsilon = Some(0.0);
        probe.entries = vec![spec(0, 5.0, 1.5), spec(1, 2.0, 1.5)];
        let report = verify_constraint_ranking(&probe).unwrap();
        assert_eq!(report.ranked.iter().map(|entry| entry.value).collect::<Vec<_>>(), vec![1, 0]);
    }

    #[test]
    fn full_ties_keep_the_input_order() {
        let mut probe = request();
        probe.entries = vec![spec(7, 1.0, 1.0), spec(8, 1.0, 1.0), spec(9, 1.0, 1.0)];
        let report = verify_constraint_ranking(&probe).unwrap();
        assert_eq!(report.ranked.iter().map(|entry| entry.value).collect::<Vec<_>>(),
            vec![7, 8, 9]);
    }

    #[test]
    fn the_stochastic_mode_is_reproducible_and_reports_its_state() {
        let mut probe = request();
        probe.mode = "stochastic".to_owned();
        probe.objective_probability = Some(0.45);
        probe.seed = Some(42);
        probe.entries = vec![spec(0, 1.0, 2.0), spec(1, 3.0, 0.0), spec(2, 2.0, 1.0)];
        let first = verify_constraint_ranking(&probe).unwrap();
        let second = verify_constraint_ranking(&probe).unwrap();
        assert_eq!(first, second);
        assert!(first.random_state.is_some());
        // 换 seed 会换流状态。 / Another seed lands on another stream state.
        probe.seed = Some(43);
        let other = verify_constraint_ranking(&probe).unwrap();
        assert_ne!(other.random_state, first.random_state);
    }

    #[test]
    fn a_zero_probability_still_sorts_feasible_pairs_by_objective() {
        let mut probe = request();
        probe.mode = "stochastic".to_owned();
        probe.objective_probability = Some(0.0);
        probe.seed = Some(7);
        probe.entries = vec![spec(0, 3.0, 0.0), spec(1, 1.0, 0.0), spec(2, 2.0, 0.0)];
        let report = verify_constraint_ranking(&probe).unwrap();
        assert_eq!(report.ranked.iter().map(|entry| entry.value).collect::<Vec<_>>(),
            vec![1, 2, 0]);
    }

    #[test]
    fn bad_input_is_refused() {
        let mut probe = request();
        probe.entries = vec![spec(0, 1.0, -1.0)];
        assert!(verify_constraint_ranking(&probe).unwrap_err().message
            .contains("constraint ranking failed validation"));
        // 指数 <= 0 被配置闸门拒。 / A non-positive exponent is refused by the configuration gate.
        let mut probe = request();
        probe.exponent = Some(0.0);
        assert!(verify_constraint_ranking(&probe).is_err());
        // 概率越界同样被拒。 / An out-of-range probability is refused too.
        let mut probe = request();
        probe.mode = "stochastic".to_owned();
        probe.objective_probability = Some(-1.0);
        assert!(verify_constraint_ranking(&probe).is_err());
        // 边界自身的前置校验。 / The boundary's own pre-checks.
        assert!(verify_constraint_ranking(&ConstraintRankingRequest { mode: "nope".to_owned(),
            ..request() }).unwrap_err().message.contains("unknown mode"));
        assert!(verify_constraint_ranking(&ConstraintRankingRequest {
            entries: (0..33).map(|index| spec(index, 1.0, 0.0)).collect(), ..request() })
            .unwrap_err().message.contains("at most 32 assessments"));
        assert!(verify_constraint_ranking(&ConstraintRankingRequest { step: Some(2_000_000),
            ..request() }).unwrap_err().message.contains("below one million"));
        assert!(verify_constraint_ranking(&ConstraintRankingRequest {
            entries: vec![spec(0, f64::INFINITY, 0.0)], ..request() }).unwrap_err()
            .message.contains("non-finite assessment values"));
        assert!(verify_constraint_ranking(&ConstraintRankingRequest {
            entries: vec![spec(0, 1.0, f64::NAN)], ..request() }).unwrap_err()
            .message.contains("non-finite assessment values"));
    }

    /// 守门：DTO 必须是 camelCase 且拒绝未知字段。 / A guard: camelCase wire, unknown fields refused.
    #[test]
    fn the_report_wire_is_camel_case_and_rejects_unknown_fields() {
        let report = verify_constraint_ranking(&request()).unwrap();
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"randomState\""), "{json}");
        assert!(json.contains("\"withinTheToleranceTheObjectiveDecides\""), "{json}");
        assert!(json.contains("\"theStochasticRankerIsReproducibleForAFixedSeed\""), "{json}");
        assert!(!json.contains("random_state"), "{json}");
        let mut copy: serde_json::Value = serde_json::from_str(&json).unwrap();
        copy["unexpected"] = serde_json::json!(1);
        assert!(serde_json::from_str::<ConstraintRankingReport>(&copy.to_string()).is_err());
    }
}
