//! 续挖第 95 单：**两个多样性度量对"维度"和"缺失值"的回答不一致**——`MeanPairwiseDistance` 的维度取的是
//! **第一个解的条目数**（`Solution` 是 `HashMap`，`len()` 是键数而不是变量数），而 `FractalDimensionDiversity`
//! 取的是 **`order.len()`**；遇到缺值前者把 `NaN` 一路传下去、后者**当成 0**；两者都把退化种群读成 0
//! （`solver/heuristic/operator/diversity.rs` 的 `MeanPairwiseDistance` / `FractalDimensionDiversity`）
//! Continuation, delivery 95: **the two diversity metrics disagree about "dimension" and about missing values** —
//! `MeanPairwiseDistance` takes its dimension from **the first solution's entry count** (`Solution` is a `HashMap`, so
//! `len()` is the key count rather than the variable count) while `FractalDimensionDiversity` takes **`order.len()`**; on a
//! missing value the former propagates `NaN` all the way out while the latter **treats it as zero**; and both read a
//! degenerate population as zero (`MeanPairwiseDistance` / `FractalDimensionDiversity` in
//! `solver/heuristic/operator/diversity.rs`).
//!
//! 第 94 单量的是分段线性，这一单量**种群多样性**。八条实测：
//!
//! 1. **不到两个解就返回 0**（`the_mean_returns_zero_below_two_solutions()`）。
//! 2. **均值度量的维度取的是第一个解的条目数**（`the_mean_takes_its_dimension_from_the_first_solution()`）：
//!    `Solution` 是 `HashMap`，`solutions[0].len()` 是**键数**，`order` 只用来取键。
//! 3. **一个缺值就把均值度量的结果变成 NaN**（`a_missing_value_poisons_the_mean()`）：`unwrap_or(f64::NAN)`。
//!    本单最值钱的一条。
//! 4. **完全相同的解读成 0**（`the_mean_of_identical_solutions_is_zero()`）。
//! 5. **均值是"距离的均值"不是"平方距离的均值"**（`the_mean_averages_distances_not_their_squares()`）：
//!    `0/3/4` 三点给 `8/3`，而不是 `26/3`。
//! 6. **分形度量把缺值当成 0**（`the_fractal_metric_reads_a_missing_value_as_zero()`）：`.unwrap_or(0.0)`，
//!    与第 3 条正好相反。
//! 7. **分形度量看的是 `order` 的每一个位置**（`the_fractal_metric_sees_every_order_position()`）：
//!    它把缺失的键补 0，于是"同一个种群"在均值度量和分形度量下读出不同的几何。
//! 8. **两个度量都把退化种群读成 0**（`both_metrics_collapse_a_degenerate_population_to_zero()`）：
//!    单点与全同点都是 0，而"算错"也被 `unwrap_or(0.0)` 压成同一个 0。
//!
//! Delivery 94 measured the piecewise constructor; this one measures **population diversity**. **Fewer than two solutions
//! returns zero.** **The mean metric takes its dimension from the first solution's entry count** because `Solution` is a
//! `HashMap` and `len()` is the key count, with `order` used only for lookups. **One missing value turns the mean into
//! `NaN`** — the most valuable finding here. **Identical solutions read as zero.** **The mean averages distances rather
//! than their squares** (`0/3/4` gives `8/3`, not `26/3`). **The fractal metric treats a missing value as zero**, the exact
//! opposite of rule three. **It sees every `order` position**, padding missing keys with zeros, so one population reads as two different geometries. And **both collapse a
//! degenerate population to zero**, where `unwrap_or(0.0)` makes a computation failure indistinguishable from a real zero.

use ospf_rust_core::model::callback::Solution;
use ospf_rust_core_plugin_heuristic::heuristic::operator::diversity::{
    DiversityMetric, FractalDimensionDiversity, MeanPairwiseDistance,
};
use ospf_rust_core::variable::VariableId;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 多样性探针请求。 / The diversity probe request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DiversityRequest {
    /// 每个解按 `order` 位置给出的值；`null` 表示这个键**不存在**。
    /// Each solution's values by `order` position; `null` means the key is **absent**.
    #[serde(default)]
    pub solutions: Vec<Vec<Option<f64>>>,
    /// `order` 的长度（可以比解里的条目多）。 / The length of `order`, which may exceed a solution's entries.
    #[serde(default)]
    pub order_len: Option<u32>,
}

