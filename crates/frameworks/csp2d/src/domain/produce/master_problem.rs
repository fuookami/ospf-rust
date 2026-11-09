//! 主问题建模 / Master problem modelling
//!
//! 图案使用量、需求覆盖、物料可用量、产能与目标。这是 csp2d 的**建模边界**：与 bpp2d 的
//! `layer-assignment` 同级，允许出现求解器数值；领域其余部分保持泛型。
//! Pattern usage, demand coverage, material availability, capacity and objectives. This is the **modelling
//! boundary** of csp2d: on the same level as bpp2d's `layer-assignment` it may use solver numbers, while the
//! rest of the domain stays generic.
//!
//! 与一维的关键差异：一维方案是「切片列表」，主问题按切片宽度累加；二维方案是「一块料上的图案」，
//! 主问题按图案使用次数累加，覆盖量来自图案里的件数，消耗量来自图案料域的面积或段高。
//! The key difference from the 1D case: a 1D plan is a slice list, so the master problem accumulates slice
//! widths, while a 2D plan is a pattern on one piece of material, so the master problem accumulates pattern
//! usage counts, with coverage coming from the piece counts inside the pattern and consumption coming from the
//! region area or segment height.

use std::collections::BTreeMap;

use ospf_rust_base::error::Ret;

use crate::domain::material::{CuttingPlan2, DemandMode2, DemandValue2, ProductId2};
use crate::infrastructure::error::validation_error;

#[cfg(test)]
mod cache_tests;

// ============================================================================
// Csp2dMasterDemand - 主问题需求项 / Master problem demand entry
// ============================================================================

/// 主问题需求项 / Master problem demand entry
///
/// `demand` 是必须被**覆盖**的下界（套裁语义：需求必须交齐），`upper_bound` 为可选的业务上限。
/// `demand` is a lower bound that must be **covered** (the demand has to be shipped) and `upper_bound` is an
/// optional business cap.
#[derive(Debug, Clone)]
pub struct Csp2dMasterDemand {
    /// 产品编号 / Product identifier
    pub product_id: ProductId2,
    /// 需求口径 / Demand mode
    pub mode: DemandMode2,
    /// 必须覆盖的需求下界 / Demand lower bound to cover
    pub demand: DemandValue2,
    /// 可选的需求上界 / Optional demand upper bound
    pub upper_bound: Option<DemandValue2>,
}

impl Csp2dMasterDemand {
    /// 创建需求项 / Create a demand entry
    pub fn new(product_id: ProductId2, demand: DemandValue2) -> Self {
        Self {
            product_id,
            mode: demand.mode(),
            demand,
            upper_bound: None,
        }
    }

    /// 设置业务上界 / Set the business upper bound
    pub fn with_upper_bound(mut self, upper_bound: DemandValue2) -> Self {
        self.upper_bound = Some(upper_bound);
        self
    }

    /// 需求下界的求解器数值 / Solver number of the demand lower bound
    pub fn demand_value(&self) -> f64 {
        self.demand.to_solver()
    }
}

// 调用内借用产品编号，不缓存公开可变的方案。 / Borrow product IDs per call; never cache publicly mutable plans.
pub(crate) struct PlanCoverage<'a> {
    products: BTreeMap<&'a str, (u64, Option<f64>)>,
}

impl<'a> PlanCoverage<'a> {
    pub(crate) fn new(
        plan: &'a CuttingPlan2<f64, ospf_rust_quantities::unit::derived::Meter>,
        needs_area: bool,
    ) -> Self {
        let mut products = BTreeMap::new();
        for placement in &plan.pattern.placements {
            let Some(id) = placement.item_id.as_deref() else { continue; };
            let entry = products.entry(id).or_insert_with(|| {
                (0, needs_area.then(|| placement.shape.area().value))
            });
            entry.0 += 1;
        }
        Self { products }
    }

    pub(crate) fn coefficient(&self, demand: &Csp2dMasterDemand) -> f64 {
        self.coefficient_if_present(demand).unwrap_or(0.0)
    }

    pub(crate) fn coefficient_if_present(&self, demand: &Csp2dMasterDemand) -> Option<f64> {
        let (count, area) = self.products.get(demand.product_id.as_str())?;
        Some(match demand.mode {
            DemandMode2::Amount => *count as f64,
            DemandMode2::Area => area.unwrap_or(0.0) * *count as f64,
        })
    }
}

/// 单次求解的需求主序覆盖矩阵。 / Demand-major coverage matrix for one solve.
pub(crate) struct CoverageProjection {
    rows: Vec<Vec<f64>>,
}

