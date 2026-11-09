//! 续挖第 83 单：**NSGA-III 参考点**——点的个数是单纯形格点、顺序按首坐标字典序、一个目标必塌成单位点，
//! 而"垂直距离"忽略参考方向之外的维度、且对参考方向**尺度不变**
//! （`solver/heuristic/operator/multi_objective/reference_points.rs` 的 `ReferencePoints`）
//! Continuation, delivery 83: the **NSGA-III reference points** — the point count is the simplex lattice, the order is
//! lexicographic in the first coordinate, a single objective collapses to the unit point, while the "perpendicular
//! distance" ignores dimensions past the reference and is **scale-invariant in the reference direction**
//! (`ReferencePoints` in `solver/heuristic/operator/multi_objective/reference_points.rs`).
//!
//! 第 81/82 单量的是偏序与分层；这一单量**铺方向的那批参考点**。八条实测：
//!
//! 1. **点数就是单纯形格点数**（`the_point_count_follows_the_simplex_lattice()`）：
//!    `C(divisions + m - 1, m - 1)`——`(2,4)=5`、`(3,4)=15`、`(2,1)=2`。
//! 2. **非法维度给空表而不是 panic**（`an_illegal_dimension_yields_an_empty_list()`）：
//!    `objective_amount == 0` 或 `divisions == 0` 一律 `Vec::new()`。
//! 3. **一个目标必塌成单位点**（`one_objective_collapses_to_the_unit_point()`）：`(1, n)` 恒为 `[[1.0]]`。
//! 4. **每个点坐标和为 1**（`every_point_sums_to_one()`）：`share / divisions` 的构造保证（容差 1e-12）。
//! 5. **顺序按首坐标字典序**（`the_order_is_lexicographic_in_the_first_coordinate()`）：`share in 0..=remaining`
//!    推入，所以输出顺序是确定的、可复现的。
//! 6. **距离忽略参考方向之外的维度**（`the_distance_ignores_objectives_beyond_the_reference()`）：
//!    循环走 `0..reference.len()`，多出来的目标维度不参与。
//! 7. **缺目标或零参考都是无穷**（`a_missing_objective_or_a_zero_reference_is_infinite()`）：
//!    `objectives.get(index)` 取不到 → `INFINITY`；参考范数 ≤ 1e-12 → `INFINITY`。
//! 8. **距离对参考方向的尺度不变**（`the_distance_is_invariant_to_reference_scaling()`）：投影按
//!    `reference_norm` 归一，`[1,0]` 与 `[2,0]` 给同一个距离。
//!
//! Deliveries 81 and 82 measured the partial order and the layering; this one measures **the directions the front is
//! supposed to be spread over**. **The point count is the simplex lattice count** `C(divisions + m - 1, m - 1)`. **An
//! illegal dimension yields an empty list rather than a panic.** **One objective collapses to the unit point** for every
//! division count. **Every point's coordinates sum to one.** **The order is lexicographic in the first coordinate**, so the
//! output is deterministic. **The distance ignores objectives past the reference.** **A missing objective or a zero
//! reference is infinite.** And **the distance is invariant to reference scaling**.

use ospf_rust_core::solver::heuristic::operator::multi_objective::reference_points::ReferencePoints;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 一对（目标向量、参考方向）。 / One (objective vector, reference direction) pair.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReferencePairSpec {
    /// 目标向量。 / The objective vector.
    #[serde(default)]
    pub objectives: Vec<f64>,
    /// 参考方向。 / The reference direction.
    #[serde(default)]
    pub reference: Vec<f64>,
}

/// 参考点探针请求。 / The reference-points probe request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReferencePointsRequest {
    /// 目标维度 `m`。 / The objective amount `m`.
    pub objective_amount: u32,
    /// 每维的分割数。 / The division count per axis.
    pub divisions: u32,
    /// 要问垂直距离的配对。 / The pairs to ask the perpendicular distance about.
    #[serde(default)]
    pub pairs: Vec<ReferencePairSpec>,
}

/// 一对的垂直距离读数（非有限值只能以 `infinite` 表达，JSON 载不动 `inf`）。
/// One pair's perpendicular-distance reading; a non-finite value can only be expressed as `infinite`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReferencePairView {
    /// 距离是否为无穷。 / Whether the distance is infinite.
    pub infinite: bool,
    /// 有限距离；`None` 表示无穷。 / The finite distance; `None` means infinite.
    #[serde(default)]
    pub distance: Option<f64>,
}

