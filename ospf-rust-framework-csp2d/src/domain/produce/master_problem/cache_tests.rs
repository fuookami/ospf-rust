use std::hint::black_box;
use std::time::Instant;
use ospf_rust_framework_geometry2d::{PackingShape2, Placement2, QuantityPoint2, rectangle_shape2};
use ospf_rust_quantities::Quantity;
use ospf_rust_quantities::unit::derived::Meter;
use crate::domain::material::{Material2, Pattern2, material_id2, pattern_id2, product_id2};
use crate::infrastructure::{CuttingProcess2, region2_of};
use super::*;

fn fixture(count: usize) -> (Csp2dPatternAssignment, Vec<Csp2dMasterDemand>) {
    let meters = |value| Quantity::<f64, Meter>::new_ct(value);
    let material = Material2::sheet(material_id2("cache"), "sheet", meters(100.0), meters(100.0), 10000);
    let shape = PackingShape2::Rectangle(rectangle_shape2(meters(1.0), meters(1.0)).unwrap());
    let plans = (0..count).map(|index| {
        let placements = (0..64).map(|position| Placement2::new(shape.clone(), QuantityPoint2 {
            x: meters(position as f64), y: meters(0.0),
        }).with_item_id(format!("p{}", (position + index) % 16))).collect();
        CuttingPlan2::from_pattern(Pattern2::new(
            pattern_id2(format!("plan{index}")), material.clone(),
            region2_of(meters(0.0), meters(0.0), meters(100.0), meters(100.0)),
            placements, CuttingProcess2::Freeform,
        ))
    }).collect();
    let demands = (0..16).flat_map(|index| [
        Csp2dMasterDemand::new(product_id2(format!("p{index}")), DemandValue2::Amount(400)),
        Csp2dMasterDemand::new(product_id2(format!("p{index}")), DemandValue2::Area(400.0)),
    ]).collect();
    (Csp2dPatternAssignment::new(plans, 10000).unwrap(), demands)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::{Csp2dHeuristicMasterSolver, Csp2dMasterSolver};

    mod bounded_pool;

    fn reference_usage(master: &Csp2dMasterProblem) -> Vec<f64> {
        let projection = master.assignment.coverage_projection(&master.demands);
        projected_usage(master, &projection, false)
    }

    fn projected_usage(
        master: &Csp2dMasterProblem,
        projection: &CoverageProjection,
        cached: bool,
    ) -> Vec<f64> {
        let mut waste = PlanWaste::new(&master.assignment);
        let mut usage = vec![0.0f64; master.assignment.len()];
        for _ in 0..4096 {
            let coverage = projection.coverage(&usage);
            let mut best: Option<(usize, f64)> = None;
            for (demand_index, demand) in master.demands.iter().enumerate() {
                if coverage[demand_index] + 1e-6 >= demand.demand_value() { continue; }
                for plan_index in 0..master.assignment.len() {
                    let coefficient = projection.coefficient(plan_index, demand_index);
                    if coefficient <= 0.0 { continue; }
                    let denominator = if cached { waste.for_scoring(plan_index) }
                        else { master.assignment.waste_area(plan_index).max(1e-9) };
                    let score = coefficient / denominator;
                    best = match best {
                        Some((_, best_score)) if best_score >= score => best,
                        _ => Some((plan_index, score)),
                    };
                }
            }
            let Some((index, _)) = best else { break; };
            let upper = master.assignment.usage_upper_bounds.get(index).copied().unwrap_or(u64::MAX) as f64;
            if usage[index] + 1.0 > upper { break; }
            usage[index] += 1.0;
        }
        usage
    }

    fn parallel_usage(master: &Csp2dMasterProblem, workers: usize) -> Vec<f64> {
        let needs_area = master.demands.iter().any(|demand| demand.mode == DemandMode2::Area);
        let chunk = master.assignment.len().div_ceil(workers);
        let plans: Vec<_> = std::thread::scope(|scope| {
            let handles: Vec<_> = master.assignment.plans.chunks(chunk).map(|part| scope.spawn(move || {
                part.iter().map(|plan| PlanCoverage::new(plan, needs_area)).collect::<Vec<_>>()
            })).collect();
            handles.into_iter().flat_map(|handle| handle.join().unwrap()).collect()
        });
        let rows = master.demands.iter().map(|demand| plans.iter()
            .map(|plan| plan.coefficient(demand)).collect()).collect();
        drop(plans);
        projected_usage(master, &CoverageProjection { rows }, true)
    }

    #[test]
    #[ignore = "手动 release 完整求解并行原型测量 / Manual release full-solve parallel prototype measurement"]
    fn performance_cache_full_solve_parallel() {
        for count in [32, 256] {
            let (assignment, demands) = fixture(count);
            let master = Csp2dMasterProblem::new(assignment, demands).unwrap();
            let expected = Csp2dHeuristicMasterSolver::new().solve_master(&master).unwrap().usage;
            for outer in [1, 4] {
                for workers in [1, 2, 4] {
                    for sample in 0..5 {
                        let start = Instant::now();
                        std::thread::scope(|scope| {
                            let master = &master;
                            let expected = &expected;
                            for _ in 0..outer {
                                scope.spawn(move || {
                                    let actual = if workers == 1 {
                                        Csp2dHeuristicMasterSolver::new().solve_master(black_box(master)).unwrap().usage
                                    } else { parallel_usage(black_box(master), workers) };
                                    assert_eq!(&actual, expected);
                                    black_box(actual);
                                });
                            }
                        });
                        println!("full_solve plans={count} outer={outer} workers={workers} sample={sample} elapsed_ms={:.6}", start.elapsed().as_secs_f64() * 1000.0);
                    }
                }
            }
        }
    }

    #[test]
    fn scoring_waste_is_lazy_and_scoped_to_the_assignment_borrow() {
        let (mut assignment, _) = fixture(3);
        let before = assignment.waste_area(0).max(1e-9);
        {
            let mut waste = PlanWaste::new(&assignment);
            assert!(waste.values.iter().all(Option::is_none));
            assert_eq!(waste.for_scoring(0).to_bits(), before.to_bits());
            // 已物化项即使替换为哨兵值，也不会重算；未访问项不物化。 / A sentinel proves materialized entries are not recomputed; unused entries stay lazy.
            waste.values[0] = Some(123.0);
            assert_eq!(waste.for_scoring(0), 123.0);
            assert!(waste.values[1..].iter().all(Option::is_none));
        }
        assignment.plans[0].pattern.placements.clear();
        let expected = assignment.waste_area(0).max(1e-9);
        assert_ne!(before, expected);
        assert_eq!(PlanWaste::new(&assignment).for_scoring(0).to_bits(), expected.to_bits());
    }

    #[test]
    fn scoring_cache_preserves_reference_usage_and_plan_mutations() {
        let (assignment, mut demands) = fixture(3);
        for demand in &mut demands { demand.demand = DemandValue2::Amount(8); demand.mode = DemandMode2::Amount; }
        let mut master = Csp2dMasterProblem::new(assignment, demands).unwrap();
        for variant in 0..6 {
            match variant {
                1 => master.assignment.usage_upper_bounds[0] = 0,
                2 => master.assignment.plans.reverse(),
                3 => master.assignment.plans[0].pattern.placements.clear(),
                4 => master.demands.push(Csp2dMasterDemand::new(product_id2("missing"), DemandValue2::Amount(2))),
                5 => master.demands.clear(),
                _ => {},
            }
            assert_eq!(Csp2dHeuristicMasterSolver::new().solve_master(&master).unwrap().usage, reference_usage(&master));
        }
    }

    #[test]
    fn scoring_cache_preserves_nonfinite_and_clamped_values() {
        for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -0.0, 0.0, 1e-12] {
            let (mut assignment, mut demands) = fixture(1);
            assignment.plans[0].pattern.region.width = Quantity::<f64, Meter>::new_ct(value);
            let expected = assignment.waste_area(0).max(1e-9);
            let mut waste = PlanWaste::new(&assignment);
            for _ in 0..2 { assert_eq!(waste.for_scoring(0).to_bits(), expected.to_bits()); }
            for demand in &mut demands { demand.demand = DemandValue2::Amount(4); demand.mode = DemandMode2::Amount; }
            let master = Csp2dMasterProblem::new(assignment, demands).unwrap();
            assert_eq!(Csp2dHeuristicMasterSolver::new().solve_master(&master).unwrap().usage, reference_usage(&master));
        }
    }

    #[test]
    #[ignore = "手动 release 分段测量 / Manual release phase measurement"]
    fn performance_cache_projection_phases() {
        for count in [32, 256] {
            let (assignment, demands) = fixture(count);
            for sample in 0..5 {
                let start = Instant::now();
                let plans = assignment.plan_coverages(&demands);
                let plan_ms = start.elapsed().as_secs_f64() * 1000.0;
                let start = Instant::now();
                let rows = demands.iter().map(|demand| plans.iter()
                    .map(|plan| plan.coefficient(demand)).collect()).collect();
                let projection = CoverageProjection { rows };
                let fill_ms = start.elapsed().as_secs_f64() * 1000.0;
                let usage = vec![1.0; count];
                let start = Instant::now();
                for _ in 0..100 { black_box(projection.coverage(black_box(&usage))); }
                let read_ms = start.elapsed().as_secs_f64() * 1000.0;
                println!("projection plans={count} demands={} sample={sample} plan_ms={plan_ms:.6} fill_ms={fill_ms:.6} read100_ms={read_ms:.6} matrix_bytes={}", demands.len(), count * demands.len() * 8);
                for workers in [2, 4] {
                    let start = Instant::now();
                    let chunk = assignment.plans.len().div_ceil(workers);
                    let parallel: Vec<_> = std::thread::scope(|scope| {
                        let handles: Vec<_> = assignment.plans.chunks(chunk).map(|part| scope.spawn(move || {
                            part.iter().map(|plan| PlanCoverage::new(plan, true)).collect::<Vec<_>>()
                        })).collect();
                        handles.into_iter().flat_map(|handle| handle.join().unwrap()).collect()
                    });
                    let elapsed = start.elapsed().as_secs_f64() * 1000.0;
                    assert_eq!(parallel.len(), plans.len());
                    for (left, right) in parallel.iter().zip(&plans) {
                        for demand in &demands { assert_eq!(left.coefficient(demand).to_bits(), right.coefficient(demand).to_bits()); }
                    }
                    println!("projection_parallel plans={count} sample={sample} workers={workers} plan_ms={elapsed:.6}");
                }
            }
        }
    }
}