impl CoverageProjection {
    pub(crate) fn coefficient(&self, plan_index: usize, demand_index: usize) -> f64 {
        self.rows[demand_index][plan_index]
    }

    pub(crate) fn coverage(&self, usage: &[f64]) -> Vec<f64> {
        self.rows.iter().map(|row| row.iter().enumerate()
            .map(|(index, coefficient)| coefficient * usage.get(index).copied().unwrap_or(0.0))
            .sum()).collect()
    }
}

// 仅缓存本次求解中实际评分的方案，借用阻止期间修改方案。 / Cache only plans scored in this solve; the borrow prevents mutations during its lifetime.
pub(crate) struct PlanWaste<'a> {
    assignment: &'a Csp2dPatternAssignment,
    values: Vec<Option<f64>>,
}

impl<'a> PlanWaste<'a> {
    pub(crate) fn new(assignment: &'a Csp2dPatternAssignment) -> Self {
        Self { assignment, values: vec![None; assignment.len()] }
    }

    pub(crate) fn for_scoring(&mut self, plan_index: usize) -> f64 {
        *self.values[plan_index].get_or_insert_with(|| self.assignment.waste_area(plan_index).max(1e-9))
    }
}

// ============================================================================
// Csp2dPatternAssignment - 图案使用量 / Pattern usage
// ============================================================================

/// 图案使用量变量集合 / The pattern usage variable set
#[derive(Debug, Clone)]
pub struct Csp2dPatternAssignment {
    /// 候选方案 / Candidate plans
    pub plans: Vec<CuttingPlan2<f64, ospf_rust_quantities::unit::derived::Meter>>,
    /// 使用次数上界 / Usage upper bounds
    pub usage_upper_bounds: Vec<u64>,
}

impl Csp2dPatternAssignment {
    /// 创建图案使用量集合 / Create the pattern usage set
    ///
    /// 使用次数上界按「需求件数上界」收紧：单方案重复次数不会超过全部需求件数之和，避免无意义的无限上界。
    /// 上界为零或方案内无件数时取 1，避免把列锁死。
    /// The usage bound is tightened by the demand piece count: a single plan cannot repeat more often than the
    /// total demanded piece count, which avoids a meaningless unbounded range. The bound falls back to 1 when it
    /// is zero or the plan holds no piece, so a zero bound never locks a plan out.
    pub fn new(
        plans: Vec<CuttingPlan2<f64, ospf_rust_quantities::unit::derived::Meter>>,
        demand_upper_bound: u64,
    ) -> Ret<Self> {
        if plans.is_empty() {
            return Err(validation_error(
                "主问题没有候选方案 / The master problem has no candidate plan",
            ));
        }
        let usage_upper_bounds = plans
            .iter()
            .map(|plan| {
                let pieces: u64 = plan.product_amounts().values().sum();
                if pieces == 0 {
                    return 1;
                }
                demand_upper_bound.div_ceil(pieces).max(1)
            })
            .collect();
        Ok(Self {
            plans,
            usage_upper_bounds,
        })
    }

    /// 方案数量 / Plan count
    pub fn len(&self) -> usize {
        self.plans.len()
    }

    /// 是否为空 / Whether empty
    pub fn is_empty(&self) -> bool {
        self.plans.is_empty()
    }

    /// 覆盖系数：方案 `plan_index` 对需求 `demand` 的贡献 / Coverage coefficient of a plan for a demand
    ///
    /// 件数需求取方案内该产品件数；面积需求取「单件面积 × 件数」。
    /// An amount demand takes the plan's piece count of that product; an area demand takes "unit area times
    /// count".
    pub fn coverage_coefficient(&self, plan_index: usize, demand: &Csp2dMasterDemand) -> f64 {
        let Some(plan) = self.plans.get(plan_index) else {
            return 0.0;
        };
        let mut count = 0u64;
        let mut unit_area = None;
        for placement in &plan.pattern.placements {
            if placement.item_id.as_deref() != Some(demand.product_id.as_str()) { continue; }
            count += 1;
            if demand.mode == DemandMode2::Area && unit_area.is_none() {
                unit_area = Some(placement.shape.area().value);
            }
        }
        match demand.mode {
            DemandMode2::Amount => count as f64,
            DemandMode2::Area => unit_area.unwrap_or(0.0) * count as f64,
        }
    }

    pub(crate) fn coverage_projection(&self, demands: &[Csp2dMasterDemand]) -> CoverageProjection {
        let plans = self.plan_coverages(demands);
        CoverageProjection { rows: demands.iter().map(|demand| plans.iter()
            .map(|plan| plan.coefficient(demand)).collect()).collect() }
    }

