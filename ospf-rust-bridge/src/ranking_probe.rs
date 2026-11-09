//! 续挖第 90 单：**排序算子根本不排序**——`ObjectiveRanking` 只是把 `ObjectiveModel::rank_indices` 的结果
//! 逐下标克隆回来，所以"升序"其实是**模型说了算**；而 `ReversedObjectiveRanking` 是"先升序再整体 `reverse()`"，
//! 于是**并列块在反向里被倒过来**（这与"反向比较器排序"是两件事）
//! （`solver/heuristic/operator/ranking.rs` 的三个排序算子 + `operator/mod.rs` 的 `rank_individuals`）
//! Continuation, delivery 90: **the ranking operators do not sort at all** — `ObjectiveRanking` merely clones back
//! whatever `ObjectiveModel::rank_indices` names, so "ascending" is **the model's call**; and
//! `ReversedObjectiveRanking` is "ascending then a whole-list `reverse()`", which **flips the tie blocks** (a different
//! thing from sorting with a reversed comparator)
//! (the three ranking operators in `solver/heuristic/operator/ranking.rs` plus `rank_individuals` in `operator/mod.rs`).
//!
//! 第 89 单量的是接受判据，这一单量**排序**。八条实测：
//!
//! 1. **`ObjectiveRanking` 只委托 `rank_indices`**（`objective_ranking_delegates_to_the_model()`）：
//!    模型把下标反过来，它就输出降序——它自己一个比较都不做。本单最值钱的一条。
//! 2. **`ReversedObjectiveRanking` 倒的是并列块**（`reversed_ranking_flips_the_tie_blocks()`）：
//!    先升序再 `reverse()`，所以并列的三条按**输入顺序的逆序**出现。
//! 3. **`rank_indices` 的默认实现是稳定排序**（`the_default_rank_indices_is_stable()`）：并列保持输入顺序。
//! 4. **重复下标会被克隆两次**（`a_duplicate_index_is_cloned_twice()`）：输出长度**不必等于输入长度**。
//! 5. **越界下标直接 panic**（`an_out_of_range_index_panics()`）：`entries[index]` 没有边界守卫。
//! 6. **`IdentityRanking` 一次都不碰模型**（`identity_ranking_never_touches_the_model()`）：
//!    连"越界模型"都不影响它。
//! 7. **排序返回克隆、输入原封不动**（`ranking_returns_clones_and_leaves_the_input_untouched()`）。
//! 8. **空与单元素是不动点**（`empty_and_single_inputs_are_fixed_points()`）。
//!
//! Delivery 89 measured acceptance; this one measures **ranking**. **`ObjectiveRanking` only delegates to
//! `rank_indices`** — when the model reverses the indices the operator emits a descending list without comparing
//! anything itself, the most valuable finding here. **`ReversedObjectiveRanking` flips the tie blocks** because it ranks
//! ascending and then reverses the whole list. **The default `rank_indices` is a stable sort**, so ties keep the input
//! order. **A duplicate index is cloned twice**, so the output length need not match the input length. **An out-of-range
//! index panics** since `entries[index]` has no bounds guard. **`IdentityRanking` never touches the model**, not even an
//! out-of-range one. **Ranking returns clones and leaves the input untouched.** And **empty and single inputs are fixed
//! points**.

use ospf_rust_core::model::callback::Solution;
use ospf_rust_core::solver::heuristic::individual::Individual;
use ospf_rust_core::solver::heuristic::model::{ObjectiveModel, VariableModel};
use ospf_rust_core::solver::heuristic::operator::ranking::{
    IdentityRanking, ObjectiveRanking, Ranking, ReversedObjectiveRanking,
};
use ospf_rust_core::variable::VariableId;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use std::cmp::Ordering;

use crate::{BridgeError, Result};

/// 一条排好序的个体（`index` 是请求里的原始位置）。 / One ranked individual; `index` is its position in the request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RankingItemRow {
    /// 请求里的原始下标。 / The original index in the request.
    pub index: u32,
    /// 该个体的适应度。 / That individual's fitness.
    pub fitness: f64,
}

