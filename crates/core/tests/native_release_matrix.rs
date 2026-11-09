//! 原生 release 矩阵 / Native release matrix.
//!
//! 这些测试只在显式 native feature 和 `--include-ignored` 下运行。
//! These tests run only with an explicit native feature and `--include-ignored`.

#[cfg(any(
    feature = "gurobi10",
    feature = "gurobi11",
    feature = "gurobi12",
    feature = "scip"
))]
use ospf_rust_core::intermediate::{BasicLinearTriadModel, LinearTriadModel, SparseVector};
#[cfg(any(
    feature = "gurobi10",
    feature = "gurobi11",
    feature = "gurobi12",
    feature = "scip"
))]
use ospf_rust_core::model::ObjectiveCategory;
#[cfg(any(
    feature = "gurobi10",
    feature = "gurobi11",
    feature = "gurobi12",
    feature = "scip"
))]
use ospf_rust_core::solver::{
    CancellationOrigin, LinearSolver, ProblemStatus, SolutionPresence, SolveHandle, SolveOptions,
    SolveReport, TerminationReason,
};
#[cfg(any(
    feature = "gurobi10",
    feature = "gurobi11",
    feature = "gurobi12",
    feature = "scip"
))]
use ospf_rust_core::token::Token;
#[cfg(any(
    feature = "gurobi10",
    feature = "gurobi11",
    feature = "gurobi12",
    feature = "scip"
))]
use ospf_rust_core::variable::ContinuousVariableItem;
#[cfg(any(
    feature = "gurobi10",
    feature = "gurobi11",
    feature = "gurobi12",
    feature = "scip"
))]
use ospf_rust_core::variable::{BinaryVariableItem, VariableId, VariableType};

#[cfg(any(
    feature = "gurobi10",
    feature = "gurobi11",
    feature = "gurobi12",
    feature = "scip"
))]
fn build_mip_model(name: &str, with_initial_solution: bool) -> LinearTriadModel {
    let item_count = 48usize;
    let mut basic = BasicLinearTriadModel::new(name);
    for index in 0..item_count {
        let variable = BinaryVariableItem::create(
            VariableId::standalone(40_000 + index),
            &format!("x_{}", index),
        );
        let token = Token::from_generic(variable, index);
        if with_initial_solution && index == 0 {
            token.set_result(1.0);
        } else if with_initial_solution {
            token.set_result(0.0);
        }
        basic.add_variable_with_bounds(token, 0.0, 1.0, VariableType::Binary);
    }

    let mut weight = SparseVector::new();
    let mut cardinality = SparseVector::new();
    let mut objective = vec![0.0; item_count];
    for (index, objective_value) in objective.iter_mut().enumerate() {
        weight.add(index, ((index % 9) + 1) as f64);
        cardinality.add(index, 1.0);
        *objective_value = (index + 2) as f64;
    }
    basic.add_constraint(weight, 96.0);
    // 保留根 LP 的分数松弛，便于验证 node limit 下无 incumbent 的合同。
    // Keep a fractional root relaxation so the no-incumbent node-limit contract is exercised.
    basic.add_constraint(cardinality, 17.5);

    let mut model = LinearTriadModel::from_basic(basic);
    model.set_objective(objective, ObjectiveCategory::Maximum);
    model
}

#[cfg(any(feature = "gurobi10", feature = "gurobi11", feature = "gurobi12"))]
fn build_lp_model(name: &str) -> LinearTriadModel {
    let mut basic = BasicLinearTriadModel::new(name);
    let variable_count = 16usize;
    for index in 0..variable_count {
        let variable = ContinuousVariableItem::create(
            VariableId::standalone(41_000 + index),
            &format!("y_{}", index),
        );
        basic.add_variable_with_bounds(
            Token::from_generic(variable, index),
            0.0,
            1.0,
            VariableType::Continuous,
        );
    }
    let mut total = SparseVector::new();
    let mut weighted = SparseVector::new();
    for index in 0..variable_count {
        total.add(index, 1.0);
        weighted.add(index, ((index % 5) + 1) as f64);
    }
    basic.add_constraint(total, 7.25);
    basic.add_constraint(weighted, 22.5);
    let objective = (0..variable_count)
        .map(|index| (index + 1) as f64)
        .collect();
    let mut model = LinearTriadModel::from_basic(basic);
    model.set_objective(objective, ObjectiveCategory::Maximum);
    model
}

#[cfg(any(
    feature = "gurobi10",
    feature = "gurobi11",
    feature = "gurobi12",
    feature = "scip"
))]
fn build_unbounded_model(name: &str) -> LinearTriadModel {
    let mut basic = BasicLinearTriadModel::new(name);
    let variable = ContinuousVariableItem::create(VariableId::standalone(43_000), "u");
    basic.add_variable_with_bounds(
        Token::from_generic(variable, 0),
        0.0,
        f64::INFINITY,
        VariableType::Continuous,
    );
    let mut model = LinearTriadModel::from_basic(basic);
    model.set_objective(vec![1.0], ObjectiveCategory::Maximum);
    model
}