/// 参考点探针结论。 / The reference-points probe verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReferencePointsReport {
    /// 目标维度回显。 / The objective amount echoed back.
    pub objective_amount: u32,
    /// 分割数回显。 / The division count echoed back.
    pub divisions: u32,
    /// 生成的参考点个数。 / How many reference points were generated.
    pub point_count: u32,
    /// 每维的分割数（参考点个数与它有界关系）。 / The division count that bounds the point count.
    pub axis_width: u32,
    /// 生成的参考点（按输出顺序；个数为 0 时为空）。 / The generated points in output order.
    pub points: Vec<Vec<f64>>,
    /// 每个点的坐标和是否为 1（容差 1e-12）。 / Whether every point's coordinates sum to one.
    pub every_point_sums_to_one: bool,
    /// 每对的垂直距离。 / Each pair's perpendicular distance.
    pub pairs: Vec<ReferencePairView>,
    /// 八条规则属性的读数。 / The eight rule readings.
    pub the_point_count_follows_the_simplex_lattice: bool,
    pub an_illegal_dimension_yields_an_empty_list: bool,
    pub one_objective_collapses_to_the_unit_point: bool,
    pub every_point_sums_to_one_measured: bool,
    pub the_order_is_lexicographic_in_the_first_coordinate: bool,
    pub the_distance_ignores_objectives_beyond_the_reference: bool,
    pub a_missing_objective_or_a_zero_reference_is_infinite: bool,
    pub the_distance_is_invariant_to_reference_scaling: bool,
}

/// 单纯形格点数 `C(divisions + m - 1, m - 1)`；非法维度给 0。
/// The simplex lattice count `C(divisions + m - 1, m - 1)`; an illegal dimension gives 0.
fn lattice_count(objective_amount: u32, divisions: u32) -> u64 {
    if objective_amount == 0 || divisions == 0 {
        return 0;
    }
    // 组合数用 u128 中间量，避免 `u64` 溢出。 / Use a u128 intermediate so `u64` cannot overflow.
    let n = u128::from(divisions).saturating_add(u128::from(objective_amount)).saturating_sub(1);
    let m = u128::from(objective_amount).saturating_sub(1);
    // `C(n, m) = C(n, n - m)`：取较小的那个迭代。 / Iterate the smaller of `m` and `n - m`.
    let k = m.min(n.saturating_sub(m));
    let mut value: u128 = 1;
    for step in 0..k {
        value = value.saturating_mul(n.saturating_sub(step));
        value /= step + 1;
    }
    u64::try_from(value).unwrap_or(u64::MAX)
}

/// 一个参考点是否坐标和为 1。 / Whether one reference point's coordinates sum to one.
fn sums_to_one(point: &[f64]) -> bool {
    (point.iter().sum::<f64>() - 1.0).abs() <= 1.0e-12
}