/// 排序算子探针请求。 / The ranking-operator probe request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RankingProbeRequest {
    /// 依次放入的适应度。 / The fitness values put in, in order.
    #[serde(default)]
    pub fitnesses: Vec<f64>,
    /// 探针模型的 `rank_indices` 行为：`default` / `reverse` / `duplicate` / `drop` / `outOfRange`。
    /// The probe model's `rank_indices` behaviour: `default`, `reverse`, `duplicate`, `drop` or `outOfRange`.
    #[serde(default)]
    pub rank_mode: Option<String>,
}

/// 排序算子探针结论。 / The ranking-operator probe verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RankingProbeReport {
    /// 输入回显（按原序）。 / The input echoed in its original order.
    pub input: Vec<RankingItemRow>,
    /// `ObjectiveRanking` 的输出。 / The output of `ObjectiveRanking`.
    pub objective: Vec<RankingItemRow>,
    /// `ReversedObjectiveRanking` 的输出。 / The output of `ReversedObjectiveRanking`.
    pub reversed: Vec<RankingItemRow>,
    /// `IdentityRanking` 的输出。 / The output of `IdentityRanking`.
    pub identity: Vec<RankingItemRow>,
    /// `ObjectiveRanking` 是否 panic 了。 / Whether `ObjectiveRanking` panicked.
    pub objective_panicked: bool,
    /// 八条规则属性的读数。 / The eight rule readings.
    pub objective_ranking_delegates_to_the_model: bool,
    pub reversed_ranking_flips_the_tie_blocks: bool,
    pub the_default_rank_indices_is_stable: bool,
    pub a_duplicate_index_is_cloned_twice: bool,
    pub an_out_of_range_index_panics: bool,
    pub identity_ranking_never_touches_the_model: bool,
    pub ranking_returns_clones_and_leaves_the_input_untouched: bool,
    pub empty_and_single_inputs_are_fixed_points: bool,
}

/// 探针模型：`rank_indices` 行为可调，其余能力一律空实现。
/// 目标值用 `(index, fitness)` 这个 **对**，于是读面能逐条认出原始位置。
/// The probe model turns `rank_indices` into a knob and leaves every other capability empty. The objective is the
/// `(index, fitness)` pair so the read face can name every original position.
struct ProbeModel {
    /// `rank_indices` 的行为。 / The `rank_indices` behaviour.
    mode: &'static str,
}

impl VariableModel<f64> for ProbeModel {
    fn num_variables(&self) -> usize { 0 }
    fn variable_order(&self) -> Vec<VariableId> { Vec::new() }
    fn lower_bound(&self, _index: usize) -> Option<f64> { None }
    fn upper_bound(&self, _index: usize) -> Option<f64> { None }
    fn coerce_in(&self, _index: usize, value: f64) -> f64 { value }
    fn initial_solutions(&self, _amount: usize) -> Vec<Solution<f64>> { Vec::new() }
    fn constraint_satisfied(&self, _solution: &Solution<f64>) -> Option<bool> { Some(true) }
}

impl ObjectiveModel<(u32, f64), f64> for ProbeModel {
    fn default_objective(&self) -> Option<(u32, f64)> { None }
    fn objective(&self, _solution: &Solution<f64>) -> Option<(u32, f64)> { None }

    fn compare_objective(&self, lhs: &(u32, f64), rhs: &(u32, f64)) -> Option<Ordering> {
        lhs.1.partial_cmp(&rhs.1)
    }

    fn rank_indices(&self, entries: &[Individual<(u32, f64), f64>]) -> Vec<usize> {
        match self.mode {
            // 反过来：排序算子会照抄成降序。 / Reversed: the ranking operator copies it into a descending list.
            "reverse" => (0..entries.len()).rev().collect(),
            // 重复第一个下标。 / Repeat the first index.
            "duplicate" => if entries.is_empty() { Vec::new() } else { vec![0, 0] },
            // 丢掉所有下标。 / Drop every index.
            "drop" => Vec::new(),
            // 越界下标。 / An out-of-range index.
            "out_of_range" => vec![entries.len() + 3],
            // 默认实现：稳定排序。 / The default: a stable sort.
            _ => {
                let mut indices: Vec<usize> = (0..entries.len()).collect();
                indices.sort_by(|&a, &b| self.compare_individuals(&entries[a], &entries[b]));
                indices
            }
        }
    }
}

