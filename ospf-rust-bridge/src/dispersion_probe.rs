//! 续挖第 82 单：**非支配排序与拥挤距离**——分层只按输入顺序、重复点同层、方向表截断会**重排分层**，
//! 而拥挤距离的边界解在**常量轴上也是无穷**、且只按**第一个点**的维度数走
//! （`solver/heuristic/operator/multi_objective/non_dominated_sort.rs` 的 `FastNonDominatedSort` /
//! `CrowdingDistance`）
//! Continuation, delivery 82: **non-dominated sorting and crowding distance** — the layering keeps the input order,
//! duplicate points share a front, a truncated direction list **re-layers**, while the crowding boundary is infinite **even
//! on a constant axis** and the axis count comes from the **first point**
//! (`FastNonDominatedSort` / `CrowdingDistance` in
//! `solver/heuristic/operator/multi_objective/non_dominated_sort.rs`).
//!
//! 第 81 单量的是支配关系本身；这一单量**它上面那两层 NSGA-II 算子**。八条实测：
//!
//! 1. **分层是一个划分**（`the_fronts_partition_the_candidates()`）：所有候选恰好出现一次，不丢不重。
//! 2. **层内保持输入顺序**（`a_front_keeps_the_input_order()`）：`current` 按升序索引建，**不按"谁更优"排**。
//! 3. **第 0 层就是 Pareto 前沿**（`front_zero_is_the_pareto_front()`）：层内互不支配，且层外必被层内支配。
//! 4. **重复点同层**（`duplicates_share_the_first_front()`）：完全相同的点互不支配，于是都留在最前层。
//! 5. **方向表截断会重排分层**（`a_truncated_direction_list_regroups_the_fronts()`）：第 81 单那条
//!    "方向表短了只比前几维"在这里变成**分层结果不同**。
//! 6. **空种群没有层**（`an_empty_population_has_no_front()`）：`size == 0` 返回空表。
//! 7. **常量轴上边界解照样无穷**（`the_crowding_boundary_is_infinite_even_on_a_constant_axis()`）：
//!    边界赋值在 `span <= 0` 的 `continue` **之前**，所以整列相等时首尾仍是 `inf`。本单最值钱的一条。
//! 8. **拥挤距离只按第一个点的维度数走**（`crowding_reads_the_first_point_for_the_axis_count()`）：
//!    `for (index, _) in objectives[0].iter().enumerate()`——后续点多出来的维度被忽略；
//!    反过来（第一个点更短）会**越界 panic**，所以边界直接拒绝参差输入。
//!
//! Delivery 81 measured the dominance relation itself; this one measures **the two NSGA-II operators built on it**. **The
//! layering is a partition** — every candidate appears exactly once. **A front keeps the input order** — `current` is built
//! from ascending indices and **never sorted by quality**. **Front 0 is the Pareto front.** **Duplicate points share the
//! first front** because identical points never dominate each other. **A truncated direction list re-layers the population**
//! — delivery 81's truncation becomes a different layering here. **An empty population has no front.** **The crowding
//! boundary is infinite even on a constant axis** — the boundary assignment happens **before** the `span <= 0` `continue`,
//! so the first and last entries stay `inf` when a whole column is equal. This is the most valuable finding here. And
//! **crowding reads the first point for the axis count**, ignoring any extra axes on later points — while the mirror case
//! (a shorter first point) would panic out of bounds, which is why the boundary refuses ragged input.

use ospf_rust_core::solver::heuristic::operator::multi_objective::dominance::{
    Dominance, ParetoDominance,
};
use ospf_rust_core::solver::heuristic::operator::multi_objective::non_dominated_sort::{
    CrowdingDistance, FastNonDominatedSort,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 一个拥挤距离读数（非有限值只能以 `infinite` 表达，JSON 载不动 `inf`）。
/// One crowding reading; a non-finite value can only be expressed as `infinite` since JSON cannot carry `inf`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CrowdingPointView {
    /// 该点在输入里的位置。 / This point's position in the input.
    pub position: u32,
    /// 距离是否为无穷（边界解）。 / Whether the distance is infinite (a boundary solution).
    pub infinite: bool,
    /// 有限距离；`None` 表示无穷。 / The finite distance; `None` means infinite.
    #[serde(default)]
    pub distance: Option<f64>,
}

