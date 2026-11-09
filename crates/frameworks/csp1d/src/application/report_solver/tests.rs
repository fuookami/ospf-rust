use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
use ospf_rust_core::model::{SubObjective, flatten::Linear};
use ospf_rust_core::solver::{CancellationOrigin, SolveFingerprints, SolveProof, SolveSolution, SolverCapability, SolverInfo, SolverOutput};
use ospf_rust_core::solver::{SolveOptions, SolverProvenance, TerminationReason, cancelled_solve_report, linear_model_fingerprint, linear_model_mapping};
use ospf_rust_core::variable::{Continuous, UInteger, VariableRange};
use crate::{Csp1dColumnGeneration, Csp1dFinalMilpStatus, Csp1dMilp, Csp1dProblem, Csp1dSolutionStatus};
use crate::domain::material::{Material, Product, ProductDemand, WidthRange, roll_count_unit};
use ospf_rust_quantities::quantity::Quantity;
use ospf_rust_quantities::unit::derived::Meter;
use ospf_rust_quantities::unit::physical_unit::CTUnit;
use super::*;

struct FunctionSolver<F> { handler: F, calls: AtomicUsize }
impl<F: Send + Sync> SolverInfo for FunctionSolver<F> {
    fn name(&self) -> &str { "csp1d-contract" }
    fn capabilities(&self) -> Vec<SolverCapability> { vec![SolverCapability::Linear] }
}
impl<F> LinearSolver for FunctionSolver<F>
where
    F: Fn(&LinearTriadModel, &SolveOptions<'_>, usize) -> ospf_rust_core::Result<SolveReport<f64>> + Send + Sync,
{
    fn solve_linear(&self, _: &LinearTriadModel) -> ospf_rust_core::Result<SolverOutput> { panic!("report path required") }
    fn solve_linear_report_with_options(&self, model: &LinearTriadModel, options: &SolveOptions<'_>) -> ospf_rust_core::Result<SolveReport<f64>> {
        (self.handler)(model, options, self.calls.fetch_add(1, Ordering::SeqCst))
    }
}

fn solver(handler: impl Fn(&LinearTriadModel, &SolveOptions<'_>, usize) -> ospf_rust_core::Result<SolveReport<f64>> + Send + Sync + 'static) -> Arc<dyn LinearSolver> {
    Arc::new(FunctionSolver { handler, calls: AtomicUsize::new(0) })
}

fn terminal(model: &LinearTriadModel, reason: TerminationReason) -> ospf_rust_core::Result<SolveReport<f64>> {
    SolveReport::builder(ProblemStatus::Unknown, reason)
        .fingerprints(SolveFingerprints { model: Some(linear_model_fingerprint(model)?), ..Default::default() })
        .model_mapping(Some(linear_model_mapping(model)?)).build()
}

fn incumbent(model: &LinearTriadModel, values: Vec<f64>) -> ospf_rust_core::Result<SolveReport<f64>> {
    let objective = model.c.iter().zip(&values).map(|(cost, value)| cost * value).sum();
    let mut solution = SolveSolution::vector(values);
    solution.objective = Some(objective);
    solution.objective_value = Some(objective);
    SolveReport::builder(ProblemStatus::Feasible, TerminationReason::Completed).solution(solution)
        .fingerprints(SolveFingerprints { model: Some(linear_model_fingerprint(model)?), ..Default::default() })
        .model_mapping(Some(linear_model_mapping(model)?)).build()
}

fn lp_model() -> MetaModel<f64> {
    let mut model = MetaModel::new("row-origins");
    let x = model.register_auto_variable_with_range::<Continuous>("x", VariableRange::bounded(0.0, 10.0)).unwrap();
    let y = model.register_auto_variable_with_range::<Continuous>("y", VariableRange::bounded(0.0, 10.0)).unwrap();
    model.add_ge_constraint(&[(x, 1.0)], 2.0, "lower").unwrap();
    model.add_eq_constraint(&[(y, 1.0)], 1.0, "equal").unwrap();
    model.add_le_constraint(&[(x, 1.0)], 5.0, "upper").unwrap();
    model.add_sub_objective(SubObjective::minimize(Linear::new(vec![
        ospf_rust_core::model::flatten::LinearMonomial::new(1.0, x),
        ospf_rust_core::model::flatten::LinearMonomial::new(3.0, y),
    ], 7.0), "cost"));
    model
}

fn certified_lp(model: &LinearTriadModel) -> ospf_rust_core::Result<SolveReport<f64>> {
    let mut report = incumbent(model, vec![2.0, 1.0])?;
    report.proof = Some(SolveProof::optimality());
    report.solution_presence = ospf_rust_core::solver::SolutionPresence::Optimal;
    report.solution.as_mut().unwrap().dual_solution = Some(vec![-1.0, 0.0, -3.0, 0.0]);
    report.validate()?;
    Ok(report)
}

fn problem() -> Csp1dProblem<f64> {
    let length = |value| Quantity::new(value, Meter::INSTANT.clone());
    let product = Product { id: "p".into(), name: "p".into(), width: vec![length(30.0)], length: None,
        unit_weight: None, weight: None, max_over_produce_length: None, dynamic_length: false };
    let material = Material { id: "m".into(), name: "m".into(), width_range: WidthRange::new(length(0.0), length(100.0)),
        length: None, unit_weight: None, machine_id: None, available_batches: 3 };
    let demand = ProductDemand { product: product.clone(), quantity: Quantity::new(2.0, roll_count_unit()), mode: Some(crate::DemandMode::Roll) };
    Csp1dProblem::new(vec![product], vec![material], Vec::new(), vec![demand])
}

#[test]
fn lp_duals_follow_signed_origins_including_split_equalities_and_constants() {
    let backend = Csp1dNativeBackend::new(solver(|model, _, _| certified_lp(model)), Default::default()).unwrap();
    let result = backend.solve_lp_model(&lp_model()).unwrap();
    assert_eq!(result.duals, vec![1.0, 3.0, 0.0]);
    assert_eq!(result.objective, 12.0);
    let records = backend.reports();
    assert_eq!(records[0].objective_constant, 7.0);
    assert_eq!(records[0].report.solution.as_ref().unwrap().objective, Some(5.0));
    assert_eq!(records[0].report.solution.as_ref().unwrap().dual_solution.as_ref().unwrap().len(), 4);
    assert_eq!(records[0].report.fingerprints.model, Some(linear_model_fingerprint(&records[0].model).unwrap()));
    assert_eq!(records[0].report.model_mapping, Some(linear_model_mapping(&records[0].model).unwrap()));
    let rebuilt = lp_model().linear_model_snapshot().unwrap().compile_flt64().unwrap().model;
    assert_ne!(linear_model_fingerprint(&rebuilt).unwrap(), linear_model_fingerprint(&records[0].model).unwrap());
    let mut copy = records[0].clone(); copy.model.basic.constraint_names.clear();
    assert!(!backend.reports()[0].model.basic.constraint_names.is_empty());
}

#[test]
fn pricing_rejects_uncertified_invalid_or_truncated_duals_without_fallback() {
    for variant in 0..4 {
        let backend = Csp1dNativeBackend::new(solver(move |model, _, _| {
            let mut report = certified_lp(model)?;
            match variant {
                0 => report.proof = None,
                1 => report.solution.as_mut().unwrap().dual_solution = Some(vec![0.0; 4]),
                2 => report.solution.as_mut().unwrap().dual_solution = None,
                _ => report.solution.as_mut().unwrap().dual_solution = Some(vec![-1.0]),
            }
            Ok(report)
        }), Default::default()).unwrap();
        assert!(backend.solve_lp_model(&lp_model()).is_err(), "variant {variant}");
    }
}

#[test]
fn identity_repair_is_limited_to_empty_cancelled_reports() {
    let model = lp_model().linear_model_snapshot().unwrap().compile_flt64().unwrap().model;
    let mut report = SolveReport::builder(ProblemStatus::Unknown, TerminationReason::Cancelled).build().unwrap();
    validate_csp1d_report(&model, &mut report, 1e-7).unwrap();
    assert!(report.proof.is_none());
    let other = LinearTriadModel::new("other");
    assert!(validate_csp1d_report(&other, &mut report, 1e-7).is_err());
    let mut timeout = SolveReport::builder(ProblemStatus::Unknown, TerminationReason::TimeLimit).build().unwrap();
    assert!(validate_csp1d_report(&model, &mut timeout, 1e-7).is_err());
    for tolerance in [f64::NAN, -1.0, 0.5, f64::INFINITY] {
        assert!(validate_csp1d_report(&model, &mut report, tolerance).is_err());
    }
}

#[test]
fn milp_revalidates_original_integrality_rows_objective_and_identity() {
    let mut model = MetaModel::new("integer");
    let x = model.register_auto_variable_with_range::<UInteger>("x", VariableRange::bounded(0.0, 3.0)).unwrap();
    model.add_ge_constraint(&[(x, 1.0)], 2.0, "demand").unwrap();
    model.add_sub_objective(SubObjective::minimize(Linear::new(vec![ospf_rust_core::model::flatten::LinearMonomial::new(1.0, x)], 7.0), "cost"));
    for value in [0.0, 1.5, 4.0] {
        let backend = Csp1dNativeBackend::new(solver(move |model, _, _| incumbent(model, vec![value])), Default::default()).unwrap();
        assert!(backend.solve_milp(&model, false).is_err());
    }
    let backend = Csp1dNativeBackend::new(solver(|model, _, _| incumbent(model, vec![2.0])), Default::default()).unwrap();
    assert!(backend.solve_lp_model(&model).is_err());
    backend.solve_milp(&model, false).unwrap();
    assert_eq!(backend.reports()[0].objective_constant, 7.0);
    for variant in 0..7 {
        let backend = Csp1dNativeBackend::new(solver(move |model, _, _| {
            let mut report = incumbent(model, vec![2.0])?;
            match variant {
                0 => report.fingerprints.model = Some(linear_model_fingerprint(&LinearTriadModel::new("foreign"))?),
                1 => report.solution.as_mut().unwrap().values.clear(),
                2 => { report.solution.as_mut().unwrap().objective = Some(100.0); report.solution.as_mut().unwrap().objective_value = Some(100.0); },
                3 => report.solution.as_mut().unwrap().value = Some(2.0),
                4 => report.solution.as_mut().unwrap().objective = Some(f64::NAN),
                5 => { report.solution.as_mut().unwrap().stable_values.insert("foreign".into(), 2.0); },
                _ => { report.solution.as_mut().unwrap().stable_values.insert(linear_model_mapping(model)?.variables_by_column[0].clone(), 3.0); },
            }
            Ok(report)
        }), Default::default()).unwrap();
        assert!(backend.solve_milp(&model, false).is_err(), "variant {variant}");
    }
}

#[test]
fn timeout_precancel_and_report_limit_never_invent_incumbents_or_start_solver() {
    for cancelled in [false, true] {
        let options = Csp1dExecutionOptions { time_limit: Duration::ZERO, report_limit: 1, ..Default::default() };
        if cancelled { options.cancellation.cancel(CancellationOrigin::User); }
        let backend = Csp1dNativeBackend::new(solver(|_, _, _| panic!("must not execute")), options).unwrap();
        let report = backend.solve_milp(&lp_model(), false).unwrap();
        assert!(!report.has_incumbent());
        assert!(report.proof.is_none());
        assert_eq!(report.termination_reason, if cancelled { TerminationReason::Cancelled } else { TerminationReason::TimeLimit });
        assert!(backend.solve_milp(&lp_model(), true).is_err());
    }
}

#[test]
fn later_child_cancellation_does_not_inherit_first_child_completion() {
    let options = Csp1dExecutionOptions::default();
    let parent = options.cancellation.clone();
    let backend = Csp1dNativeBackend::new(solver(move |model, options, index| {
        let child = options.cancellation_handle.unwrap();
        assert!(child.completed_at_epoch_ms().is_none());
        if index == 0 {
            child.mark_completed();
            terminal(model, TerminationReason::TimeLimit)
        } else {
            parent.cancel(CancellationOrigin::User);
            assert!(child.is_cancelled());
            assert!(child.cancellation_preceded_completion());
            child.mark_completed();
            cancelled_solve_report(SolverProvenance::default(), child)
        }
    }), options).unwrap();
    assert_eq!(backend.solve_milp(&lp_model(), false).unwrap().termination_reason, TerminationReason::TimeLimit);
    assert_eq!(backend.solve_milp(&lp_model(), true).unwrap().termination_reason, TerminationReason::Cancelled);
    assert_eq!(backend.reports().len(), 2);
}

#[test]
fn native_report_failures_never_fall_back_to_heuristic_or_partial_business_output() {
    for cg in [false, true] {
        let backend = Arc::new(Csp1dNativeBackend::new(solver(|model, _, _| terminal(model, TerminationReason::TimeLimit)), Default::default()).unwrap());
        let result = if cg {
            Csp1dColumnGeneration { lp_solver: Some(backend.clone()), milp_solver: Some(backend.clone()), ..Default::default() }.solve_with_trace(problem(), None)
        } else {
            Csp1dMilp { milp_solver: Some(backend.clone()), ..Default::default() }.solve_with_trace(problem(), None)
        };
        assert_eq!(result.solution.status, Csp1dSolutionStatus::Failed);
        assert_eq!(result.trace.final_milp_status, Csp1dFinalMilpStatus::Failed);
        assert!(!result.trace.partial_solution_available);
        assert!(!backend.reports().is_empty());
    }
}

#[test]
fn invalid_options_are_rejected_before_execution() {
    for options in [
        Csp1dExecutionOptions { tolerance: f64::NAN, ..Default::default() },
        Csp1dExecutionOptions { tolerance: -1.0, ..Default::default() },
        Csp1dExecutionOptions { tolerance: 0.5, ..Default::default() },
        Csp1dExecutionOptions { report_limit: 0, ..Default::default() },
        Csp1dExecutionOptions { report_limit: 100_001, ..Default::default() },
    ] {
        assert!(Csp1dNativeBackend::new(solver(|_, _, _| panic!("invalid options")), options).is_err());
    }
}

#[test]
fn pricing_rejects_nonfinite_objective_primal_and_duals() {
    for variant in 0..3 {
        let mut solution = LpMasterSolution::optimal(1.0, vec![1.0], vec![1.0]);
        match variant {
            0 => solution.objective = f64::INFINITY,
            1 => solution.duals[0] = f64::NAN,
            _ => solution.primal[0] = f64::NEG_INFINITY,
        }
        assert!(crate::application::lp_solver::require_pricing_ready(&solution).is_err());
    }
}

#[test]
fn concurrent_calls_are_rejected_without_deadlocking_report_observation() {
    let started = Arc::new(std::sync::Barrier::new(2));
    let release = Arc::new(std::sync::Barrier::new(2));
    let (worker_started, worker_release) = (started.clone(), release.clone());
    let backend = Arc::new(Csp1dNativeBackend::new(solver(move |model, _, _| {
        worker_started.wait(); worker_release.wait();
        terminal(model, TerminationReason::TimeLimit)
    }), Default::default()).unwrap());
    let worker_backend = backend.clone();
    let worker = std::thread::spawn(move || worker_backend.solve_milp(&lp_model(), false));
    started.wait();
    let concurrent = backend.solve_milp(&lp_model(), true);
    let records = backend.reports();
    release.wait();
    worker.join().unwrap().unwrap();
    assert!(concurrent.unwrap_err().to_string().contains("already executing"));
    assert!(records.is_empty());
    assert_eq!(backend.reports().len(), 1);
}

#[derive(Debug)]
struct RoundLp { phase_one: bool, bad_shape: bool, calls: AtomicUsize }
impl Csp1dLpSolver<f64> for RoundLp {
    fn name(&self) -> &str { "round-lp" }
    fn solve_lp(&self, _: &str, model: &MetaModel<f64>) -> Ret<LpMasterSolution> {
        if self.phase_one && self.calls.fetch_add(1, Ordering::SeqCst) == 0 { return Ok(LpMasterSolution::infeasible()); }
        Ok(LpMasterSolution::optimal(41.0, vec![7.0; if self.bad_shape { 1 + model.constraints().len() } else { model.constraints().len() }], Vec::new()))
    }
}

struct InspectPricing { phase_one: bool, observed: Arc<AtomicUsize> }
impl crate::Csp1dPricingGenerator<f64> for InspectPricing {
    fn generate(&self, input: &crate::Csp1dPricingInput<f64>) -> Vec<crate::CuttingPlan<f64>> {
        let key = crate::Csp1dShadowPriceKey::ProductDemand(crate::ProductDemandShadowPriceKey {
            product_id: "p".into(), unit_symbol: crate::shadow_price_unit_symbol(&roll_count_unit()),
        });
        assert_eq!(input.shadow_prices.get(&key), Some(&7.0));
        if self.phase_one {
            let plan = &input.generation_input.existing_plans[0];
            let cost = input.pricing_cost_modifiers.iter().fold(99.0, |cost, modifier| modifier(plan, cost));
            assert_eq!(cost, 0.0);
        }
        self.observed.fetch_add(1, Ordering::SeqCst);
        Vec::new()
    }
}

#[test]
fn actual_round_duals_objective_and_phase_one_zero_cost_reach_pricing() {
    for phase_one in [false, true] {
        let observed = Arc::new(AtomicUsize::new(0));
        let service = Csp1dColumnGeneration {
            lp_solver: Some(Arc::new(RoundLp { phase_one, bad_shape: false, calls: AtomicUsize::new(0) })),
            pricing_generator: Box::new(InspectPricing { phase_one, observed: observed.clone() }),
            ..Default::default()
        };
        let result = service.solve_with_trace(problem(), None);
        assert_eq!(observed.load(Ordering::SeqCst), 1);
        assert_eq!(result.trace.iterations[0].lp_objective, 41.0);
    }
    let observed = Arc::new(AtomicUsize::new(0));
    let result = Csp1dColumnGeneration {
        lp_solver: Some(Arc::new(RoundLp { phase_one: false, bad_shape: true, calls: AtomicUsize::new(0) })),
        pricing_generator: Box::new(InspectPricing { phase_one: false, observed: observed.clone() }),
        ..Default::default()
    }.solve_with_trace(problem(), None);
    assert_eq!(observed.load(Ordering::SeqCst), 0);
    assert_eq!(result.trace.termination_reason, crate::Csp1dTerminationReason::LpSolveFailed);
}

#[derive(Debug)]
struct RequireThree;
impl ospf_rust_framework::model::Pipeline<MetaModel<f64>> for RequireThree {
    fn name(&self) -> &str { "require-three" }
    fn constraint_group(&self) -> Option<&ospf_rust_core::model::ConstraintGroup> { None }
    fn register(&self, model: &mut MetaModel<f64>) { model.add_ge_constraint(&[(0, 1.0)], 3.0, "extension-three").unwrap(); }
    fn invoke(&self, _: &MetaModel<f64>) -> ospf_rust_core::Result<()> { Ok(()) }
}

#[test]
fn injected_milp_report_cannot_bypass_registered_extensions() {
    let backend = Arc::new(Csp1dNativeBackend::new(solver(|model, _, _| incumbent(model, vec![2.0])), Default::default()).unwrap());
    let mut config = crate::Csp1dSolveConfig::default();
    config.extensions.push(crate::Csp1dModelingExtension::new(Arc::new(RequireThree)));
    let result = Csp1dMilp { milp_solver: Some(backend), ..Default::default() }.solve_with_trace(problem(), Some(config));
    assert_eq!(result.solution.status, Csp1dSolutionStatus::Failed);
    assert!(!result.trace.partial_solution_available);
}

#[test]
fn unsafe_or_foreign_warm_starts_fail_before_backend_or_domain_casts() {
    use crate::Csp1dInitialCuttingPlanGenerator;
    let problem = problem();
    let input = crate::CuttingPlanGenerationInput { products: problem.products.clone(), materials: problem.materials.clone(),
        demands: problem.demands.clone(), ..Default::default() };
    let plan = crate::SimpleInitialCuttingPlanGenerator.generate(&input).remove(0);
    for variant in 0..3 {
        let mut usage = crate::CuttingPlanUsage { plan: plan.clone(), amount: 1 };
        if variant == 0 { usage.plan.material.id = "foreign".into(); }
        if variant == 1 { usage.amount = 9_007_199_254_740_992; }
        let usages = if variant == 2 {
            vec![crate::CuttingPlanUsage { amount: 9_007_199_254_740_991, ..usage.clone() }, usage]
        } else { vec![usage] };
        let backend = Arc::new(Csp1dNativeBackend::new(solver(|_, _, _| panic!("invalid warm start must not reach backend")), Default::default()).unwrap());
        let result = Csp1dMilp { milp_solver: Some(backend.clone()), warm_start_plan_usages: usages, ..Default::default() }
            .solve_with_trace(problem.clone(), None);
        assert_eq!(result.solution.status, Csp1dSolutionStatus::Failed, "variant {variant}");
        assert!(backend.reports().is_empty());
    }
}

#[cfg(any(feature = "scip", feature = "scip-bundled", feature = "scip-from-source", feature = "gurobi10", feature = "gurobi11", feature = "gurobi12"))]
fn real_backend_contract(solver: Arc<dyn LinearSolver>) {
    use ospf_rust_core::solver::require_optimal_lp_certificate_for_linear_model;
    let progress_events = Arc::new(AtomicUsize::new(0));
    let observed = progress_events.clone();
    let backend = Csp1dNativeBackend::new(solver.clone(), Csp1dExecutionOptions {
        progress: Some(Arc::new(move |_| { observed.fetch_add(1, Ordering::SeqCst); Ok(()) })), ..Default::default()
    }).unwrap();
    let result = backend.solve_lp_model(&lp_model()).unwrap();
    assert!(progress_events.load(Ordering::SeqCst) > 0);
    assert_eq!(result.duals.len(), 3);
    assert!((result.duals[0] - 1.0).abs() < 1e-7);
    assert!((result.duals[1] - 3.0).abs() < 1e-7);
    assert!((result.objective - 12.0).abs() < 1e-7);
    for cg in [false, true] {
        let backend = Arc::new(Csp1dNativeBackend::new(solver.clone(), Default::default()).unwrap());
        let result = if cg {
            Csp1dColumnGeneration { lp_solver: Some(backend.clone()), milp_solver: Some(backend.clone()), ..Default::default() }.solve_with_trace(problem(), None)
        } else {
            Csp1dMilp { milp_solver: Some(backend.clone()), ..Default::default() }.solve_with_trace(problem(), None)
        };
        assert_eq!(result.solution.status, Csp1dSolutionStatus::Feasible, "{:?}", result.trace);
        assert_eq!(result.trace.final_milp_status, Csp1dFinalMilpStatus::Solved);
        let records = backend.reports();
        for record in &records {
            use ospf_rust_framework::solver::remote::{encode_compiled_linear_model, decode_compiled_linear_model};
            let artifact = encode_compiled_linear_model(&record.model).unwrap();
            let restored = decode_compiled_linear_model(&artifact).unwrap();
            assert_eq!(record.report.fingerprints.model, Some(linear_model_fingerprint(&restored).unwrap()));
            assert_eq!(record.report.model_mapping, Some(linear_model_mapping(&restored).unwrap()));
            assert_eq!(restored.var_types, record.model.var_types);
            let portable: SolveReport<f64, u64> = serde_json::from_str(&serde_json::to_string(&record.report).unwrap()).unwrap();
            if record.stage == Csp1dSolveStage::Lp && portable.is_optimal() {
                require_optimal_lp_certificate_for_linear_model(&portable, &restored).unwrap();
            }
        }
        assert_eq!(records.last().unwrap().stage, if cg { Csp1dSolveStage::FinalMilp } else { Csp1dSolveStage::Milp });
        assert!(records.last().unwrap().report.has_incumbent());
        if cg {
            let lp = records.iter().find(|record| record.stage == Csp1dSolveStage::Lp).unwrap();
            let objective = lp.report.solution.as_ref().unwrap().objective_value.unwrap() + lp.objective_constant;
            assert!((result.trace.iterations[0].lp_objective - objective).abs() < 1e-7);
            assert!((objective - 2.0).abs() < 1e-7);
            assert!(result.trace.lp_failure_message.is_none());
        }
    }
    for cg in [false, true] {
        let backend = Arc::new(Csp1dNativeBackend::new(solver.clone(), Default::default()).unwrap());
        let mut config = crate::Csp1dSolveConfig::default();
        config.extensions.push(crate::Csp1dModelingExtension::new(Arc::new(RequireThree)));
        let result = if cg {
            Csp1dColumnGeneration { lp_solver: Some(backend.clone()), milp_solver: Some(backend.clone()), ..Default::default() }.solve_with_trace(problem(), Some(config))
        } else {
            Csp1dMilp { milp_solver: Some(backend.clone()), ..Default::default() }.solve_with_trace(problem(), Some(config))
        };
        assert_eq!(result.solution.status, Csp1dSolutionStatus::Feasible, "{:?}", result.trace);
        assert_eq!(backend.reports().last().unwrap().report.solution.as_ref().unwrap().values[0], 3.0);
    }
    let backend = Arc::new(Csp1dNativeBackend::new(solver, Default::default()).unwrap());
    let mut infeasible = problem();
    infeasible.materials[0].available_batches = 1;
    let result = Csp1dMilp { milp_solver: Some(backend.clone()), ..Default::default() }.solve_with_trace(infeasible, None);
    assert_eq!(result.solution.status, Csp1dSolutionStatus::Failed);
    assert!(!result.trace.partial_solution_available);
    assert_eq!(backend.reports()[0].report.problem_status, ProblemStatus::Infeasible);
}

#[cfg(any(feature = "scip", feature = "scip-bundled", feature = "scip-from-source"))]
#[test]
fn real_scip_lp_origins_milp_and_column_generation() {
    use ospf_rust_core_plugin_scip::{SCIPConfig, SCIPSolver};
    real_backend_contract(Arc::new(SCIPSolver::with_config(SCIPConfig::default().with_output(false).with_lp_subproblem_defaults())));
}

#[cfg(any(feature = "gurobi10", feature = "gurobi11", feature = "gurobi12"))]
#[test]
fn real_gurobi_lp_origins_milp_and_column_generation() {
    use ospf_rust_core_plugin_gurobi::{GurobiConfig, GurobiSolver};
    real_backend_contract(Arc::new(GurobiSolver::with_config(GurobiConfig::default().with_output(false))));
}