/// 造一批个体；目标值带上原始下标。 / Build the individuals, tagging each objective with its original index.
fn individuals(fitnesses: &[f64]) -> Vec<Individual<(u32, f64), f64>> {
    fitnesses.iter().enumerate().map(|(index, fitness)| Individual::new(Solution::new(),
        (u32::try_from(index).unwrap_or(u32::MAX), *fitness))).collect()
}

/// 把排好的列表转成读面。 / Turn a ranked list into the readable face.
fn rows(entries: &[Individual<(u32, f64), f64>]) -> Vec<RankingItemRow> {
    entries.iter().map(|entry| RankingItemRow { index: entry.fitness.0,
        fitness: entry.fitness.1 }).collect()
}

/// 跑三路排序（`ObjectiveRanking` 的 panic 被兜住）。 / Run all three, catching the `ObjectiveRanking` panic.
fn rank_all(fitnesses: &[f64], mode: &'static str)
    -> (Vec<RankingItemRow>, Vec<RankingItemRow>, Vec<RankingItemRow>, bool)
{
    let model = ProbeModel { mode };
    let entries = individuals(fitnesses);
    let objective = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        ObjectiveRanking.rank(&model, &entries)
    })).ok();
    let reversed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        ReversedObjectiveRanking.rank(&model, &entries)
    })).ok().map(|ranked| rows(&ranked)).unwrap_or_default();
    let identity = rows(&IdentityRanking.rank(&model, &entries));
    let panicked = objective.is_none();
    (objective.map(|ranked| rows(&ranked)).unwrap_or_default(), reversed, identity, panicked)
}

/// 走一遍排序探针。非法输入是**错误**。 / Walk the ranking probe.
pub fn verify_ranking(request: &RankingProbeRequest) -> Result<RankingProbeReport> {
    if request.fitnesses.len() > 32 {
        return Err(BridgeError::new("InvalidInput",
            "ranking probe failed validation: at most 32 individuals are accepted"));
    }
    if request.fitnesses.iter().any(|fitness| !fitness.is_finite()) {
        return Err(BridgeError::new("InvalidInput", "ranking probe failed validation: \
            non-finite fitness cannot cross the boundary"));
    }
    let mode: &'static str = match request.rank_mode.as_deref().unwrap_or("default") {
        "default" => "default",
        "reverse" => "reverse",
        "duplicate" => "duplicate",
        "drop" => "drop",
        "out_of_range" | "outOfRange" => "out_of_range",
        other => return Err(BridgeError::new("InvalidInput", &format!(
            "ranking probe failed validation: unknown rank mode `{other}`"))),
    };
    let findings = canonical_findings();
    let (objective, reversed, identity, objective_panicked) = rank_all(&request.fitnesses, mode);
    Ok(RankingProbeReport {
        input: individuals(&request.fitnesses).iter().map(|entry| RankingItemRow {
            index: entry.fitness.0, fitness: entry.fitness.1 }).collect(),
        objective,
        reversed,
        identity,
        objective_panicked,
        objective_ranking_delegates_to_the_model:
            findings.objective_ranking_delegates_to_the_model,
        reversed_ranking_flips_the_tie_blocks: findings.reversed_ranking_flips_the_tie_blocks,
        the_default_rank_indices_is_stable: findings.the_default_rank_indices_is_stable,
        a_duplicate_index_is_cloned_twice: findings.a_duplicate_index_is_cloned_twice,
        an_out_of_range_index_panics: findings.an_out_of_range_index_panics,
        identity_ranking_never_touches_the_model:
            findings.identity_ranking_never_touches_the_model,
        ranking_returns_clones_and_leaves_the_input_untouched:
            findings.ranking_returns_clones_and_leaves_the_input_untouched,
        empty_and_single_inputs_are_fixed_points: findings.empty_and_single_inputs_are_fixed_points,
    })
}

/// 固定探针只跑一次（`entries[index]` 的 panic 会打到 stderr）。 / The fixed probes run once only.
static FINDINGS: std::sync::OnceLock<Findings> = std::sync::OnceLock::new();