#[cfg(feature = "scip")]
fn build_integer_infeasible_root_model(name: &str) -> LinearTriadModel {
    let mut basic = BasicLinearTriadModel::new(name);
    for index in 0..2usize {
        let variable = BinaryVariableItem::create(
            VariableId::standalone(42_000 + index),
            &format!("z_{}", index),
        );
        basic.add_variable_with_bounds(
            Token::from_generic(variable, index),
            0.0,
            1.0,
            VariableType::Binary,
        );
    }
    let mut upper = SparseVector::new();
    upper.add(0, 1.0);
    upper.add(1, 1.0);
    basic.add_constraint(upper, 1.5);
    let mut lower = SparseVector::new();
    lower.add(0, -1.0);
    lower.add(1, -1.0);
    basic.add_constraint(lower, -1.5);
    let mut model = LinearTriadModel::from_basic(basic);
    model.set_objective(vec![1.0, 1.0], ObjectiveCategory::Maximum);
    model
}

#[cfg(any(
    feature = "gurobi10",
    feature = "gurobi11",
    feature = "gurobi12",
    feature = "scip"
))]
fn assert_limit_report(
    report: &SolveReport<f64>,
    termination: TerminationReason,
    has_incumbent: bool,
) {
    report
        .validate()
        .expect("native release report must validate");
    assert_eq!(report.termination_reason, termination);
    assert_eq!(report.has_incumbent(), has_incumbent);
    assert!(
        !report.is_optimal(),
        "a limit report is not an exact optimum"
    );
    assert_eq!(
        report.solution_presence,
        if has_incumbent {
            SolutionPresence::Incumbent
        } else {
            SolutionPresence::None
        }
    );
    assert_eq!(
        report.problem_status,
        if has_incumbent {
            ProblemStatus::Feasible
        } else {
            ProblemStatus::Unknown
        }
    );
    assert!(report.provenance.backend_version.is_some());
    assert!(!report.provenance.solver_id.is_empty());
    if has_incumbent {
        let solution = report.solution.as_ref().expect("incumbent vector");
        let objective = solution.objective_value.expect("incumbent objective");
        assert!(!solution.values.is_empty());
        assert!(report.statistics.solution_count.unwrap_or_default() >= 1);

        // 无有限 bound 时不能伪造 gap；否则最大化 bound 必须支配 incumbent。
        // Without a finite bound, no gap may be fabricated; otherwise a maximization bound must dominate the incumbent.
        if let Some(best_bound) = report.statistics.best_bound_value {
            let absolute_gap = report.statistics.absolute_gap.expect("incumbent MIP absolute gap");
            let relative_gap = report.statistics.relative_gap.expect("incumbent MIP relative gap");
            assert!(best_bound.is_finite());
            assert!(absolute_gap.is_finite());
            assert!(relative_gap.is_finite());
            assert!(
                best_bound + 1e-7 >= objective,
                "maximization bound {best_bound} is below incumbent {objective}"
            );
            assert!(absolute_gap >= 0.0);
            assert!(relative_gap >= 0.0);
            let expected_absolute_gap = (objective - best_bound).abs();
            let expected_relative_gap = expected_absolute_gap / objective.abs().max(1.0);
            assert!((absolute_gap - expected_absolute_gap).abs() <= 1e-7);
            assert!((relative_gap - expected_relative_gap).abs() <= 1e-7);
        } else {
            assert_eq!(report.statistics.absolute_gap, None);
            assert_eq!(report.statistics.relative_gap, None);
        }
    } else {
        assert!(report.solution.is_none());
    }
}

#[cfg(any(
    feature = "gurobi10",
    feature = "gurobi11",
    feature = "gurobi12",
    feature = "scip"
))]
fn assert_cancelled_report(report: &SolveReport<f64>) {
    report.validate().expect("cancelled report must validate");
    assert_eq!(report.termination_reason, TerminationReason::Cancelled);
    assert_eq!(
        report.diagnostics.extensions.get("cancellation.origin"),
        Some(&"CALLBACK".to_owned())
    );
    assert!(!report.is_optimal());
}

#[cfg(any(
    feature = "gurobi10",
    feature = "gurobi11",
    feature = "gurobi12",
    feature = "scip"
))]
fn assert_unbounded_report(report: &SolveReport<f64>) {
    report
        .validate()
        .expect("native unbounded report must validate");
    assert_eq!(report.problem_status, ProblemStatus::Unbounded);
    assert_eq!(report.termination_reason, TerminationReason::Completed);
    assert_eq!(report.solution_presence, SolutionPresence::None);
    assert!(!report.has_incumbent());
    assert!(!report.is_optimal());
    assert!(report.provenance.backend_version.is_some());
    assert!(!report.provenance.solver_id.is_empty());
}

#[cfg(any(feature = "gurobi10", feature = "gurobi11", feature = "gurobi12"))]
mod gurobi {
    use super::*;
    use ospf_rust_core::error::CoreError;
    use ospf_rust_core::solver::solvers::GurobiSolver;
    use ospf_rust_core::solver::{ProgressValue, SolveProgressReporter, SolveProgressSnapshot, SolveStage};
    use ospf_rust_core::solvers::gurobi::{
        GurobiConfig, GurobiEnvCallback, GurobiNativeCallback, GurobiNativeControl, GurobiNativeObserver,
        GurobiNativeWhere, GurobiTelemetryCallback,
    };
    use std::sync::{Arc, Mutex};
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn config_without_root_shortcuts() -> GurobiConfig {
        let callback: GurobiEnvCallback = Arc::new(|env| {
            env.set(grb::param::Presolve, 0).map_err(|error| {
                CoreError::solver_environment(format!("disable Gurobi presolve: {}", error))
            })?;
            env.set(grb::param::Heuristics, 0.0).map_err(|error| {
                CoreError::solver_environment(format!("disable Gurobi heuristics: {}", error))
            })?;
            env.set(grb::param::Cuts, 0).map_err(|error| {
                CoreError::solver_environment(format!("disable Gurobi cuts: {}", error))
            })?;
            Ok(())
        });
        GurobiConfig::new()
            .with_output(false)
            .with_threads(1)
            .add_env_callback(callback)
    }