/// 走一遍参考点探针。非法输入是**错误**。 / Walk the reference-points probe.
pub fn verify_reference_points(request: &ReferencePointsRequest) -> Result<ReferencePointsReport> {
    if request.objective_amount > 8 {
        return Err(BridgeError::new("InvalidInput",
            "reference points failed validation: at most 8 objectives are accepted"));
    }
    if request.divisions > 16 {
        return Err(BridgeError::new("InvalidInput",
            "reference points failed validation: at most 16 divisions are accepted"));
    }
    let counted = lattice_count(request.objective_amount, request.divisions);
    if counted > 512 {
        return Err(BridgeError::new("InvalidInput", &format!(
            "reference points failed validation: {counted} reference points exceed the 512-point cap")));
    }
    if request.pairs.len() > 32 {
        return Err(BridgeError::new("InvalidInput",
            "reference points failed validation: at most 32 pairs are accepted"));
    }
    for spec in &request.pairs {
        if spec.objectives.len() > 8 || spec.reference.len() > 8 {
            return Err(BridgeError::new("InvalidInput",
                "reference points failed validation: at most 8 axes per pair are accepted"));
        }
        if spec.objectives.iter().chain(spec.reference.iter()).any(|value| !value.is_finite()) {
            return Err(BridgeError::new("InvalidInput", "reference points failed validation: \
                non-finite values cannot cross the boundary"));
        }
    }
    let findings = canonical_findings();
    let amount = usize::try_from(request.objective_amount)
        .map_err(|_| BridgeError::new("InvalidInput",
            "reference points failed validation: objective amount overflow"))?;
    let divisions = usize::try_from(request.divisions)
        .map_err(|_| BridgeError::new("InvalidInput",
            "reference points failed validation: division count overflow"))?;
    let points = ReferencePoints::das_dennis(amount, divisions);
    let every_point_sums_to_one = points.iter().all(|point| sums_to_one(point));
    let pairs = request.pairs.iter().map(|spec| {
        let distance = ReferencePoints::perpendicular_distance(&spec.objectives, &spec.reference);
        ReferencePairView { infinite: !distance.is_finite(),
            distance: distance.is_finite().then_some(distance) }
    }).collect::<Vec<_>>();
    Ok(ReferencePointsReport {
        objective_amount: request.objective_amount,
        divisions: request.divisions,
        point_count: u32::try_from(points.len()).unwrap_or(u32::MAX),
        axis_width: request.divisions,
        points,
        every_point_sums_to_one,
        pairs,
        the_point_count_follows_the_simplex_lattice:
            findings.the_point_count_follows_the_simplex_lattice,
        an_illegal_dimension_yields_an_empty_list:
            findings.an_illegal_dimension_yields_an_empty_list,
        one_objective_collapses_to_the_unit_point:
            findings.one_objective_collapses_to_the_unit_point,
        every_point_sums_to_one_measured: findings.every_point_sums_to_one_measured,
        the_order_is_lexicographic_in_the_first_coordinate:
            findings.the_order_is_lexicographic_in_the_first_coordinate,
        the_distance_ignores_objectives_beyond_the_reference:
            findings.the_distance_ignores_objectives_beyond_the_reference,
        a_missing_objective_or_a_zero_reference_is_infinite:
            findings.a_missing_objective_or_a_zero_reference_is_infinite,
        the_distance_is_invariant_to_reference_scaling:
            findings.the_distance_is_invariant_to_reference_scaling,
    })
}

/// 八条规则属性的内部固定探针。 / The eight rule properties measured by fixed internal probes.
struct Findings {
    the_point_count_follows_the_simplex_lattice: bool,
    an_illegal_dimension_yields_an_empty_list: bool,
    one_objective_collapses_to_the_unit_point: bool,
    every_point_sums_to_one_measured: bool,
    the_order_is_lexicographic_in_the_first_coordinate: bool,
    the_distance_ignores_objectives_beyond_the_reference: bool,
    a_missing_objective_or_a_zero_reference_is_infinite: bool,
    the_distance_is_invariant_to_reference_scaling: bool,
}