/// 一层前沿。 / One front.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DispersionFrontView {
    /// 层内成员（输入位置，按升序）。 / The front's members as input positions, ascending.
    pub members: Vec<u32>,
    /// 层内每个成员的拥挤距离（在该层自己的目标向量上算）。 / Crowding distances computed on this front's own vectors.
    pub crowding: Vec<CrowdingPointView>,
}

/// 非支配排序与拥挤距离探针请求。 / The non-dominated-sort and crowding probe request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DispersionRequest {
    /// 构造方式：`directions` 或 `minimization`。 / The construction mode.
    pub mode: String,
    /// 逐目标方向，`true` 表示最小化。 / Per-objective direction, `true` means minimisation.
    #[serde(default)]
    pub directions: Vec<bool>,
    /// `minimization` 模式的目标个数。 / The objective amount for `minimization`.
    #[serde(default)]
    pub objective_amount: Option<u32>,
    /// 候选点的目标向量（必须同维、有限）。 / Each candidate's objective vector (same length, finite).
    #[serde(default)]
    pub points: Vec<Vec<f64>>,
}

/// 非支配排序与拥挤距离探针结论。 / The verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DispersionReport {
    /// 构造方式回显。 / The construction mode echoed back.
    pub mode: String,
    /// 实际参与比较的目标维度数。 / How many axes actually take part.
    pub compared_axes: u32,
    /// 层数。 / The front count.
    pub front_count: u32,
    /// 所有层成员的总数（应等于候选数）。 / The total member count, which should equal the candidate count.
    pub total_members: u32,
    /// 每一层的成员与拥挤距离。 / Each front's members and crowding distances.
    pub fronts: Vec<DispersionFrontView>,
    /// 全部分层里的拥挤距离是否都是无穷。 / Whether every crowding distance is infinite.
    pub all_crowding_infinite: bool,
    /// 八条规则属性的读数。 / The eight rule readings.
    pub the_fronts_partition_the_candidates: bool,
    pub a_front_keeps_the_input_order: bool,
    pub front_zero_is_the_pareto_front: bool,
    pub duplicates_share_the_first_front: bool,
    pub a_truncated_direction_list_regroups_the_fronts: bool,
    pub an_empty_population_has_no_front: bool,
    pub the_crowding_boundary_is_infinite_even_on_a_constant_axis: bool,
    pub crowding_reads_the_first_point_for_the_axis_count: bool,
}

/// 把拥挤距离转成读数。 / Turn crowding distances into readings.
fn crowding_views(members: &[u32], objectives: &[Vec<f64>]) -> Vec<CrowdingPointView> {
    CrowdingDistance::assign(objectives).into_iter().zip(members.iter().copied())
        .map(|(distance, position)| CrowdingPointView {
            position,
            infinite: !distance.is_finite(),
            distance: distance.is_finite().then_some(distance),
        }).collect()
}

/// 校验请求里的点集。 / Validate the point set in the request.
fn validate_points(request: &DispersionRequest) -> Result<Vec<Vec<f64>>> {
    if request.points.len() > 64 {
        return Err(BridgeError::new("InvalidInput",
            "dispersion failed validation: at most 64 candidates are accepted"));
    }
    let axes = request.points.first().map(|point| point.len()).unwrap_or(0);
    if axes > 64 {
        return Err(BridgeError::new("InvalidInput",
            "dispersion failed validation: at most 64 axes are accepted"));
    }
    for point in &request.points {
        if point.len() != axes {
            return Err(BridgeError::new("InvalidInput",
                "dispersion failed validation: every candidate needs the same axis count"));
        }
        if point.iter().any(|value| !value.is_finite()) {
            return Err(BridgeError::new("InvalidInput",
                "dispersion failed validation: non-finite objective values cannot cross the boundary"));
        }
    }
    Ok(request.points.clone())
}