    #[test]
    #[ignore = "requires matching Gurobi native library and license"]
    fn node_limit_without_incumbent() {
        let solver = GurobiSolver::with_config(config_without_root_shortcuts().with_node_limit(0));
        let report = solver
            .solve_linear_report(&build_mip_model("gurobi_release_node_no_incumbent", false))
            .expect("Gurobi node-limit report");
        assert_limit_report(&report, TerminationReason::NodeLimit, false);
    }

    #[test]
    #[ignore = "requires matching Gurobi native library and license"]
    fn node_limit_with_incumbent() {
        let solver = GurobiSolver::with_config(config_without_root_shortcuts().with_node_limit(0));
        let report = solver
            .solve_linear_report(&build_mip_model("gurobi_release_node_incumbent", true))
            .expect("Gurobi node-limit report");
        assert_limit_report(&report, TerminationReason::NodeLimit, true);
    }

    #[test]
    #[ignore = "requires matching Gurobi native library and license"]
    fn iteration_limit_without_incumbent() {
        let solver =
            GurobiSolver::with_config(config_without_root_shortcuts().with_max_iterations(0));
        let report = solver
            .solve_linear_report(&build_lp_model("gurobi_release_iteration_no_incumbent"))
            .expect("Gurobi iteration-limit report");
        assert_limit_report(&report, TerminationReason::IterationLimit, false);
    }

    #[test]
    #[ignore = "requires matching Gurobi native library and license"]
    fn time_limit_without_incumbent() {
        let solver =
            GurobiSolver::with_config(config_without_root_shortcuts().with_time_limit(0.0));
        let report = solver
            .solve_linear_report(&build_mip_model("gurobi_release_time_no_incumbent", false))
            .expect("Gurobi time-limit report");
        assert_limit_report(&report, TerminationReason::TimeLimit, false);
    }

    #[test]
    #[ignore = "requires matching Gurobi native library and license"]
    fn time_limit_with_incumbent() {
        let solver = GurobiSolver::with_config(
            config_without_root_shortcuts()
                .with_seed(1)
                .with_time_limit(0.005),
        );
        let report = solver
            .solve_linear_report(&build_mip_model("gurobi_release_time_incumbent", true))
            .expect("Gurobi time-limit report");
        assert_limit_report(&report, TerminationReason::TimeLimit, true);
    }

    #[test]
    #[ignore = "requires matching Gurobi native library and license"]
    fn gap_limit_with_incumbent_preserves_non_exact_report() {
        let solver = GurobiSolver::with_config(
            config_without_root_shortcuts()
                .with_mip_gap(0.5)
                .with_node_limit(10_000),
        );
        let report = solver
            .solve_linear_report(&build_mip_model("gurobi_release_gap_incumbent", true))
            .expect("Gurobi gap-limit report");
        assert_limit_report(&report, TerminationReason::GapLimit, true);
    }

    #[test]
    #[ignore = "requires matching Gurobi native library and license"]
    fn unbounded_model_preserves_native_terminal() {
        let solver = GurobiSolver::with_config(config_without_root_shortcuts());
        let events = Arc::new(Mutex::new(Vec::new()));
        let recorded = events.clone();
        let reporter: SolveProgressReporter = Arc::new(move |event| {
            recorded.lock().unwrap().push(event.clone());
            Ok(())
        });
        let options = SolveOptions::new().with_progress_reporter(Some(&reporter));
        let report = solver
            .solve_linear_report_with_options(&build_unbounded_model("gurobi_release_unbounded"), &options)
            .expect("Gurobi unbounded report");
        assert_unbounded_report(&report);
        assert!(report.statistics.best_bound_value.is_none_or(|bound| bound.abs() < grb::INFINITY));
        let events = events.lock().unwrap();
        assert!(events.last().unwrap().terminal);
        assert_eq!(events.last().unwrap().best_bound, report.statistics.best_bound_value);
    }

    #[test]
    #[ignore = "requires matching Gurobi native library and license"]
    fn solution_limit_with_incumbent() {
        let solver = GurobiSolver::with_config(
            GurobiConfig::new()
                .with_output(false)
                .with_threads(1)
                .with_solution_limit(1),
        );
        let report = solver
            .solve_linear_report(&build_mip_model("gurobi_release_solution_limit", true))
            .expect("Gurobi solution-limit report");
        assert_limit_report(&report, TerminationReason::SolutionLimit, true);
    }

    #[test]
    #[ignore = "requires matching Gurobi native library and license"]
    fn callback_cancellation_preserves_origin() {
        let handle = SolveHandle::new();
        let callback_handle = handle.clone();
        let callback_hits = Arc::new(AtomicUsize::new(0));
        let callback_hits_ref = callback_hits.clone();
        let observer: GurobiNativeObserver = Arc::new(move |snapshot| {
            if snapshot.where_point == GurobiNativeWhere::Mip
                && callback_hits_ref.fetch_add(1, Ordering::SeqCst) == 0
            {
                callback_handle.cancel(CancellationOrigin::Callback);
            }
            Ok(GurobiNativeControl::Continue)
        });
        let solver = GurobiSolver::with_config(
            GurobiConfig::new()
                .with_output(false)
                .with_threads(1)
                .add_native_observer(observer),
        );
        let options = SolveOptions::new().with_cancellation_handle(Some(&handle));
        let report = solver
            .solve_linear_report_with_options(
                &build_mip_model("gurobi_release_callback_cancel", false),
                &options,
            )
            .expect("Gurobi cancellation report");
        assert!(callback_hits.load(Ordering::SeqCst) > 0);
        assert_cancelled_report(&report);
    }