/// 多样性探针结论。 / The diversity probe verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DiversityReport {
    /// 平均成对距离（`NaN` 时这里是 `null`）。 / The mean pairwise distance, `null` when it is `NaN`.
    #[serde(default)]
    pub mean: Option<f64>,
    /// 平均成对距离是否算出了 `NaN`。 / Whether the mean came out `NaN`.
    pub mean_nan: bool,
    /// 相关维数（算不出来就是 0）。 / The correlation dimension, zero when it cannot be computed.
    pub fractal: f64,
    /// 第一个解的条目数（均值度量真正的"维度"）。 / The first solution's entry count, the mean metric's real dimension.
    pub first_entry_count: u32,
    /// `order` 的长度。 / The length of `order`.
    pub order_len: u32,
    /// 八条规则属性的读数。 / The eight rule readings.
    pub the_mean_returns_zero_below_two_solutions: bool,
    pub the_mean_takes_its_dimension_from_the_first_solution: bool,
    pub a_missing_value_poisons_the_mean: bool,
    pub the_mean_of_identical_solutions_is_zero: bool,
    pub the_mean_averages_distances_not_their_squares: bool,
    pub the_fractal_metric_reads_a_missing_value_as_zero: bool,
    pub the_fractal_metric_sees_every_order_position: bool,
    pub both_metrics_collapse_a_degenerate_population_to_zero: bool,
}

/// 按请求铺出 `order` 与解集。 / Lay out `order` and the solution set from the request.
fn layout(solutions: &[Vec<Option<f64>>], order_len: usize) -> (Vec<VariableId>, Vec<Solution<f64>>) {
    let order: Vec<VariableId> = (0..order_len)
        .map(|index| VariableId::new(1, index)).collect();
    let built = solutions.iter().map(|values| {
        let mut solution = Solution::new();
        for (index, value) in values.iter().enumerate() {
            if let (Some(value), Some(id)) = (value, order.get(index)) {
                solution.insert(*id, *value);
            }
        }
        solution
    }).collect();
    (order, built)
}

/// 走一遍多样性探针。非法输入是**错误**。 / Walk the diversity probe.
pub fn verify_diversity(request: &DiversityRequest) -> Result<DiversityReport> {
    if request.solutions.len() > 8 {
        return Err(BridgeError::new("InvalidInput",
            "diversity probe failed validation: at most eight solutions are accepted"));
    }
    if request.solutions.iter().any(|values| values.len() > 8) {
        return Err(BridgeError::new("InvalidInput",
            "diversity probe failed validation: at most eight entries per solution"));
    }
    let candidates: Vec<f64> = request.solutions.iter().flatten().flatten().copied().collect();
    if candidates.iter().any(|value| !value.is_finite()) {
        return Err(BridgeError::new("InvalidInput", "diversity probe failed validation: \
            non-finite values cannot cross the boundary"));
    }
    let order_len = usize::try_from(request.order_len.unwrap_or_else(|| {
        request.solutions.iter().map(Vec::len).max().unwrap_or(0) as u32
    })).map_err(|_| BridgeError::new("InvalidInput",
        "diversity probe failed validation: the order length overflowed"))?;
    if order_len > 8 {
        return Err(BridgeError::new("InvalidInput",
            "diversity probe failed validation: the order holds at most eight ids"));
    }
    let findings = canonical_findings();
    let (order, built) = layout(&request.solutions, order_len);
    let raw_mean = MeanPairwiseDistance.measure(&built, &order);
    let fractal = FractalDimensionDiversity.measure(&built, &order);
    Ok(DiversityReport {
        mean: if raw_mean.is_nan() { None } else { Some(raw_mean) },
        mean_nan: raw_mean.is_nan(),
        fractal,
        first_entry_count: u32::try_from(built.first().map(Solution::len).unwrap_or(0))
            .unwrap_or(u32::MAX),
        order_len: u32::try_from(order.len()).unwrap_or(u32::MAX),
        the_mean_returns_zero_below_two_solutions:
            findings.the_mean_returns_zero_below_two_solutions,
        the_mean_takes_its_dimension_from_the_first_solution:
            findings.the_mean_takes_its_dimension_from_the_first_solution,
        a_missing_value_poisons_the_mean: findings.a_missing_value_poisons_the_mean,
        the_mean_of_identical_solutions_is_zero: findings.the_mean_of_identical_solutions_is_zero,
        the_mean_averages_distances_not_their_squares:
            findings.the_mean_averages_distances_not_their_squares,
        the_fractal_metric_reads_a_missing_value_as_zero:
            findings.the_fractal_metric_reads_a_missing_value_as_zero,
        the_fractal_metric_sees_every_order_position:
            findings.the_fractal_metric_sees_every_order_position,
        both_metrics_collapse_a_degenerate_population_to_zero:
            findings.both_metrics_collapse_a_degenerate_population_to_zero,
    })
}

