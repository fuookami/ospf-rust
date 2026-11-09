use std::sync::Mutex;
use ospf_rust_core::model::intermediate::LinearTriadModel;
use ospf_rust_core::solver::{SolverCapability, SolverInfo, SolverOutput};
use ospf_rust_framework_network_scheduling::domain::vrp::{
    DistanceArcCostCalculator, DistanceAsTravelTimeCalculator, EuclideanDistanceCalculator,
};
use super::*;
use crate::framework::demo5::infrastructure::{first25_instance, small_instance};

#[derive(Default)]
struct RecordingSolver(Mutex<Option<LinearTriadModel>>);

impl SolverInfo for RecordingSolver {
    fn name(&self) -> &str { "recording" }
    fn capabilities(&self) -> Vec<SolverCapability> { Vec::new() }
}

impl LinearSolver for RecordingSolver {
    fn solve_linear(&self, model: &LinearTriadModel) -> ospf_rust_core::error::Result<SolverOutput> {
        *self.0.lock().unwrap() = Some(model.clone());
        Ok(SolverOutput::infeasible())
    }
}

fn snapshot(model: &LinearTriadModel) -> String {
    let variables = model.variables.iter().map(|token| (token.name(), token.solver_index)).collect::<Vec<_>>();
    format!("{:?}", ((&model.name, variables, &model.A, &model.b, &model.constraint_names,
        &model.constraint_group_ids, &model.constraint_lazy_flags, &model.constraint_priorities,
        &model.constraint_args, &model.constraint_source_symbol_ids), (&model.lb, &model.ub,
        &model.var_types, &model.c, &model.objective_category)))
}

#[test]
fn direct_mip_adjacency_preserves_full_model_snapshot() {
    let policy = DirectMipPolicy {
        distance_calculator: EuclideanDistanceCalculator,
        travel_time_calculator: DistanceAsTravelTimeCalculator::new(EuclideanDistanceCalculator),
        arc_cost_calculator: DistanceArcCostCalculator,
    };
    for count in [0, 1, 2, 6, 12, 25] {
        let mut instance = (*first25_instance().unwrap()).clone();
        instance.customers.truncate(count);
        instance.vehicle_types[0].amount = count;
        let expected_solver = RecordingSolver::default();
        let actual_solver = RecordingSolver::default();
        let expected = reference::solve_reference(&instance, &expected_solver, &policy);
        let actual = solve(&instance, &actual_solver, &policy);
        assert_eq!(format!("{actual:?}"), format!("{expected:?}"));
        let expected_model = expected_solver.0.lock().unwrap();
        let actual_model = actual_solver.0.lock().unwrap();
        match (expected_model.as_ref(), actual_model.as_ref()) {
            (Some(expected), Some(actual)) => assert_eq!(snapshot(actual), snapshot(expected), "count={count}"),
            (None, None) => assert!(actual.is_err()),
            _ => panic!("model construction changed"),
        }
    }
    let mut invalid = (*small_instance().unwrap()).clone();
    invalid.vehicle_types.clear();
    assert!(solve(&invalid, &RecordingSolver::default(), &policy).is_err());
}

#[cfg(feature = "demo5-gurobi-bp")]
#[test]
#[ignore = "requires a native Gurobi installation; run explicitly with --ignored"]
fn direct_mip_small_native_matches_reference_optimum() {
    use ospf_rust_core::solver::solvers::gurobi::{GurobiConfig, GurobiSolver};

    let instance = small_instance().unwrap();
    let solver = GurobiSolver::with_config(GurobiConfig::new()
        .with_time_limit(10.0).with_mip_gap(0.0).with_output(false));
    let policy = DirectMipPolicy {
        distance_calculator: EuclideanDistanceCalculator,
        travel_time_calculator: DistanceAsTravelTimeCalculator::new(EuclideanDistanceCalculator),
        arc_cost_calculator: DistanceArcCostCalculator,
    };
    let expected = reference::solve_reference(&instance, &solver, &policy).unwrap();
    let actual = solve(&instance, &solver, &policy).unwrap();
    assert_eq!(actual.status, SolverStatus::Optimal);
    assert_eq!(actual.status, expected.status);
    assert!((actual.objective - expected.objective).abs() < 1e-8);
    let covered = actual.customer_sequences.iter().flatten().collect::<HashSet<_>>();
    assert_eq!(covered, instance.customers.iter().map(|customer| &customer.id).collect());
}

#[test]
#[ignore = "manual release timing; run with --ignored --nocapture --test-threads=1"]
fn performance_b10_direct_mip_build() {
    struct NoopSolver;
    impl SolverInfo for NoopSolver {
        fn name(&self) -> &str { "noop" }
        fn capabilities(&self) -> Vec<SolverCapability> { Vec::new() }
    }
    impl LinearSolver for NoopSolver {
        fn solve_linear(&self, model: &LinearTriadModel) -> ospf_rust_core::error::Result<SolverOutput> {
            std::hint::black_box(model);
            Ok(SolverOutput::infeasible())
        }
    }
    let policy = DirectMipPolicy {
        distance_calculator: EuclideanDistanceCalculator,
        travel_time_calculator: DistanceAsTravelTimeCalculator::new(EuclideanDistanceCalculator),
        arc_cost_calculator: DistanceArcCostCalculator,
    };
    for count in [6, 12, 25] {
        let mut instance = (*first25_instance().unwrap()).clone();
        instance.customers.truncate(count);
        for sample in 0..5 {
            for reference in if sample % 2 == 0 { [true, false] } else { [false, true] } {
                let started = std::time::Instant::now();
                let result = if reference { reference::solve_reference(std::hint::black_box(&instance), &NoopSolver, &policy) }
                    else { solve(std::hint::black_box(&instance), &NoopSolver, &policy) };
                let elapsed = started.elapsed();
                assert!(result.is_err());
                eprintln!("direct_mip_build count={count} reference={reference} sample={sample} elapsed={elapsed:?}");
            }
        }
    }
}