    #[test]
    #[ignore = "requires matching Gurobi native library and license"]
    fn intermediate_progress_coexists_with_native_telemetry() {
        use ospf_rust_core::intermediate::{BasicQuadraticTetradModel, QuadraticTetradModel, SparseMatrix};
        use ospf_rust_core::model::{ConstraintRelation, mechanism::QuadraticInequality};
        use ospf_rust_core::solver::QuadraticSolver;
        use ospf_rust_core::symbol::flatten::{Quadratic, QuadraticMonomial};

        for (quadratic, pool_size) in [(false, 1), (false, 2), (true, 1), (true, 2)] {
            let events = Arc::new(Mutex::new(Vec::<SolveProgressSnapshot>::new()));
            let recorded = events.clone();
            let reporter: SolveProgressReporter = Arc::new(move |event| {
                recorded.lock().unwrap().push(event.clone());
                Ok(())
            });
            let telemetry_hits = Arc::new(AtomicUsize::new(0));
            let recorded_hits = telemetry_hits.clone();
            let telemetry: GurobiTelemetryCallback = Arc::new(move |_| {
                recorded_hits.fetch_add(1, Ordering::SeqCst);
                Ok(())
            });
            let solver = GurobiSolver::with_config(config_without_root_shortcuts()
                .with_telemetry_min_interval(1e-9).with_telemetry_callback(Some(telemetry)));
            let options = SolveOptions::new().with_solution_amount(pool_size).with_progress_reporter(Some(&reporter));
            let linear = build_mip_model("gurobi_progress", false);
            let report = if quadratic {
                let mut model = QuadraticTetradModel::from_basic(BasicQuadraticTetradModel::from_linear(linear.basic));
                model.set_objective(linear.c, SparseMatrix::new(), linear.objective_category);
                model.add_quadratic_constraint_with_metadata(
                    QuadraticInequality::new(
                        Quadratic::new(vec![QuadraticMonomial::new_quadratic(1.0, 0, 0)], 0.0),
                        ConstraintRelation::LessEqual,
                        1.0,
                    ),
                    "binary-square".to_owned(), None, false, 0, None, None,
                );
                solver.solve_quadratic_report_with_options(&model, &options)
            } else {
                solver.solve_linear_report_with_options(&linear, &options)
            }.expect("Gurobi progress solve");
            report.validate().unwrap();
            assert!(telemetry_hits.load(Ordering::SeqCst) > 0);
            let events = events.lock().unwrap();
            assert_eq!(events.first().unwrap().stage_progress, ProgressValue::Known(0.0));
            assert!(events.last().unwrap().terminal);
            assert!(events.windows(2).all(|pair| pair[0].elapsed <= pair[1].elapsed));
            let intermediate: Vec<_> = events.iter().filter(|event| {
                !event.terminal && event.stage_progress == ProgressValue::Indeterminate
            }).collect();
            assert!(!intermediate.is_empty(), "missing intermediate progress: {events:?}");
            for event in intermediate {
                assert_eq!(event.attempt_id, "Gurobi");
                assert_eq!(event.stage, SolveStage::Solving);
                assert_eq!(event.overall_progress, ProgressValue::Indeterminate);
                assert!(event.objective_value.is_none_or(|value| value.is_finite() && value.abs() < grb::INFINITY));
                assert!(event.best_bound.is_none_or(|value| value.is_finite() && value.abs() < grb::INFINITY));
                match (event.objective_value, event.best_bound, event.relative_gap) {
                    (Some(objective), Some(bound), Some(gap)) => {
                        let expected = (objective - bound).abs() / objective.abs().max(1.0);
                        assert!((gap - expected).abs() < 1e-9);
                    }
                    (_, _, None) => {}
                    values => panic!("inconsistent progress values: {values:?}"),
                }
            }
        }
    }

    #[test]
    #[ignore = "requires matching Gurobi native library and license"]
    fn intermediate_progress_error_and_panic_interrupt_the_solve() {
        for panic_in_reporter in [false, true] {
            let reporter: SolveProgressReporter = Arc::new(move |event| {
                if event.stage_progress == ProgressValue::Indeterminate {
                    if panic_in_reporter { panic!("progress panic"); }
                    return Err(CoreError::callback_error("progress failure"));
                }
                Ok(())
            });
            let solver = GurobiSolver::with_config(config_without_root_shortcuts());
            let options = SolveOptions::new().with_progress_reporter(Some(&reporter));
            let error = solver.solve_linear_report_with_options(&build_mip_model("gurobi_progress_failure", false), &options)
                .expect_err("intermediate callback must fail the solve");
            assert!(error.to_string().contains(if panic_in_reporter { "progress callback panicked" } else { "progress failure" }));
        }
    }

    #[test]
    #[ignore = "requires matching Gurobi native library and license"]
    fn intermediate_progress_can_cancel_the_solve() {
        let handle = SolveHandle::new();
        let callback_handle = handle.clone();
        let reporter: SolveProgressReporter = Arc::new(move |event| {
            if event.stage_progress == ProgressValue::Indeterminate {
                callback_handle.cancel(CancellationOrigin::Callback);
            }
            Ok(())
        });
        let solver = GurobiSolver::with_config(config_without_root_shortcuts());
        let options = SolveOptions::new().with_cancellation_handle(Some(&handle)).with_progress_reporter(Some(&reporter));
        let report = solver.solve_linear_report_with_options(&build_mip_model("gurobi_progress_cancel", false), &options)
            .expect("progress cancellation report");
        assert_cancelled_report(&report);
    }