/// 固定探针只跑一次。 / The fixed probes run once only.
static FINDINGS: std::sync::OnceLock<Findings> = std::sync::OnceLock::new();

/// 八条规则属性的内部固定探针。 / The eight rule properties measured by fixed internal probes.
struct Findings {
    the_mean_returns_zero_below_two_solutions: bool,
    the_mean_takes_its_dimension_from_the_first_solution: bool,
    a_missing_value_poisons_the_mean: bool,
    the_mean_of_identical_solutions_is_zero: bool,
    the_mean_averages_distances_not_their_squares: bool,
    the_fractal_metric_reads_a_missing_value_as_zero: bool,
    the_fractal_metric_sees_every_order_position: bool,
    both_metrics_collapse_a_degenerate_population_to_zero: bool,
}

/// 一次探针：给解集与 `order` 长度，读回两个度量。 / One probe: solutions plus an order length, read both metrics.
fn probe(solutions: &[Vec<Option<f64>>], order_len: usize) -> (f64, f64) {
    let (order, built) = layout(solutions, order_len);
    (MeanPairwiseDistance.measure(&built, &order),
        FractalDimensionDiversity.measure(&built, &order))
}

/// 值数组（全有值）。 / A value array with every key present.
fn values(items: &[f64]) -> Vec<Option<f64>> {
    items.iter().map(|item| Some(*item)).collect()
}