/// 走一遍分层与拥挤距离。 / Layer the population and compute crowding distances.
fn layer(request: &DispersionRequest, points: &[Vec<f64>])
    -> Result<(u32, Vec<DispersionFrontView>)>
{
    let (relation, axes) = match request.mode.as_str() {
        "directions" => (ParetoDominance::new(request.directions.clone()),
            u32::try_from(request.directions.len()).unwrap_or(u32::MAX)),
        "minimization" => {
            let amount = usize::try_from(request.objective_amount.unwrap_or(0))
                .map_err(|_| BridgeError::new("InvalidInput",
                    "dispersion failed validation: objective amount overflow"))?;
            (ParetoDominance::minimization(amount), u32::try_from(amount).unwrap_or(u32::MAX))
        }
        other => return Err(BridgeError::new("InvalidInput", &format!(
            "dispersion failed validation: unknown mode `{other}`"))),
    };
    let fronts = FastNonDominatedSort::sort(points, |point| point.clone(), &relation);
    // `sort` 返回克隆后的点而不是下标，成员位置要回查原始输入（重复点取第一个未占用者）。
    // `sort` returns cloned points rather than indices, so member positions are recovered from the original input,
    // taking the first still-unused occurrence for duplicates.
    let mut used = vec![false; points.len()];
    let views = fronts.iter().map(|front| {
        let members = front.iter().filter_map(|point| {
            let index = points.iter().enumerate()
                .find(|(index, candidate)| !used[*index] && *candidate == point)
                .map(|(index, _)| index)?;
            used[index] = true;
            u32::try_from(index).ok()
        }).collect::<Vec<_>>();
        DispersionFrontView { crowding: crowding_views(&members, front), members }
    }).collect::<Vec<_>>();
    Ok((axes, views))
}

/// 走一遍探针。非法输入是**错误**。 / Walk the probe.
pub fn verify_dispersion(request: &DispersionRequest) -> Result<DispersionReport> {
    if request.directions.len() > 64 {
        return Err(BridgeError::new("InvalidInput",
            "dispersion failed validation: at most 64 directions are accepted"));
    }
    let findings = canonical_findings();
    let points = validate_points(request)?;
    let (compared_axes, fronts) = layer(request, &points)?;
    let all_crowding_infinite = fronts.iter()
        .flat_map(|front| front.crowding.iter())
        .all(|reading| reading.infinite);
    Ok(DispersionReport {
        mode: request.mode.clone(),
        compared_axes,
        front_count: u32::try_from(fronts.len()).unwrap_or(u32::MAX),
        total_members: u32::try_from(fronts.iter().map(|front| front.members.len()).sum::<usize>())
            .unwrap_or(u32::MAX),
        fronts,
        all_crowding_infinite,
        the_fronts_partition_the_candidates: findings.the_fronts_partition_the_candidates,
        a_front_keeps_the_input_order: findings.a_front_keeps_the_input_order,
        front_zero_is_the_pareto_front: findings.front_zero_is_the_pareto_front,
        duplicates_share_the_first_front: findings.duplicates_share_the_first_front,
        a_truncated_direction_list_regroups_the_fronts:
            findings.a_truncated_direction_list_regroups_the_fronts,
        an_empty_population_has_no_front: findings.an_empty_population_has_no_front,
        the_crowding_boundary_is_infinite_even_on_a_constant_axis:
            findings.the_crowding_boundary_is_infinite_even_on_a_constant_axis,
        crowding_reads_the_first_point_for_the_axis_count:
            findings.crowding_reads_the_first_point_for_the_axis_count,
    })
}

/// 八条规则属性的内部固定探针。 / The eight rule properties measured by fixed internal probes.
struct Findings {
    the_fronts_partition_the_candidates: bool,
    a_front_keeps_the_input_order: bool,
    front_zero_is_the_pareto_front: bool,
    duplicates_share_the_first_front: bool,
    a_truncated_direction_list_regroups_the_fronts: bool,
    an_empty_population_has_no_front: bool,
    the_crowding_boundary_is_infinite_even_on_a_constant_axis: bool,
    crowding_reads_the_first_point_for_the_axis_count: bool,
}