    #[test]
    #[ignore = "requires matching Gurobi native library and license"]
    fn intermediate_progress_uses_the_accepted_incumbent_at_mip_solution_callbacks() {
        let events = Arc::new(Mutex::new(Vec::<SolveProgressSnapshot>::new()));
        let recorded = events.clone();
        let reporter: SolveProgressReporter = Arc::new(move |event| {
            recorded.lock().unwrap().push(event.clone());
            Ok(())
        });
        let observed = Arc::new(Mutex::new(Vec::new()));
        let captured = observed.clone();
        let callback: GurobiNativeCallback = Arc::new(move |where_ctx| {
            if let grb::callback::Where::MIPSol(context) = where_ctx {
                let expected = context.obj_best().ok().filter(|value| value.is_finite() && value.abs() < grb::INFINITY);
                let latest = events.lock().unwrap().last().cloned();
                captured.lock().unwrap().push((latest, expected));
            }
            Ok(())
        });
        let solver = GurobiSolver::with_config(config_without_root_shortcuts()
            .with_telemetry_min_interval(1e-9).with_native_callback(Some(callback)));
        let options = SolveOptions::new().with_progress_reporter(Some(&reporter));
        solver.solve_linear_report_with_options(&build_mip_model("gurobi_accepted_incumbent", false), &options).unwrap();
        let observed = observed.lock().unwrap();
        assert!(!observed.is_empty());
        for (latest, expected) in observed.iter() {
            let latest = latest.as_ref().expect("MIPSol progress snapshot");
            assert_eq!(latest.stage_progress, ProgressValue::Indeterminate);
            assert_eq!(latest.objective_value, *expected);
        }
    }

    #[test]
    #[ignore = "requires matching Gurobi native library and license"]
    fn continuous_progress_and_cp_objective_constant_are_preserved() {
        use ospf_rust_core::model::constraint_programming::{
            ConstraintDefinition, ConstraintProgrammingConstraint, ConstraintProgrammingModel,
            IntegerDomain, IntegerExpression, IntegerObjective, IntegerRelation, IntegerTerm, IntegerVariable,
        };
        use ospf_rust_core::solver::constraint_programming::{
            ConstraintProgrammingSolveOptions, ConstraintProgrammingSolver, MipBackedConstraintProgrammingSolver,
        };

        let events = Arc::new(Mutex::new(Vec::<SolveProgressSnapshot>::new()));
        let recorded = events.clone();
        let reporter: SolveProgressReporter = Arc::new(move |event| {
            recorded.lock().unwrap().push(event.clone());
            Ok(())
        });
        let config = config_without_root_shortcuts().with_telemetry_min_interval(1e-9);
        let solver = GurobiSolver::with_config(config.clone());
        let options = SolveOptions::new().with_progress_reporter(Some(&reporter));
        solver.solve_linear_report_with_options(&build_lp_model("gurobi_lp_progress"), &options).unwrap();
        assert!(events.lock().unwrap().iter().any(|event| event.stage_progress == ProgressValue::Indeterminate));
        events.lock().unwrap().clear();

        let mut model = ConstraintProgrammingModel::new("gurobi-cp-progress");
        let mut objective_terms = Vec::new();
        let mut limit_terms = Vec::new();
        for index in 0..48 {
            let variable = IntegerVariable::new(format!("x{index}"));
            model.register_variable(variable.clone(), IntegerDomain::boolean()).unwrap();
            objective_terms.push(IntegerTerm { variable: variable.clone(), coefficient: index + 2 });
            limit_terms.push(IntegerTerm { variable, coefficient: 1 });
        }
        model.add_constraint(ConstraintDefinition::new(
            "limit",
            ConstraintProgrammingConstraint::integer(
                IntegerExpression::linear(0, limit_terms).unwrap(), IntegerRelation::LessOrEqual, 17,
            ),
        )).unwrap();
        model.set_objective(IntegerObjective::maximize(IntegerExpression::linear(137, objective_terms).unwrap()));
        let options = ConstraintProgrammingSolveOptions::builder().progress_reporter(Some(&reporter)).finish();
        let report = MipBackedConstraintProgrammingSolver::new(GurobiSolver::with_config(config))
            .solve_constraint_programming(&model.freeze().unwrap(), &options).unwrap();
        assert!(report.is_optimal());
        let events = events.lock().unwrap();
        let intermediate: Vec<_> = events.iter().filter(|event| event.stage_progress == ProgressValue::Indeterminate).collect();
        assert!(!intermediate.is_empty());
        assert!(intermediate.iter().any(|event| event.objective_value.is_some() || event.best_bound.is_some()));
        for event in intermediate {
            assert!(event.objective_value.is_none_or(|value| value >= 137.0));
            assert!(event.best_bound.is_none_or(|value| value >= 137.0));
        }
        let terminal = events.last().unwrap();
        assert!(terminal.terminal);
        assert_eq!(terminal.objective_value, report.solution.as_ref().and_then(|solution| solution.objective).map(|value| value as f64));
    }