    fn plan_coverages(&self, demands: &[Csp2dMasterDemand]) -> Vec<PlanCoverage<'_>> {
        let needs_area = demands.iter().any(|demand| demand.mode == DemandMode2::Area);
        self.plans.iter().map(|plan| PlanCoverage::new(plan, needs_area)).collect()
    }

    /// 消耗量：方案 `plan_index` 消耗的料域面积 / Consumption: the region area consumed by a plan
    pub fn consumed_area(&self, plan_index: usize) -> f64 {
        self.plans
            .get(plan_index)
            .map(|plan| plan.consumed_area().value)
            .unwrap_or(0.0)
    }

    /// 消耗量：方案 `plan_index` 消耗的卷材段高 / Consumption: the coil segment height consumed by a plan
    ///
    /// 定尺板材没有段高口径，返回 `None`。
    /// A fixed-size sheet has no segment-height convention and returns `None`.
    pub fn consumed_height(&self, plan_index: usize) -> Option<f64> {
        self.plans
            .get(plan_index)
            .and_then(|plan| plan.consumed_height())
            .map(|height| height.value)
    }

    /// 余料量：方案 `plan_index` 的余料面积 / Waste: the remnant area of a plan
    pub fn waste_area(&self, plan_index: usize) -> f64 {
        self.plans
            .get(plan_index)
            .map(|plan| plan.waste_area().value)
            .unwrap_or(0.0)
    }

    /// 按使用次数累计各需求的覆盖量 / Accumulate coverage per demand using the usage counts
    pub fn coverage(
        &self,
        usage: &[f64],
        demands: &[Csp2dMasterDemand],
    ) -> Vec<f64> {
        if demands.is_empty() { return Vec::new(); }
        let plans = self.plan_coverages(demands);
        demands.iter().map(|demand| plans.iter().enumerate()
            .map(|(index, plan)| plan.coefficient(demand) * usage.get(index).copied().unwrap_or(0.0))
            .sum()).collect()
    }

    /// 需求是否全部被覆盖 / Whether every demand is covered
    ///
    /// 使用次数按四舍五入取整后比较，与最终整数解的口径一致。
    /// Usage counts are rounded before comparison, matching the final integer solution's convention.
    pub fn covers_all(&self, usage: &[f64], demands: &[Csp2dMasterDemand]) -> bool {
        let rounded: Vec<f64> = usage.iter().map(|value| value.round()).collect();
        let coverage = self.coverage(&rounded, demands);
        demands
            .iter()
            .zip(coverage.iter())
            .all(|(demand, covered)| *covered + 1e-6 >= demand.demand_value())
    }

    /// 使用次数向量取整 / Round the usage counts
    pub fn rounded_usage(&self, usage: &[f64]) -> Vec<u64> {
        usage
            .iter()
            .map(|value| value.max(0.0).round() as u64)
            .collect()
    }
}

// ============================================================================
// Csp2dMasterProblem - 主问题 / Master problem
// ============================================================================

/// 主问题 / Master problem
#[derive(Debug, Clone)]
pub struct Csp2dMasterProblem {
    /// 图案使用量 / Pattern usage
    pub assignment: Csp2dPatternAssignment,
    /// 需求项 / Demand entries
    pub demands: Vec<Csp2dMasterDemand>,
    /// 物料可用量：物料编号到可用数量 / Material availability: material identifier to count
    pub material_availability: BTreeMap<String, u64>,
}

impl Csp2dMasterProblem {
    /// 创建主问题 / Create the master problem
    pub fn new(
        assignment: Csp2dPatternAssignment,
        demands: Vec<Csp2dMasterDemand>,
    ) -> Ret<Self> {
        if demands.is_empty() {
            return Err(validation_error(
                "主问题没有需求项 / The master problem has no demand entry",
            ));
        }
        let mut material_availability = BTreeMap::new();
        for plan in &assignment.plans {
            material_availability
                .entry(plan.pattern.material.id.as_str().to_string())
                .or_insert(plan.pattern.material.available_batches);
        }
        Ok(Self {
            assignment,
            demands,
            material_availability,
        })
    }