/// 用固定探针测出全部规则属性——不读调用方的请求值。
/// Measure every rule property with fixed probes; the caller's request values are not read.
fn canonical_findings() -> Findings {
    // 1) 点数就是单纯形格点数。 / The point count is the simplex lattice count.
    let the_point_count_follows_the_simplex_lattice = {
        // `(2,4)=5`、`(3,4)=15`、`(2,1)=2`、`(4,2)=10`——四组都对着格点公式。 
        // (2,4)=5, (3,4)=15, (2,1)=2 and (4,2)=10, all matching the lattice formula.
        ReferencePoints::das_dennis(2, 4).len() == 5
            && ReferencePoints::das_dennis(3, 4).len() == 15
            && ReferencePoints::das_dennis(2, 1).len() == 2
            && ReferencePoints::das_dennis(4, 2).len() == 10
            && lattice_count(3, 4) == 15 && lattice_count(4, 2) == 10
    };
    // 2) 非法维度给空表。 / An illegal dimension yields an empty list.
    let an_illegal_dimension_yields_an_empty_list = {
        // 两个维度都可能为 0；一个目标配 0 分割也是空（`share / divisions` 会除零）。 
        // Either dimension can be zero, and one objective with zero divisions is also empty because the constructor
        // would divide by zero.
        ReferencePoints::das_dennis(0, 4).is_empty()
            && ReferencePoints::das_dennis(3, 0).is_empty()
            && ReferencePoints::das_dennis(0, 0).is_empty()
            && ReferencePoints::das_dennis(1, 0).is_empty()
            && lattice_count(0, 4) == 0 && lattice_count(1, 0) == 0
    };
    // 3) 一个目标必塌成单位点。 / One objective collapses to the unit point.
    let one_objective_collapses_to_the_unit_point = {
        // 分割数只影响坐标格点，一个目标时只有 `divisions / divisions`。 
        // The division count only shapes the lattice; with one objective the only point is `divisions / divisions`.
        [1_usize, 2, 5, 16].iter().all(|divisions| {
            let points = ReferencePoints::das_dennis(1, *divisions);
            points.len() == 1 && points[0].len() == 1 && points[0][0] == 1.0
        })
    };
    // 4) 每个点坐标和为 1。 / Every point's coordinates sum to one.
    let every_point_sums_to_one_measured = {
        [(2_usize, 4_usize), (3, 4), (4, 3), (5, 2)].iter().all(|(amount, divisions)| {
            ReferencePoints::das_dennis(*amount, *divisions).iter()
                .all(|point| point.len() == *amount && sums_to_one(point))
        })
    };
    // 5) 顺序按首坐标字典序。 / The order is lexicographic in the first coordinate.
    let the_order_is_lexicographic_in_the_first_coordinate = {
        let points = ReferencePoints::das_dennis(2, 4);
        // 首坐标从 0 递增到 1，同首坐标内第二坐标递增。 
        // The first coordinate ascends from 0 to 1 and the second ascends inside each first coordinate.
        let firsts = points.iter().map(|point| point[0]).collect::<Vec<_>>();
        let ascending = firsts.windows(2).all(|window| window[0] <= window[1]);
        // `(2,4)` 的点是 `[0,1],[0.25,0.75],...,[1,0]`，同首坐标只有两两比较。 
        // The (2,4) points run from `[0,1]` to `[1,0]`; equal first coordinates compare on the second.
        ascending
            && points[0] == vec![0.0, 1.0]
            && points[points.len() - 1] == vec![1.0, 0.0]
            && points.windows(2).all(|window|
                window[0][0] < window[1][0] || window[0][1] < window[1][1])
    };
    // 6) 距离忽略参考方向之外的维度。 / The distance ignores objectives beyond the reference.
    let the_distance_ignores_objectives_beyond_the_reference = {
        // 参考方向只有一维：第二维无论多大都不影响。 
        // The reference has one axis only, so the second objective cannot matter no matter how large.
        let small = ReferencePoints::perpendicular_distance(&[1.0, 0.0], &[1.0]);
        let large = ReferencePoints::perpendicular_distance(&[1.0, 1.0e9], &[1.0]);
        small.abs() < 1.0e-12 && large.abs() < 1.0e-12
    };
    // 7) 缺目标或零参考都是无穷。 / A missing objective or a zero reference is infinite.
    let a_missing_objective_or_a_zero_reference_is_infinite = {
        // 目标比参考短 → `get()` 取不到 → 无穷。 
        // A shorter objective vector misses `get()` and turns infinite.
        ReferencePoints::perpendicular_distance(&[1.0], &[1.0, 0.0]).is_infinite()
            // 零参考方向 → 范数 <= 1e-12 → 无穷。 
            // A zero reference gives a norm <= 1e-12 and turns infinite.
            && ReferencePoints::perpendicular_distance(&[1.0, 1.0], &[0.0, 0.0]).is_infinite()
            // 恰好等于容差内的极小参考同样无穷。 
            // A tiny reference inside the tolerance is infinite too.
            && ReferencePoints::perpendicular_distance(&[1.0], &[1.0e-13]).is_infinite()
            // 对照：正常 pairs 有限。 / Control: an ordinary pair stays finite.
            && ReferencePoints::perpendicular_distance(&[1.0, 1.0], &[1.0, 0.0]).is_finite()
    };
    // 8) 距离对参考方向的尺度不变。 / The distance is invariant to reference scaling.
    let the_distance_is_invariant_to_reference_scaling = {
        // 投影按 `reference_norm` 归一，所以 `[1,0]` 与 `[2,0]`（乃至 `[100,0]`）同值。 
        // The projection divides by `reference_norm`, so `[1,0]`, `[2,0]` and even `[100,0]` agree.
        let unit = ReferencePoints::perpendicular_distance(&[1.0, 1.0], &[1.0, 0.0]);
        let doubled = ReferencePoints::perpendicular_distance(&[1.0, 1.0], &[2.0, 0.0]);
        let scaled = ReferencePoints::perpendicular_distance(&[1.0, 1.0], &[100.0, 0.0]);
        // 斜方向的等比缩放也一样。 / A diagonal direction scales the same way.
        let diagonal = ReferencePoints::perpendicular_distance(&[3.0, 1.0], &[1.0, 1.0]);
        let diagonal_scaled = ReferencePoints::perpendicular_distance(&[3.0, 1.0], &[5.0, 5.0]);
        (unit - doubled).abs() < 1.0e-12 && (unit - scaled).abs() < 1.0e-12
            && (unit - 1.0).abs() < 1.0e-12
            && (diagonal - diagonal_scaled).abs() < 1.0e-12
    };
    Findings {
        the_point_count_follows_the_simplex_lattice,
        an_illegal_dimension_yields_an_empty_list,
        one_objective_collapses_to_the_unit_point,
        every_point_sums_to_one_measured,
        the_order_is_lexicographic_in_the_first_coordinate,
        the_distance_ignores_objectives_beyond_the_reference,
        a_missing_objective_or_a_zero_reference_is_infinite,
        the_distance_is_invariant_to_reference_scaling,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> ReferencePointsRequest {
        ReferencePointsRequest { objective_amount: 3, divisions: 4, pairs: Vec::new() }
    }

    #[test]
    fn the_lattice_shapes_the_point_count() {
        let report = verify_reference_points(&request()).unwrap();
        assert_eq!(report.point_count, 15);
        assert_eq!(report.axis_width, 4);
        assert!(report.every_point_sums_to_one);
        assert!(report.points.iter().all(|point| point.len() == 3));
        assert!(report.the_point_count_follows_the_simplex_lattice);
        assert!(report.an_illegal_dimension_yields_an_empty_list);
        assert!(report.one_objective_collapses_to_the_unit_point);
        assert!(report.every_point_sums_to_one_measured);
        assert!(report.the_order_is_lexicographic_in_the_first_coordinate);
        assert!(report.the_distance_ignores_objectives_beyond_the_reference);
        assert!(report.a_missing_objective_or_a_zero_reference_is_infinite);
        assert!(report.the_distance_is_invariant_to_reference_scaling);
        // 首尾是单纯形的两个角。 / The ends are the two corners of the simplex.
        assert_eq!(report.points[0], vec![0.0, 0.0, 1.0]);
        assert_eq!(report.points[14], vec![1.0, 0.0, 0.0]);
    }

    #[test]
    fn an_illegal_dimension_gives_an_empty_list() {
        for (amount, divisions) in [(0_u32, 4_u32), (3, 0), (0, 0), (1, 0)] {
            let report = verify_reference_points(&ReferencePointsRequest { objective_amount: amount,
                divisions, pairs: Vec::new() }).unwrap();
            assert_eq!(report.point_count, 0, "({amount},{divisions})");
            assert!(report.points.is_empty(), "({amount},{divisions})");
            // 空表的"和为 1"是空洞为真。 / The empty list satisfies "sums to one" vacuously.
            assert!(report.every_point_sums_to_one, "({amount},{divisions})");
        }
    }

    #[test]
    fn one_objective_collapses_to_the_unit_point() {
        for divisions in [1_u32, 2, 5, 16] {
            let report = verify_reference_points(&ReferencePointsRequest { objective_amount: 1,
                divisions, pairs: Vec::new() }).unwrap();
            assert_eq!(report.point_count, 1, "{divisions}");
            assert_eq!(report.points, vec![vec![1.0]], "{divisions}");
        }
    }

    #[test]
    fn the_first_point_is_the_last_corner_for_two_objectives() {
        let report = verify_reference_points(&ReferencePointsRequest { objective_amount: 2,
            divisions: 4, pairs: Vec::new() }).unwrap();
        assert_eq!(report.point_count, 5);
        assert_eq!(report.points[0], vec![0.0, 1.0]);
        assert_eq!(report.points[4], vec![1.0, 0.0]);
        // 逐项递增：首坐标或第二坐标单调上升。 
        // Each step increases either the first or the second coordinate.
        assert!(report.points.windows(2).all(|window|
            window[0][0] < window[1][0] || window[0][1] < window[1][1]));
    }

    #[test]
    fn distances_are_finite_or_explicitly_infinite() {
        let report = verify_reference_points(&ReferencePointsRequest { objective_amount: 2,
            divisions: 4, pairs: vec![
                ReferencePairSpec { objectives: vec![1.0, 1.0], reference: vec![1.0, 0.0] },
                ReferencePairSpec { objectives: vec![1.0, 1.0], reference: vec![0.0, 0.0] },
                ReferencePairSpec { objectives: vec![1.0], reference: vec![1.0, 0.0] },
                ReferencePairSpec { objectives: vec![1.0, 1.0], reference: vec![1.0, 1.0] },
            ] }).unwrap();
        assert_eq!(report.pairs[0].distance, Some(1.0));
        assert!(!report.pairs[0].infinite);
        // 零参考 → 无穷。 / A zero reference is infinite.
        assert!(report.pairs[1].infinite && report.pairs[1].distance.is_none());
        // 目标比参考短 → 无穷。 / A shorter objective vector is infinite.
        assert!(report.pairs[2].infinite);
        // 沿方向本身 → 距离 0。 / Along the direction itself the distance is zero.
        assert_eq!(report.pairs[3].distance, Some(0.0));
    }

    #[test]
    fn the_distance_is_scale_invariant_and_truncating() {
        let report = verify_reference_points(&ReferencePointsRequest { objective_amount: 2,
            divisions: 4, pairs: vec![
                ReferencePairSpec { objectives: vec![1.0, 1.0], reference: vec![1.0, 0.0] },
                ReferencePairSpec { objectives: vec![1.0, 1.0], reference: vec![2.0, 0.0] },
                ReferencePairSpec { objectives: vec![1.0, 900.0], reference: vec![1.0] },
            ] }).unwrap();
        assert_eq!(report.pairs[0].distance, report.pairs[1].distance);
        // 参考只有一维 → 第二维不参与，距离为 0。 / One reference axis means the second objective never counts.
        assert_eq!(report.pairs[2].distance, Some(0.0));
    }

    #[test]
    fn bad_input_is_refused() {
        assert!(verify_reference_points(&ReferencePointsRequest { objective_amount: 9,
            divisions: 4, pairs: Vec::new() }).unwrap_err()
            .message.contains("at most 8 objectives"));
        assert!(verify_reference_points(&ReferencePointsRequest { objective_amount: 3,
            divisions: 17, pairs: Vec::new() }).unwrap_err()
            .message.contains("at most 16 divisions"));
        // 8 个目标 × 16 分割 = C(23,7) = 245157 → 超过 512 点上限。 
        // 8 objectives over 16 divisions give C(23,7) = 245157 points, far past the 512-point cap.
        assert!(verify_reference_points(&ReferencePointsRequest { objective_amount: 8,
            divisions: 16, pairs: Vec::new() }).unwrap_err()
            .message.contains("exceed the 512-point cap"));
        assert!(verify_reference_points(&ReferencePointsRequest { pairs: (0..33)
            .map(|_| ReferencePairSpec { objectives: vec![1.0], reference: vec![1.0] }).collect(),
            ..request() }).unwrap_err().message.contains("at most 32 pairs"));
        assert!(verify_reference_points(&ReferencePointsRequest { pairs: vec![
            ReferencePairSpec { objectives: vec![f64::INFINITY], reference: vec![1.0] }],
            ..request() }).unwrap_err().message.contains("non-finite values"));
        assert!(verify_reference_points(&ReferencePointsRequest { pairs: vec![
            ReferencePairSpec { objectives: vec![1.0; 9], reference: vec![1.0] }],
            ..request() }).unwrap_err().message.contains("at most 8 axes"));
    }

    /// 守门：DTO 必须是 camelCase 且拒绝未知字段。 / A guard: camelCase wire, unknown fields refused.
    #[test]
    fn the_report_wire_is_camel_case_and_rejects_unknown_fields() {
        let report = verify_reference_points(&request()).unwrap();
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"pointCount\""), "{json}");
        assert!(json.contains("\"axisWidth\""), "{json}");
        assert!(json.contains("\"thePointCountFollowsTheSimplexLattice\""), "{json}");
        assert!(!json.contains("point_count"), "{json}");
        let mut copy: serde_json::Value = serde_json::from_str(&json).unwrap();
        copy["unexpected"] = serde_json::json!(1);
        assert!(serde_json::from_str::<ReferencePointsReport>(&copy.to_string()).is_err());
    }
}