    #[test]
    #[ignore = "requires matching Gurobi native library and license"]
    fn interruption_preserves_existing_linear_incumbent() {
        for cancel in [false, true] {
            let handle = SolveHandle::new();
            let callback_handle = handle.clone();
            let observer: GurobiNativeObserver = Arc::new(move |snapshot| {
                if snapshot.where_point == GurobiNativeWhere::Mip
                    && snapshot.incumbent_solution.is_none()
                    && snapshot.objective_value.is_some_and(|value| value.abs() < 1e90)
                {
                    if cancel {
                        callback_handle.cancel(CancellationOrigin::Callback);
                    }
                    return Ok(GurobiNativeControl::Terminate);
                }
                Ok(GurobiNativeControl::Continue)
            });
            let solver = GurobiSolver::with_config(
                config_without_root_shortcuts().add_native_observer(observer),
            );
            let options = SolveOptions::new().with_cancellation_handle(Some(&handle));
            let report = solver
                .solve_linear_report_with_options(
                    &build_mip_model("gurobi_interrupted_incumbent", true),
                    &options,
                )
                .expect("Gurobi interruption report");
            assert_eq!(
                report.termination_reason,
                if cancel { TerminationReason::Cancelled } else { TerminationReason::Interrupted }
            );
            assert_eq!(report.problem_status, ProblemStatus::Feasible);
            assert!(report.has_incumbent());
            assert!(report.statistics.solution_count.unwrap_or_default() > 0);
            assert!(report.proof.is_none());
            report.validate().expect("interrupted incumbent report must validate");
        }
    }
}

#[cfg(feature = "scip")]
mod scip {
    use super::*;
    use ospf_rust_core::error::CoreError;
    use ospf_rust_core_plugin_scip::SCIPSolver;
    use ospf_rust_core::solver::{ProgressValue, SolveProgressReporter, SolveProgressSnapshot, SolveStage};
    use ospf_rust_core_plugin_scip::{
        PresolvingMode, SCIPConfig, SCIPNativeControl, SCIPNativeObserver, SCIPTelemetryCallback,
    };
    use std::sync::{Arc, Mutex};
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn config() -> SCIPConfig {
        SCIPConfig::recommended_lp_subproblem_defaults()
            .with_output(false)
            .with_presolving(PresolvingMode::Off)
            .with_heuristics_priority(0)
    }

    #[test]
    #[ignore = "requires matching SCIP native library"]
    fn node_limit_without_incumbent() {
        let solver = SCIPSolver::with_config(config().with_node_limit(0));
        let report = solver
            .solve_linear_report(&build_integer_infeasible_root_model(
                "scip_release_node_no_incumbent",
            ))
            .expect("SCIP node-limit report");
        assert_limit_report(&report, TerminationReason::NodeLimit, false);
    }

    #[test]
    #[ignore = "requires matching SCIP native library"]
    fn node_limit_with_incumbent() {
        let solver = SCIPSolver::with_config(config().with_node_limit(0));
        let report = solver
            .solve_linear_report(&build_mip_model("scip_release_node_incumbent", true))
            .expect("SCIP node-limit report");
        assert_limit_report(&report, TerminationReason::NodeLimit, true);
        assert_eq!(report.statistics.best_bound_value, None);
    }

    #[test]
    #[ignore = "requires matching SCIP native library"]
    fn iteration_limit_without_incumbent() {
        let solver = SCIPSolver::with_config(config().with_max_iterations(0));
        let report = solver
            .solve_linear_report(&build_integer_infeasible_root_model(
                "scip_release_iteration_no_incumbent",
            ))
            .expect("SCIP iteration-limit report");
        assert_eq!(
            report.termination_reason,
            TerminationReason::IterationLimit,
            "SCIP iteration fixture report: iterations={:?}, problem_status={:?}, solution_presence={:?}",
            report.statistics.iterations,
            report.problem_status,
            report.solution_presence
        );
        assert_limit_report(&report, TerminationReason::IterationLimit, false);
        assert!(
            report.statistics.iterations.is_some(),
            "SCIP iteration-limit fixture should expose native LP statistics"
        );
    }

    #[test]
    #[ignore = "requires matching SCIP native library"]
    fn time_limit_without_incumbent() {
        let solver = SCIPSolver::with_config(config().with_time_limit(0.0));
        let report = solver
            .solve_linear_report(&build_integer_infeasible_root_model(
                "scip_release_time_no_incumbent",
            ))
            .expect("SCIP time-limit report");
        assert_limit_report(&report, TerminationReason::TimeLimit, false);
    }

    #[test]
    #[ignore = "requires matching SCIP native library"]
    fn time_limit_with_incumbent() {
        let solver = SCIPSolver::with_config(config().with_time_limit(0.0));
        let report = solver
            .solve_linear_report(&build_mip_model("scip_release_time_incumbent", true))
            .expect("SCIP time-limit report");
        assert_limit_report(&report, TerminationReason::TimeLimit, true);
        assert_eq!(report.statistics.best_bound_value, None);
    }

    #[test]
    #[ignore = "requires matching SCIP native library"]
    fn gap_limit_with_incumbent() {
        let solver = SCIPSolver::with_config(config().with_mip_gap(1.0));
        let report = solver
            .solve_linear_report(&build_mip_model("scip_release_gap_incumbent", true))
            .expect("SCIP gap-limit report");
        assert_limit_report(&report, TerminationReason::GapLimit, true);
        assert!(report.statistics.best_bound_value.is_some());
    }

    #[test]
    #[ignore = "requires matching SCIP native library"]
    fn memory_limit_without_incumbent() {
        let solver = SCIPSolver::with_config(config().with_mem_limit(0.0));
        let report = solver
            .solve_linear_report(&build_mip_model("scip_release_memory_no_incumbent", false))
            .expect("SCIP memory-limit report");
        assert_limit_report(&report, TerminationReason::MemoryLimit, false);
    }