    /// 物料用量：按使用次数累计每种物料的消耗数量 / Material usage accumulated per material
    ///
    /// 一个方案使用一次即消耗一块/一段物料。
    /// Using a plan once consumes one piece or segment of material.
    pub fn material_usage(&self, usage: &[f64]) -> BTreeMap<String, u64> {
        let mut totals: BTreeMap<String, u64> = BTreeMap::new();
        for (index, plan) in self.assignment.plans.iter().enumerate() {
            let count = usage.get(index).copied().unwrap_or(0.0).max(0.0).round() as u64;
            if count == 0 {
                continue;
            }
            *totals
                .entry(plan.pattern.material.id.as_str().to_string())
                .or_insert(0) += count;
        }
        totals
    }

    /// 物料可用量是否被满足 / Whether the material availability holds
    pub fn material_available(&self, usage: &[f64]) -> bool {
        self.material_usage(usage)
            .iter()
            .all(|(material, used)| {
                self.material_availability
                    .get(material)
                    .map(|available| used <= available)
                    .unwrap_or(true)
            })
    }

    /// 目标项：余料面积加权 / Objective terms: weighted remnant area
    ///
    /// 主目标为余料面积最小（等价于板材面积消耗最小），`weight` 为权重。
    /// The primary objective is minimum remnant area (equivalent to minimum sheet area consumption); `weight`
    /// scales it.
    pub fn waste_objective_terms(&self, weight: f64) -> Vec<(usize, f64)> {
        (0..self.assignment.plans.len())
            .map(|index| (index, self.assignment.waste_area(index) * weight))
            .collect()
    }

    /// 目标项：板材面积消耗加权 / Objective terms: weighted sheet area consumption
    pub fn area_objective_terms(&self, weight: f64) -> Vec<(usize, f64)> {
        (0..self.assignment.plans.len())
            .map(|index| (index, self.assignment.consumed_area(index) * weight))
            .collect()
    }

    /// 目标项：卷材段高消耗加权 / Objective terms: weighted coil segment height consumption
    pub fn height_objective_terms(&self, weight: f64) -> Vec<(usize, f64)> {
        (0..self.assignment.plans.len())
            .filter_map(|index| {
                self.assignment
                    .consumed_height(index)
                    .map(|height| (index, height * weight))
            })
            .collect()
    }

    /// 覆盖率 / Coverage ratio
    ///
    /// 每个需求取 `min(覆盖 / 需求, 1)` 后平均；需求量为零的需求不计入。
    /// Each demand contributes `min(coverage / demand, 1)` and the average is taken; zero-demand entries are
    /// skipped.
    pub fn coverage_ratio(&self, usage: &[f64]) -> f64 {
        let coverage = self.assignment.coverage(usage, &self.demands);
        let mut total = 0.0;
        let mut count = 0usize;
        for (demand, covered) in self.demands.iter().zip(coverage.iter()) {
            let required = demand.demand_value();
            if required <= 0.0 {
                continue;
            }
            total += (covered / required).min(1.0);
            count += 1;
        }
        if count == 0 {
            0.0
        } else {
            total / count as f64
        }
    }
}

// ============================================================================
// 列生成迭代 / Column generation iteration
// ============================================================================

/// 一列生成迭代的记录 / A single column-generation iteration record
#[derive(Debug, Clone)]
pub struct Csp2dIterationRecord {
    /// 迭代序号 / Iteration index
    pub iteration: usize,
    /// 方案数 / Plan count
    pub plan_count: usize,
    /// 新增方案数 / Newly added plan count
    pub added_plan_count: usize,
    /// 覆盖是否完整 / Whether the coverage is complete
    pub covered: bool,
}

/// 由剩余需求构造下一轮生成的产品列表 / Build the next round's product list from the residual demand
///
/// 剩余件数按「需求 − 覆盖」计算并向上取整；为空的条目被丢弃。
/// The residual amount is "demand − coverage" rounded up; empty entries are dropped.
pub fn residual_products(
    assignment: &Csp2dPatternAssignment,
    usage: &[f64],
    demands: &[Csp2dMasterDemand],
    products: &[crate::domain::material::Product2<f64, ospf_rust_quantities::unit::derived::Meter>],
) -> Vec<(
    crate::domain::material::Product2<f64, ospf_rust_quantities::unit::derived::Meter>,
    u64,
)> {
    let coverage = assignment.coverage(usage, demands);
    let mut residuals: BTreeMap<ProductId2, u64> = BTreeMap::new();
    for (demand, covered) in demands.iter().zip(coverage.iter()) {
        let remaining = (demand.demand_value() - covered).max(0.0).ceil() as u64;
        if remaining > 0 {
            residuals.insert(demand.product_id.clone(), remaining);
        }
    }
    products
        .iter()
        .filter_map(|product| {
            residuals
                .get(&product.id)
                .copied()
                .map(|amount| (product.clone(), amount))
        })
        .collect()
}