/// 用固定探针测出全部规则属性——不读调用方的请求值。
/// Measure every rule property with fixed probes; the caller's request values are not read.
fn canonical_findings() -> &'static Findings {
    FINDINGS.get_or_init(|| {
        // 1) 不到两个解就返回 0。 / Fewer than two solutions returns zero.
        let the_mean_returns_zero_below_two_solutions = {
            // 0 个解、1 个解都是 0；两个解才有正值。 
            // Zero and one solution both give zero while two solutions give a positive value.
            probe(&[], 1).0 == 0.0
                && probe(&[values(&[4.0])], 1).0 == 0.0
                && probe(&[values(&[0.0]), values(&[5.0])], 1).0 == 5.0
        };
        // 2) 均值度量的维度取的是第一个解的条目数。 / The mean's dimension is the first solution's entry count.
        let the_mean_takes_its_dimension_from_the_first_solution = {
            // **同一份几何**，只是键装在不同的解上：第一个解只装 1 个键 → 循环只走 `k = 0`，第二维上的 100 看不见。 
            // The **same geometry** with the keys placed differently: the first solution holds one key, so the loop runs
            // `k = 0` only and the gap of 100 in the second coordinate stays invisible.
            let shallow = probe(&[vec![Some(0.0), None, None], values(&[0.0, 100.0, 0.0])], 3).0;
            // 把键放到第一个解上 → 三个维度都参与，结果变成 100。 
            // Moving the keys onto the first solution makes all three coordinates count, so the reading becomes 100.
            let deep = probe(&[values(&[0.0, 0.0, 0.0]), values(&[0.0, 100.0, 0.0])], 3).0;
            shallow == 0.0 && deep == 100.0
        };
        // 3) 一个缺值就把均值度量的结果变成 NaN。 / One missing value turns the mean into NaN.
        let a_missing_value_poisons_the_mean = {
            // 第一个解两个键、第二个解只有一个键 → `k = 1` 处缺值 → `NaN`。 
            // The first solution has two keys and the second only one, so the second coordinate is missing and yields NaN.
            let (mean, _) = probe(&[values(&[1.0, 2.0]), vec![Some(1.0), None]], 2);
            mean.is_nan()
                // 对照：键齐全时是正常的 0。 / Control: with every key present the mean is a normal zero.
                && probe(&[values(&[1.0, 2.0]), values(&[1.0, 2.0])], 2).0 == 0.0
        };
        // 4) 完全相同的解读成 0。 / Identical solutions read as zero.
        let the_mean_of_identical_solutions_is_zero = {
            probe(&[values(&[3.0, 1.0]), values(&[3.0, 1.0]), values(&[3.0, 1.0])], 2).0 == 0.0
        };
        // 5) 均值是"距离的均值"不是"平方距离的均值"。 / The mean averages distances rather than their squares.
        let the_mean_averages_distances_not_their_squares = {
            // 一条直线上的 `0 / 3 / 4`：三对距离是 4、1、3 → 均值 `8/3`；平方均值会是 `26/3`。 
            // On a line, `0 / 3 / 4` gives the distances 4, 1 and 3, so the mean is `8/3` where the squared mean is `26/3`.
            let mean = probe(&[values(&[0.0]), values(&[3.0]), values(&[4.0])], 1).0;
            (mean - 8.0 / 3.0).abs() < 1e-12 && (mean - 26.0 / 3.0).abs() > 1.0
        };
        // 6) 分形度量把缺值当成 0。 / The fractal metric treats a missing value as zero.
        let the_fractal_metric_reads_a_missing_value_as_zero = {
            // 第一个解两个键、第二个只一个键：均值是 NaN，而分形度量**照样给出有限值**。 
            // The first solution has two keys and the second one, so the mean is NaN while the fractal metric still returns a
            // finite value.
            let (mean, fractal) = probe(&[values(&[1.0, 2.0]), vec![Some(1.0), None]], 2);
            mean.is_nan() && fractal.is_finite()
                // 缺值被当成 0：与"真的有 0"结果一致。 
                // The missing value is read as zero, so it matches a genuine zero.
                && (fractal - probe(&[values(&[1.0, 2.0]), values(&[1.0, 0.0])], 2).1).abs() < 1e-12
        };
        // 7) 分形度量看的是 `order` 的每一个位置。 / The fractal metric sees every order position.
        let the_fractal_metric_sees_every_order_position = {
            // 第一个解只装了 1 个键（均值度量的维度是 1），而 `order` 有 3 个 → 分形度量看全三个坐标，
            // 于是它的读数与"把缺的键显式补 0"完全一致。 
            // The first solution holds one key (the mean's dimension is one) against an order of three, so the fractal metric
            // sees all three coordinates and matches the population where the missing keys are explicitly zero.
            let short = [vec![Some(0.0), None, None], values(&[0.0, 100.0, 0.0])];
            let padded = [values(&[0.0, 0.0, 0.0]), values(&[0.0, 100.0, 0.0])];
            let (short_mean, short_fractal) = probe(&short, 3);
            let (padded_mean, padded_fractal) = probe(&padded, 3);
            // 同一个种群：均值一个是 0、一个是 100，分形读数却一样。 
            // For one and the same population the means are 0 and 100 while the fractal readings agree.
            short_mean == 0.0 && padded_mean == 100.0
                && (short_fractal - padded_fractal).abs() < 1e-12
                && short_fractal.is_finite()
        };
        // 8) 两个度量都把退化种群读成 0。 / Both metrics collapse a degenerate population to zero.
        let both_metrics_collapse_a_degenerate_population_to_zero = {
            // 单点：两个都是 0。 / A single solution: both give zero.
            let single = probe(&[values(&[7.0, 3.0])], 2);
            // 全同点：均值 0，分形也被压到 0（算不出来就 `unwrap_or(0.0)`）。 
            // Identical solutions: the mean is zero and the fractal is clamped to zero through `unwrap_or(0.0)`.
            let identical = probe(&[values(&[1.0, 1.0]), values(&[1.0, 1.0])], 2);
            single.0 == 0.0 && single.1 == 0.0 && identical.0 == 0.0 && identical.1 == 0.0
        };
        Findings {
            the_mean_returns_zero_below_two_solutions,
            the_mean_takes_its_dimension_from_the_first_solution,
            a_missing_value_poisons_the_mean,
            the_mean_of_identical_solutions_is_zero,
            the_mean_averages_distances_not_their_squares,
            the_fractal_metric_reads_a_missing_value_as_zero,
            the_fractal_metric_sees_every_order_position,
            both_metrics_collapse_a_degenerate_population_to_zero,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> DiversityRequest {
        DiversityRequest { solutions: vec![values(&[0.0]), values(&[3.0]), values(&[4.0])],
            order_len: Some(1) }
    }

    #[test]
    fn the_eight_rules_hold_on_a_plain_run() {
        let report = verify_diversity(&request()).unwrap();
        assert!(report.the_mean_returns_zero_below_two_solutions);
        assert!(report.the_mean_takes_its_dimension_from_the_first_solution);
        assert!(report.a_missing_value_poisons_the_mean);
        assert!(report.the_mean_of_identical_solutions_is_zero);
        assert!(report.the_mean_averages_distances_not_their_squares);
        assert!(report.the_fractal_metric_reads_a_missing_value_as_zero);
        assert!(report.the_fractal_metric_sees_every_order_position);
        assert!(report.both_metrics_collapse_a_degenerate_population_to_zero);
        // `0 / 3 / 4` → `8/3`（距离的均值）。 / `0 / 3 / 4` gives `8/3`, the mean of the distances.
        let mean = report.mean.unwrap();
        assert!((mean - 8.0 / 3.0).abs() < 1e-12);
        assert!(!report.mean_nan);
        assert_eq!(report.first_entry_count, 1);
        assert_eq!(report.order_len, 1);
    }

    #[test]
    fn fewer_than_two_solutions_reads_zero() {
        for solutions in [Vec::new(), vec![values(&[4.0])]] {
            let report = verify_diversity(&DiversityRequest { solutions,
                order_len: Some(1) }).unwrap();
            assert_eq!(report.mean, Some(0.0));
            assert_eq!(report.fractal, 0.0);
        }
    }

    #[test]
    fn a_missing_key_turns_the_mean_into_nan_but_not_the_fractal() {
        let report = verify_diversity(&DiversityRequest { solutions: vec![values(&[1.0, 2.0]),
            vec![Some(1.0), None]], order_len: Some(2) }).unwrap();
        // 均值：`NaN`（键缺失），分形：有限值（补 0）。 
        // The mean is NaN for the missing key while the fractal stays finite through zero padding.
        assert!(report.mean_nan);
        assert_eq!(report.mean, None);
        assert!(report.fractal.is_finite());
        assert_eq!(report.first_entry_count, 2);
    }

    #[test]
    fn the_mean_dimension_follows_the_first_solution() {
        // 同一份几何，键装在第一个解上 → 100；装在第二个解上 → 0。 
        // One geometry: keys on the first solution read 100, keys on the second read 0.
        let deep = verify_diversity(&DiversityRequest { solutions: vec![values(&[0.0, 0.0, 0.0]),
            values(&[0.0, 100.0, 0.0])], order_len: Some(3) }).unwrap();
        assert_eq!(deep.mean, Some(100.0));
        let shallow = verify_diversity(&DiversityRequest { solutions: vec![
            vec![Some(0.0), None, None], values(&[0.0, 100.0, 0.0])], order_len: Some(3) })
            .unwrap();
        assert_eq!(shallow.mean, Some(0.0));
        assert_eq!(shallow.first_entry_count, 1);
        assert_eq!(shallow.order_len, 3);
    }

    #[test]
    fn identical_solutions_read_zero_on_both() {
        let report = verify_diversity(&DiversityRequest { solutions: vec![values(&[3.0, 1.0]),
            values(&[3.0, 1.0]), values(&[3.0, 1.0])], order_len: Some(2) }).unwrap();
        assert_eq!(report.mean, Some(0.0));
        assert_eq!(report.fractal, 0.0);
    }

    #[test]
    fn bad_input_is_refused() {
        assert!(verify_diversity(&DiversityRequest { solutions: vec![values(&[1.0]); 9],
            order_len: Some(1) }).unwrap_err().message.contains("at most eight solutions"));
        assert!(verify_diversity(&DiversityRequest { solutions: vec![values(&[1.0; 9])],
            order_len: Some(1) }).unwrap_err().message.contains("at most eight entries"));
        assert!(verify_diversity(&DiversityRequest { solutions: vec![values(&[f64::NAN])],
            order_len: Some(1) }).unwrap_err().message.contains("non-finite values"));
        assert!(verify_diversity(&DiversityRequest { solutions: vec![values(&[1.0])],
            order_len: Some(9) }).unwrap_err().message.contains("at most eight ids"));
    }

    /// 守门：DTO 必须是 camelCase 且拒绝未知字段。 / A guard: camelCase wire, unknown fields refused.
    #[test]
    fn the_report_wire_is_camel_case_and_rejects_unknown_fields() {
        let report = verify_diversity(&request()).unwrap();
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"meanNan\""), "{json}");
        assert!(json.contains("\"firstEntryCount\""), "{json}");
        assert!(json.contains("\"aMissingValuePoisonsTheMean\""), "{json}");
        assert!(!json.contains("mean_nan"), "{json}");
        let mut copy: serde_json::Value = serde_json::from_str(&json).unwrap();
        copy["unexpected"] = serde_json::json!(1);
        assert!(serde_json::from_str::<DiversityReport>(&copy.to_string()).is_err());
    }
}