    #[test]
    #[ignore = "requires matching SCIP native library"]
    fn unbounded_model_preserves_native_terminal() {
        let solver = SCIPSolver::with_config(config());
        let events = Arc::new(Mutex::new(Vec::new()));
        let recorded = events.clone();
        let reporter: SolveProgressReporter = Arc::new(move |event| {
            recorded.lock().unwrap().push(event.clone());
            Ok(())
        });
        let options = SolveOptions::new().with_progress_reporter(Some(&reporter));
        let report = solver
            .solve_linear_report_with_options(&build_unbounded_model("scip_release_unbounded"), &options)
            .expect("SCIP unbounded report");
        assert_unbounded_report(&report);
        assert!(report.statistics.best_bound_value.is_none_or(|bound| bound.abs() < 1e20));
        let events = events.lock().unwrap();
        assert!(events.last().unwrap().terminal);
        assert_eq!(events.last().unwrap().best_bound, report.statistics.best_bound_value);
    }

    #[test]
    #[ignore = "requires matching SCIP native library"]
    fn solution_limit_with_incumbent() {
        let solver = SCIPSolver::with_config(config().with_solution_limit(1));
        let report = solver
            .solve_linear_report(&build_mip_model("scip_release_solution_limit", true))
            .expect("SCIP solution-limit report");
        assert_limit_report(&report, TerminationReason::SolutionLimit, true);
        assert_eq!(report.statistics.best_bound_value, None);
    }

    #[test]
    #[ignore = "requires matching SCIP native library"]
    fn callback_cancellation_preserves_origin() {
        let handle = SolveHandle::new();
        let callback_handle = handle.clone();
        let callback_hits = Arc::new(AtomicUsize::new(0));
        let callback_hits_ref = callback_hits.clone();
        let observer: SCIPNativeObserver = Arc::new(move |_snapshot| {
            if callback_hits_ref.fetch_add(1, Ordering::SeqCst) == 0 {
                callback_handle.cancel(CancellationOrigin::Callback);
            }
            Ok(SCIPNativeControl::Continue)
        });
        let solver = SCIPSolver::with_config(config().add_native_observer(observer));
        let options = SolveOptions::new().with_cancellation_handle(Some(&handle));
        let report = solver
            .solve_linear_report_with_options(
                &build_mip_model("scip_release_callback_cancel", false),
                &options,
            )
            .expect("SCIP cancellation report");
        assert!(callback_hits.load(Ordering::SeqCst) > 0);
        assert_cancelled_report(&report);
    }

    #[test]
    #[ignore = "requires matching SCIP native library"]
    fn intermediate_progress_coexists_with_native_telemetry() {
        use ospf_rust_core::intermediate::{BasicQuadraticTetradModel, QuadraticTetradModel, SparseMatrix};
        use ospf_rust_core::model::{ConstraintRelation, mechanism::QuadraticInequality};
        use ospf_rust_core::solver::QuadraticSolver;
        use ospf_rust_core::symbol::flatten::{Quadratic, QuadraticMonomial};

        for (quadratic, pool_size) in [(false, 1), (false, 2), (true, 1)] {
            if quadratic && !cfg!(feature = "scip-quadratic") { continue; }
            let events = Arc::new(Mutex::new(Vec::<SolveProgressSnapshot>::new()));
            let recorded = events.clone();
            let reporter: SolveProgressReporter = Arc::new(move |event| {
                recorded.lock().unwrap().push(event.clone());
                Ok(())
            });
            let telemetry_hits = Arc::new(AtomicUsize::new(0));
            let recorded_hits = telemetry_hits.clone();
            let telemetry: SCIPTelemetryCallback = Arc::new(move |_| {
                recorded_hits.fetch_add(1, Ordering::SeqCst);
                Ok(())
            });
            let solver = SCIPSolver::with_config(config().with_telemetry_callback(Some(telemetry)));
            let options = SolveOptions::new().with_solution_amount(pool_size).with_progress_reporter(Some(&reporter));
            let linear = build_mip_model("scip_progress", true);
            let report = if quadratic {
                let mut model = QuadraticTetradModel::from_basic(BasicQuadraticTetradModel::from_linear(linear.basic));
                model.set_objective(linear.c, SparseMatrix::new(), linear.objective_category);
                model.add_quadratic_constraint_with_metadata(
                    QuadraticInequality::new(
                        Quadratic::new(vec![QuadraticMonomial::new_quadratic(1.0, 0, 0)], 0.0),
                        ConstraintRelation::LessEqual,
                        1.0,
                    ),
                    "binary-square".to_owned(), None, false, 0, None, None,
                );
                solver.solve_quadratic_report_with_options(&model, &options)
            } else {
                solver.solve_linear_report_with_options(&linear, &options)
            }.expect("SCIP progress solve");
            report.validate().unwrap();
            assert!(telemetry_hits.load(Ordering::SeqCst) > 0);
            let events = events.lock().unwrap();
            assert_eq!(events.first().unwrap().stage_progress, ProgressValue::Known(0.0));
            assert!(events.last().unwrap().terminal);
            let intermediate: Vec<_> = events.iter().filter(|event| {
                !event.terminal && event.stage_progress == ProgressValue::Indeterminate
            }).collect();
            assert!(!intermediate.is_empty(), "missing intermediate progress: {events:?}");
            for event in intermediate {
                assert_eq!(event.attempt_id, "SCIP");
                assert_eq!(event.stage, SolveStage::Solving);
                assert_eq!(event.overall_progress, ProgressValue::Indeterminate);
                assert!(event.objective_value.is_none_or(|value| value.is_finite() && value.abs() < 1e20));
                assert!(event.best_bound.is_none_or(|value| value.is_finite() && value.abs() < 1e20));
                match (event.objective_value, event.best_bound, event.relative_gap) {
                    (Some(objective), Some(bound), Some(gap)) => {
                        let expected = (objective - bound).abs() / objective.abs().max(1.0);
                        assert!((gap - expected).abs() < 1e-9);
                    }
                    (_, _, None) => {}
                    values => panic!("inconsistent progress values: {values:?}"),
                }
            }
        }
    }