/// 八条规则属性的内部固定探针。 / The eight rule properties measured by fixed internal probes.
struct Findings {
    objective_ranking_delegates_to_the_model: bool,
    reversed_ranking_flips_the_tie_blocks: bool,
    the_default_rank_indices_is_stable: bool,
    a_duplicate_index_is_cloned_twice: bool,
    an_out_of_range_index_panics: bool,
    identity_ranking_never_touches_the_model: bool,
    ranking_returns_clones_and_leaves_the_input_untouched: bool,
    empty_and_single_inputs_are_fixed_points: bool,
}

/// 用固定探针测出全部规则属性——不读调用方的请求值。
/// Measure every rule property with fixed probes; the caller's request values are not read.
fn canonical_findings() -> &'static Findings {
    FINDINGS.get_or_init(|| {
        // 1) `ObjectiveRanking` 只委托 `rank_indices`。 / It only delegates to `rank_indices`.
        let objective_ranking_delegates_to_the_model = {
            // 模型把下标反过来 → 输出降序，而适应度是升序的 `[1, 2, 3]`。 
            // The model reverses the indices, so the output is descending even though the fitness values ascend.
            let (objective, _, _, panicked) = rank_all(&[1.0, 2.0, 3.0], "reverse");
            !panicked && objective.iter().map(|row| row.index).collect::<Vec<_>>()
                == vec![2, 1, 0]
                // 对照：默认实现给升序。 / Control: the default gives ascending.
                && { let (default, _, _, _) = rank_all(&[1.0, 2.0, 3.0], "default");
                    default.iter().map(|row| row.index).collect::<Vec<_>>() == vec![0, 1, 2] }
        };
        // 2) `ReversedObjectiveRanking` 倒的是并列块。 / It flips the tie blocks.
        let reversed_ranking_flips_the_tie_blocks = {
            // 并列的三条：升序保持 `[0, 1, 2]`，反向却是 `[2, 1, 0]` —— **顺序被倒过来**。 
            // Three tied entries: ascending keeps `[0, 1, 2]` while the reversed variant emits `[2, 1, 0]`, which is the
            // tie block flipped.
            let (objective, reversed, _, _) = rank_all(&[5.0, 5.0, 5.0], "default");
            objective.iter().map(|row| row.index).collect::<Vec<_>>() == vec![0, 1, 2]
                && reversed.iter().map(|row| row.index).collect::<Vec<_>>() == vec![2, 1, 0]
                // 与"反向比较器排序"不同：后者会保住并列块的输入顺序。 
                // This differs from sorting with a reversed comparator, which would keep the tie block in input order.
                && { let mut indices: Vec<usize> = (0..3).collect();
                    let model = ProbeModel { mode: "default" };
                    let entries = individuals(&[5.0, 5.0, 5.0]);
                    indices.sort_by(|&a, &b| {
                        model.compare_individuals(&entries[b], &entries[a])
                    });
                    indices == vec![0, 1, 2] }
        };
        // 3) 默认实现是稳定排序。 / The default is a stable sort.
        let the_default_rank_indices_is_stable = {
            // 混合并列：`[0(1), 1(9), 2(1), 3(9)]` → 升序按值、并列保序。 
            // A mix: ascending by value with ties keeping the input order.
            let (objective, _, _, _) = rank_all(&[1.0, 9.0, 1.0, 9.0], "default");
            objective.iter().map(|row| row.index).collect::<Vec<_>>() == vec![0, 2, 1, 3]
                && { let (tied, _, _, _) = rank_all(&[4.0, 4.0, 4.0, 4.0], "default");
                    tied.iter().map(|row| row.index).collect::<Vec<_>>() == vec![0, 1, 2, 3] }
        };
        // 4) 重复下标会被克隆两次。 / A duplicate index is cloned twice.
        let a_duplicate_index_is_cloned_twice = {
            let (objective, reversed, _, panicked) = rank_all(&[7.0, 8.0], "duplicate");
            // 输出两条都是同一个原始个体——**长度不必等于输入长度**。 
            // Both rows are the same original individual, so the output length need not match the input length.
            !panicked && objective.len() == 2
                && objective.iter().all(|row| row.index == 0 && row.fitness == 7.0)
                && reversed.iter().all(|row| row.index == 0)
                // 丢掉所有下标 → 空输出。 / Dropping every index yields an empty output.
                && { let (dropped, _, _, _) = rank_all(&[7.0, 8.0], "drop");
                    dropped.is_empty() }
        };
        // 5) 越界下标直接 panic。 / An out-of-range index panics.
        let an_out_of_range_index_panics = {
            let (_, _, _, panicked) = rank_all(&[1.0, 2.0], "out_of_range");
            panicked
                // 对照：空输入里的越界下标同样 panic（`[0+3]` 越界）。 
                // Control: an out-of-range index also panics on an empty input.
                && rank_all(&[], "out_of_range").3
        };
        // 6) `IdentityRanking` 一次都不碰模型。 / `IdentityRanking` never touches the model.
        let identity_ranking_never_touches_the_model = {
            // 连"越界模型"也不影响它。 / Even an out-of-range model leaves it alone.
            let (_, _, identity, panicked) = rank_all(&[3.0, 1.0, 2.0], "out_of_range");
            identity.iter().map(|row| row.index).collect::<Vec<_>>() == vec![0, 1, 2]
                && !identity.is_empty()
                // 而这一模式下其他两路已经崩了（前一路被兜住，`reversed` 为空）。 
                // In that mode the other two paths have already died, with the first caught and the second left empty.
                && panicked
        };
        // 7) 排序返回克隆、输入原封不动。 / Ranking returns clones and leaves the input untouched.
        let ranking_returns_clones_and_leaves_the_input_untouched = {
            let entries = individuals(&[2.0, 1.0]);
            let ranked = ObjectiveRanking.rank(&ProbeModel { mode: "default" }, &entries);
            // 输入仍是 `[0, 1]`，输出是 `[1, 0]`。 / The input stays `[0, 1]` while the output is `[1, 0]`.
            entries.iter().map(|entry| entry.fitness.0).collect::<Vec<_>>() == vec![0, 1]
                && ranked.iter().map(|entry| entry.fitness.0).collect::<Vec<_>>() == vec![1, 0]
                // 克隆与原个体是两份数据。 / The clone and the original are two separate values.
                && ranked.len() == entries.len()
        };
        // 8) 空与单元素是不动点。 / Empty and single inputs are fixed points.
        let empty_and_single_inputs_are_fixed_points = {
            let (objective, reversed, identity, panicked) = rank_all(&[], "default");
            objective.is_empty() && reversed.is_empty() && identity.is_empty() && !panicked
                && { let (objective, reversed, identity, _) = rank_all(&[42.0], "default");
                    objective.len() == 1 && reversed.len() == 1 && identity.len() == 1
                        && objective[0].index == 0 && objective[0].fitness == 42.0 }
        };
        Findings {
            objective_ranking_delegates_to_the_model,
            reversed_ranking_flips_the_tie_blocks,
            the_default_rank_indices_is_stable,
            a_duplicate_index_is_cloned_twice,
            an_out_of_range_index_panics,
            identity_ranking_never_touches_the_model,
            ranking_returns_clones_and_leaves_the_input_untouched,
            empty_and_single_inputs_are_fixed_points,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> RankingProbeRequest {
        RankingProbeRequest { fitnesses: vec![3.0, 1.0, 2.0], rank_mode: None }
    }

    fn indices(rows: &[RankingItemRow]) -> Vec<u32> {
        rows.iter().map(|row| row.index).collect()
    }

    #[test]
    fn the_eight_rules_hold_on_a_plain_list() {
        let report = verify_ranking(&request()).unwrap();
        assert!(report.objective_ranking_delegates_to_the_model);
        assert!(report.reversed_ranking_flips_the_tie_blocks);
        assert!(report.the_default_rank_indices_is_stable);
        assert!(report.a_duplicate_index_is_cloned_twice);
        assert!(report.an_out_of_range_index_panics);
        assert!(report.identity_ranking_never_touches_the_model);
        assert!(report.ranking_returns_clones_and_leaves_the_input_untouched);
        assert!(report.empty_and_single_inputs_are_fixed_points);
        // 默认排序：`[1, 2, 3]` 的下标是 `[1, 2, 0]`。 / With the default the indices become `[1, 2, 0]`.
        assert_eq!(indices(&report.input), vec![0, 1, 2]);
        assert_eq!(indices(&report.objective), vec![1, 2, 0]);
        assert_eq!(indices(&report.reversed), vec![0, 2, 1]);
        assert_eq!(indices(&report.identity), vec![0, 1, 2]);
        assert!(!report.objective_panicked);
        assert_eq!(report.objective[0].fitness, 1.0);
    }

    #[test]
    fn a_reversing_model_turns_the_ascending_ranking_descending() {
        let report = verify_ranking(&RankingProbeRequest { fitnesses: vec![1.0, 2.0, 3.0],
            rank_mode: Some("reverse".to_owned()) }).unwrap();
        // 适应度升序，输出却是降序——排序算子一个比较都没做。 
        // The fitness values ascend yet the output descends: the operator itself compared nothing.
        assert_eq!(indices(&report.objective), vec![2, 1, 0]);
        assert_eq!(report.objective[0].fitness, 3.0);
    }

    #[test]
    fn ties_keep_the_input_order_but_the_reversed_variant_flips_them() {
        let report = verify_ranking(&RankingProbeRequest { fitnesses: vec![5.0, 5.0, 5.0],
            rank_mode: None }).unwrap();
        assert_eq!(indices(&report.objective), vec![0, 1, 2]);
        assert_eq!(indices(&report.reversed), vec![2, 1, 0]);
    }

    #[test]
    fn a_duplicate_index_shows_up_twice() {
        let report = verify_ranking(&RankingProbeRequest { fitnesses: vec![7.0, 8.0],
            rank_mode: Some("duplicate".to_owned()) }).unwrap();
        assert_eq!(report.objective.len(), 2);
        assert!(report.objective.iter().all(|row| row.index == 0));
        assert!(report.reversed.iter().all(|row| row.index == 0));
    }

    #[test]
    fn an_out_of_range_index_panics_the_objective_path_only() {
        let report = verify_ranking(&RankingProbeRequest { fitnesses: vec![1.0, 2.0],
            rank_mode: Some("outOfRange".to_owned()) }).unwrap();
        assert!(report.objective_panicked);
        assert!(report.objective.is_empty());
        assert!(report.reversed.is_empty());
        // `IdentityRanking` 不理模型，所以它照旧按原序。 / IdentityRanking ignores the model and keeps the input order.
        assert_eq!(indices(&report.identity), vec![0, 1]);
    }

    #[test]
    fn bad_input_is_refused() {
        assert!(verify_ranking(&RankingProbeRequest { fitnesses: (0..33).map(f64::from)
            .collect(), rank_mode: None }).unwrap_err().message
            .contains("at most 32 individuals"));
        assert!(verify_ranking(&RankingProbeRequest { fitnesses: vec![f64::INFINITY],
            rank_mode: None }).unwrap_err().message.contains("non-finite fitness"));
        assert!(verify_ranking(&RankingProbeRequest { fitnesses: vec![1.0],
            rank_mode: Some("nope".to_owned()) }).unwrap_err().message
            .contains("unknown rank mode"));
    }

    /// 守门：DTO 必须是 camelCase 且拒绝未知字段。 / A guard: camelCase wire, unknown fields refused.
    #[test]
    fn the_report_wire_is_camel_case_and_rejects_unknown_fields() {
        let report = verify_ranking(&request()).unwrap();
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"objectivePanicked\""), "{json}");
        assert!(json.contains("\"theDefaultRankIndicesIsStable\""), "{json}");
        assert!(json.contains("\"reversedRankingFlipsTheTieBlocks\""), "{json}");
        assert!(!json.contains("objective_panicked"), "{json}");
        let mut copy: serde_json::Value = serde_json::from_str(&json).unwrap();
        copy["unexpected"] = serde_json::json!(1);
        assert!(serde_json::from_str::<RankingProbeReport>(&copy.to_string()).is_err());
    }
}