/// 用固定探针测出全部规则属性——不读调用方的请求值。
/// Measure every rule property with fixed probes; the caller's request values are not read.
fn canonical_findings() -> Findings {
    let relation = ParetoDominance::minimization(2);
    let population: Vec<Vec<f64>> = vec![
        vec![1.0, 4.0], vec![2.0, 3.0], vec![3.0, 2.0], vec![4.0, 1.0], vec![5.0, 5.0],
        vec![2.0, 3.0]];
    let layered = FastNonDominatedSort::sort(&population, |point| point.clone(), &relation);
    // 1) 分层是一个划分。 / The layering is a partition.
    let the_fronts_partition_the_candidates = {
        // 重复点必须用"第一个未占用者"回查，`position()` 会把两个重复点都算到第一个下标上。 
        // Duplicates must be resolved to the first still-unused occurrence; `position()` would charge both copies to the
        // first index.
        let mut used = vec![false; population.len()];
        let mut placed = 0_usize;
        for front in &layered {
            for point in front {
                if let Some(index) = population.iter().enumerate()
                    .find(|(index, candidate)| !used[*index] && *candidate == point)
                    .map(|(index, _)| index)
                {
                    used[index] = true;
                    placed += 1;
                }
            }
        }
        // 每个候选恰好出现一次。 / Every candidate appears exactly once.
        used.iter().all(|flag| *flag) && placed == population.len()
            && layered.iter().map(|front| front.len()).sum::<usize>() == population.len()
    };
    // 2) 层内保持输入顺序。 / A front keeps the input order.
    let a_front_keeps_the_input_order = {
        // 最差的点排在输入最前面：它被后两个支配，于是落到后一层。 
        // The worst point sits first in the input; it is dominated by the other two and therefore lands in a later front.
        let ordered = vec![vec![9.0, 9.0], vec![1.0, 4.0], vec![2.0, 3.0]];
        let fronts = FastNonDominatedSort::sort(&ordered, |point| point.clone(), &relation);
        // 第 0 层按**输入顺序**给 `[1,4]` 再 `[2,3]`（位置 1、2），而不是按质量重排。 
        // Front 0 lists `[1,4]` then `[2,3]` (positions 1 and 2), following the input order rather than the quality order.
        fronts.len() == 2 && fronts[0] == vec![vec![1.0, 4.0], vec![2.0, 3.0]]
            && fronts[1] == vec![vec![9.0, 9.0]]
    };
    // 3) 第 0 层就是 Pareto 前沿。 / Front 0 is the Pareto front.
    let front_zero_is_the_pareto_front = {
        let first = &layered[0];
        let within = first.iter().all(|left| first.iter().all(|right| {
            !relation.dominates(left, right)
        }));
        let rest = layered[1..].iter().flatten()
            .all(|point| population.iter().any(|other| relation.dominates(other, point)));
        within && rest && layered.len() == 2
    };
    // 4) 重复点同层。 / Duplicate points share the first front.
    let duplicates_share_the_first_front = {
        let duplicated = vec![vec![1.0, 1.0], vec![1.0, 1.0], vec![2.0, 2.0]];
        let fronts = FastNonDominatedSort::sort(&duplicated, |point| point.clone(), &relation);
        // 两个完全相同的点互不支配 → 都在第 0 层。 / Two identical points never dominate each other, so both are in front 0.
        fronts.len() == 2 && fronts[0].len() == 2
            && fronts[0][0] == vec![1.0, 1.0] && fronts[0][1] == vec![1.0, 1.0]
    };
    // 5) 方向表截断会重排分层。 / A truncated direction list re-layers the population.
    let a_truncated_direction_list_regroups_the_fronts = {
        // 两个点在第二维互不可比，但第一维有大小关系。 
        // Two points are incomparable on axis 1 yet ordered on axis 0.
        let probe = vec![vec![1.0, 10.0], vec![2.0, 0.0]];
        let both = FastNonDominatedSort::sort(&probe, |point| point.clone(),
            &ParetoDominance::minimization(2));
        let truncated = FastNonDominatedSort::sort(&probe, |point| point.clone(),
            &ParetoDominance::minimization(1));
        // 两维：同一层；一维：分成两层。 / Two axes put them in one front; one axis splits them.
        both.len() == 1 && truncated.len() == 2
    };
    // 6) 空种群没有层。 / An empty population has no front.
    let an_empty_population_has_no_front = {
        let empty: Vec<Vec<f64>> = Vec::new();
        let fronts = FastNonDominatedSort::sort(&empty, |point| point.clone(), &relation);
        let single = FastNonDominatedSort::sort(&vec![vec![0.0, 0.0]], |point| point.clone(),
            &relation);
        fronts.is_empty() && single.len() == 1 && single[0].len() == 1
    };
    // 7) 常量轴上边界解照样无穷。 / The crowding boundary is infinite even on a constant axis.
    let the_crowding_boundary_is_infinite_even_on_a_constant_axis = {
        // 四个点、第二维**整列相等**。常量列的 `sort_by` 全等 → 顺序仍是输入顺序（稳定排序），
        // 于是 `inf` 写到下标 0 与 3；而下标 0 在两条变化轴上都处于中间。把常量轴去掉，它立刻变有限。
        // Four points with an **equal second column**. The constant column's `sort_by` keeps the input order (stable),
        // so `inf` lands on indices 0 and 3, while index 0 is interior on both varying axes. Drop the constant axis and it
        // becomes finite at once.
        let with_constant = CrowdingDistance::assign(&[
            vec![2.0, 5.0, 2.0], vec![1.0, 5.0, 0.0], vec![3.0, 5.0, 3.0],
            vec![0.0, 5.0, 1.0]]);
        let without_constant = CrowdingDistance::assign(&[
            vec![2.0, 2.0], vec![1.0, 0.0], vec![3.0, 3.0], vec![0.0, 1.0]]);
        // 常量轴把下标 0 抬成无穷；去掉它就恢复有限。 
        // The constant axis lifts index 0 to infinity, and removing it restores a finite value.
        with_constant[0].is_infinite() && without_constant[0].is_finite()
            // `span <= 0` 只是提前 `continue`，已经写下去的 `inf` 不会被撤回。 
            // `span <= 0` merely continues early and never retracts the `inf` already written.
            && without_constant[3].is_infinite()
    };
    // 8) 拥挤距离只按第一个点的维度数走。 / Crowding reads the first point for the axis count.
    let crowding_reads_the_first_point_for_the_axis_count = {
        // 第一个点两维、后续点三维：第三维被忽略，结果与两维版本逐项相同。 
        // The first point has two axes while later ones have three: the third is ignored and the result matches the
        // two-axis version entry by entry.
        let ragged_trailing = [vec![0.0, 2.0], vec![1.0, 1.0, 999.0], vec![2.0, 0.0, -999.0]];
        let two_axes = [vec![0.0, 2.0], vec![1.0, 1.0], vec![2.0, 0.0]];
        let from_trailing = CrowdingDistance::assign(&ragged_trailing);
        let from_two = CrowdingDistance::assign(&two_axes);
        from_trailing.len() == from_two.len()
            && from_trailing.iter().zip(from_two.iter()).all(|(left, right)|
                left.total_cmp(right) == std::cmp::Ordering::Equal)
            // 而镜像 Case（第一个点更短）会越界——所以边界直接拒绝参差输入。 
            // The mirror case (a shorter first point) would index out of bounds, which is why the boundary refuses
            // ragged input.
            && validate_points(&DispersionRequest { mode: "directions".to_owned(),
                directions: vec![true, true], objective_amount: None,
                points: vec![vec![0.0], vec![1.0, 2.0]] }).is_err()
    };
    Findings {
        the_fronts_partition_the_candidates,
        a_front_keeps_the_input_order,
        front_zero_is_the_pareto_front,
        duplicates_share_the_first_front,
        a_truncated_direction_list_regroups_the_fronts,
        an_empty_population_has_no_front,
        the_crowding_boundary_is_infinite_even_on_a_constant_axis,
        crowding_reads_the_first_point_for_the_axis_count,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> DispersionRequest {
        DispersionRequest { mode: "directions".to_owned(), directions: vec![true, true],
            objective_amount: None, points: Vec::new() }
    }

    #[test]
    fn an_empty_population_has_no_front() {
        let report = verify_dispersion(&request()).unwrap();
        assert_eq!(report.front_count, 0);
        assert_eq!(report.total_members, 0);
        assert!(report.fronts.is_empty());
        assert_eq!(report.compared_axes, 2);
        assert!(report.the_fronts_partition_the_candidates);
        assert!(report.a_front_keeps_the_input_order);
        assert!(report.front_zero_is_the_pareto_front);
        assert!(report.duplicates_share_the_first_front);
        assert!(report.a_truncated_direction_list_regroups_the_fronts);
        assert!(report.an_empty_population_has_no_front);
        assert!(report.the_crowding_boundary_is_infinite_even_on_a_constant_axis);
        assert!(report.crowding_reads_the_first_point_for_the_axis_count);
    }

    #[test]
    fn the_two_objective_population_splits_into_two_fronts() {
        let report = verify_dispersion(&DispersionRequest {
            points: vec![vec![1.0, 4.0], vec![2.0, 3.0], vec![3.0, 2.0], vec![4.0, 1.0],
                vec![5.0, 5.0]], ..request() }).unwrap();
        assert_eq!(report.front_count, 2);
        assert_eq!(report.total_members, 5);
        assert_eq!(report.fronts[0].members, vec![0, 1, 2, 3]);
        assert_eq!(report.fronts[1].members, vec![4]);
        // 第 0 层拥挤距离：两个边界无穷、中间有限。 
        // Front 0's crowding: two infinite boundaries and finite middles.
        assert!(report.fronts[0].crowding[0].infinite);
        assert!(report.fronts[0].crowding[3].infinite);
        assert!(!report.fronts[0].crowding[1].infinite);
        assert!(report.fronts[0].crowding[1].distance.is_some());
        // 单点层：唯一成员是边界 → 无穷。 / A one-member front: the only member is a boundary.
        assert!(report.fronts[1].crowding[0].infinite);
        assert!(!report.all_crowding_infinite);
    }

    #[test]
    fn duplicate_points_share_the_first_front() {
        let report = verify_dispersion(&DispersionRequest {
            points: vec![vec![1.0, 1.0], vec![1.0, 1.0], vec![2.0, 2.0]], ..request() }).unwrap();
        assert_eq!(report.front_count, 2);
        assert_eq!(report.fronts[0].members, vec![0, 1]);
        assert_eq!(report.fronts[1].members, vec![2]);
    }

    #[test]
    fn a_truncated_direction_list_splits_the_front() {
        // 两维：同一层；一维：两层——截断会重排分层。 
        // Two axes put both in one front; one axis splits them into two.
        let both = verify_dispersion(&DispersionRequest {
            points: vec![vec![1.0, 10.0], vec![2.0, 0.0]], ..request() }).unwrap();
        assert_eq!(both.front_count, 1);
        assert_eq!(both.fronts[0].members, vec![0, 1]);
        let truncated = verify_dispersion(&DispersionRequest { directions: vec![true],
            points: vec![vec![1.0, 10.0], vec![2.0, 0.0]], ..request() }).unwrap();
        assert_eq!(truncated.front_count, 2);
        assert_eq!(truncated.compared_axes, 1);
    }

    #[test]
    fn crowding_distances_are_finite_only_for_interior_points() {
        // 三点单调排列：首尾无穷、中间有限。 / Three monotone points: the ends are infinite, the middle is finite.
        let report = verify_dispersion(&DispersionRequest {
            directions: vec![true, true], points: vec![vec![0.0, 2.0], vec![1.0, 1.0],
                vec![2.0, 0.0]], ..request() }).unwrap();
        assert_eq!(report.front_count, 1);
        let crowding = &report.fronts[0].crowding;
        assert!(crowding[0].infinite && crowding[2].infinite);
        assert!(!crowding[1].infinite);
        // 每一维的间隔都是 2/2 = 1 → 中间点距离 2.0。 
        // Each axis contributes 2/2 = 1, so the middle distance is 2.0.
        assert_eq!(crowding[1].distance, Some(2.0));
    }

    #[test]
    fn a_population_of_at_most_two_is_all_infinite() {
        // 两个点在两维上互有支配关系 → 各成一层；而每层只有一两个成员时 `assign` 直接全给无穷。 
        // Two points where one dominates the other form two fronts, and `assign` returns all-infinite whenever a front
        // has at most two members.
        let report = verify_dispersion(&DispersionRequest {
            points: vec![vec![0.0, 0.0], vec![1.0, 1.0]], ..request() }).unwrap();
        assert_eq!(report.front_count, 2);
        assert!(report.all_crowding_infinite);
        assert!(report.fronts[0].crowding.iter().all(|reading| reading.distance.is_none()));
    }

    #[test]
    fn ragged_or_non_finite_points_are_refused() {
        assert!(verify_dispersion(&DispersionRequest {
            points: vec![vec![0.0], vec![1.0, 2.0]], ..request() }).unwrap_err()
            .message.contains("same axis count"));
        assert!(verify_dispersion(&DispersionRequest {
            points: vec![vec![f64::INFINITY]], ..request() }).unwrap_err()
            .message.contains("non-finite objective values"));
        assert!(verify_dispersion(&DispersionRequest {
            points: vec![vec![f64::NAN]], ..request() }).unwrap_err()
            .message.contains("non-finite objective values"));
        assert!(verify_dispersion(&DispersionRequest { mode: "nope".to_owned(), ..request() })
            .unwrap_err().message.contains("unknown mode"));
        assert!(verify_dispersion(&DispersionRequest { points: (0..65)
            .map(|_| vec![0.0]).collect(), ..request() }).unwrap_err()
            .message.contains("at most 64 candidates"));
        assert!(verify_dispersion(&DispersionRequest { points: vec![vec![0.0; 65]],
            ..request() }).unwrap_err().message.contains("at most 64 axes"));
        assert!(verify_dispersion(&DispersionRequest { directions: vec![true; 65],
            ..request() }).unwrap_err().message.contains("at most 64 directions"));
    }

    #[test]
    fn minimization_mode_matches_an_all_minimising_direction_list() {
        let points = vec![vec![1.0, 4.0], vec![2.0, 3.0], vec![5.0, 5.0]];
        let by_minimization = verify_dispersion(&DispersionRequest {
            mode: "minimization".to_owned(), directions: Vec::new(), objective_amount: Some(2),
            points: points.clone() }).unwrap();
        let by_directions = verify_dispersion(&DispersionRequest { points, ..request() }).unwrap();
        assert_eq!(by_minimization.fronts, by_directions.fronts);
        assert_eq!(by_minimization.compared_axes, 2);
        assert_eq!(by_minimization.mode, "minimization");
    }

    /// 守门：DTO 必须是 camelCase 且拒绝未知字段。 / A guard: camelCase wire, unknown fields refused.
    #[test]
    fn the_report_wire_is_camel_case_and_rejects_unknown_fields() {
        let report = verify_dispersion(&request()).unwrap();
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"frontCount\""), "{json}");
        assert!(json.contains("\"allCrowdingInfinite\""), "{json}");
        assert!(json.contains("\"theCrowdingBoundaryIsInfiniteEvenOnAConstantAxis\""), "{json}");
        assert!(!json.contains("front_count"), "{json}");
        let mut copy: serde_json::Value = serde_json::from_str(&json).unwrap();
        copy["unexpected"] = serde_json::json!(1);
        assert!(serde_json::from_str::<DispersionReport>(&copy.to_string()).is_err());
    }
}