    #[test]
    #[ignore = "requires matching SCIP native library"]
    fn intermediate_progress_error_and_panic_interrupt_the_solve() {
        for panic_in_reporter in [false, true] {
            let reporter: SolveProgressReporter = Arc::new(move |event| {
                if event.stage_progress == ProgressValue::Indeterminate {
                    if panic_in_reporter { panic!("progress panic"); }
                    return Err(CoreError::callback_error("progress failure"));
                }
                Ok(())
            });
            let solver = SCIPSolver::with_config(config());
            let options = SolveOptions::new().with_progress_reporter(Some(&reporter));
            let error = solver.solve_linear_report_with_options(&build_mip_model("scip_progress_failure", true), &options)
                .expect_err("intermediate callback must fail the solve");
            assert!(error.to_string().contains(if panic_in_reporter { "progress callback panicked" } else { "progress failure" }));
        }
    }

    #[test]
    #[ignore = "requires matching SCIP native library"]
    fn intermediate_progress_can_cancel_the_solve() {
        let handle = SolveHandle::new();
        let callback_handle = handle.clone();
        let reporter: SolveProgressReporter = Arc::new(move |event| {
            if event.stage_progress == ProgressValue::Indeterminate {
                callback_handle.cancel(CancellationOrigin::Callback);
            }
            Ok(())
        });
        let solver = SCIPSolver::with_config(config());
        let options = SolveOptions::new().with_cancellation_handle(Some(&handle)).with_progress_reporter(Some(&reporter));
        let report = solver.solve_linear_report_with_options(&build_mip_model("scip_progress_cancel", true), &options)
            .expect("progress cancellation report");
        assert_cancelled_report(&report);
    }

    #[test]
    #[ignore = "requires matching SCIP native library"]
    fn interruption_preserves_existing_linear_and_quadratic_incumbents() {
        use ospf_rust_core::intermediate::{
            BasicQuadraticTetradModel, QuadraticTetradModel, SparseMatrix,
        };
        use ospf_rust_core::model::{ConstraintRelation, mechanism::QuadraticInequality};
        use ospf_rust_core::solver::QuadraticSolver;
        use ospf_rust_core::symbol::flatten::{Quadratic, QuadraticMonomial};

        for quadratic in [false, true] {
            if quadratic && !cfg!(feature = "scip-quadratic") {
                continue;
            }
            for cancel in [false, true] {
                let handle = SolveHandle::new();
                let callback_handle = handle.clone();
                let observer: SCIPNativeObserver = Arc::new(move |snapshot| {
                    if snapshot.incumbent_solution.is_some() {
                        if cancel {
                            callback_handle.cancel(CancellationOrigin::Callback);
                        }
                        return Ok(SCIPNativeControl::Interrupt);
                    }
                    Ok(SCIPNativeControl::Continue)
                });
                let solver = SCIPSolver::with_config(config().add_native_observer(observer));
                let options = SolveOptions::new().with_cancellation_handle(Some(&handle));
                let linear = build_mip_model("scip_interrupted_incumbent", true);
                let report = if quadratic {
                    let mut model = QuadraticTetradModel::from_basic(
                        BasicQuadraticTetradModel::from_linear(linear.basic),
                    );
                    model.set_objective(linear.c, SparseMatrix::new(), linear.objective_category);
                    model.add_quadratic_constraint_with_metadata(
                        QuadraticInequality::new(
                            Quadratic::new(vec![QuadraticMonomial::new_quadratic(1.0, 0, 0)], 0.0),
                            ConstraintRelation::LessEqual,
                            1.0,
                        ),
                        "binary-square".to_owned(),
                        None,
                        false,
                        0,
                        None,
                        None,
                    );
                    solver.solve_quadratic_report_with_options(&model, &options)
                } else {
                    solver.solve_linear_report_with_options(&linear, &options)
                }
                .expect("SCIP interruption report");
                assert_eq!(
                    report.termination_reason,
                    if cancel { TerminationReason::Cancelled } else { TerminationReason::Interrupted }
                );
                assert_eq!(report.problem_status, ProblemStatus::Feasible);
                assert!(report.has_incumbent());
                assert!(report.statistics.solution_count.unwrap_or_default() > 0);
                assert!(report.proof.is_none());
                report
                    .validate()
                    .expect("interrupted incumbent report must validate");
            }
        }
    }
}

#[cfg(not(any(
    feature = "gurobi10",
    feature = "gurobi11",
    feature = "gurobi12",
    feature = "scip"
)))]
#[test]
#[ignore = "native release matrix requires an explicit Gurobi or SCIP feature"]
fn native_release_matrix_requires_explicit_backend_feature() {
    panic!("native release matrix requires an explicit Gurobi or SCIP feature");
}
