use std::sync::atomic::{AtomicUsize, Ordering};
use ospf_rust_core::model::LinearObjectiveInput;
use ospf_rust_core::solver::{SolutionPresence, SolveProof, SolveSolution, SolverCapability, SolverInfo, SolverOutput};
use ospf_rust_core::variable::{Continuous, UInteger, VariableRange};
use super::*;

struct FunctionSolver<F> { handler: F, calls: AtomicUsize }
impl<F: Send + Sync> SolverInfo for FunctionSolver<F> {
    fn name(&self) -> &str { "linear-execution-contract" }
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

fn executor(handler: impl Fn(&LinearTriadModel, &SolveOptions<'_>, usize) -> ospf_rust_core::Result<SolveReport<f64>> + Send + Sync + 'static) -> LinearReportExecutor<u32> {
    LinearReportExecutor::new(Arc::new(FunctionSolver { handler, calls: AtomicUsize::new(0) }), Default::default()).unwrap()
}

fn lp_model() -> MetaModel<f64> {
    let mut model = MetaModel::new("row-origins");
    let x = model.register_auto_variable_with_range::<Continuous>("x", VariableRange::bounded(0.0, 10.0)).unwrap();
    let y = model.register_auto_variable_with_range::<Continuous>("y", VariableRange::bounded(0.0, 10.0)).unwrap();
    model.add_ge_constraint(&[(x, 1.0)], 2.0, "lower").unwrap();
    model.add_eq_constraint(&[(y, 1.0)], 1.0, "equal").unwrap();
    model.add_le_constraint(&[(x, 1.0)], 5.0, "upper").unwrap();
    model.add_linear_objective_input(LinearObjectiveInput::minimize("cost").term(x, 1.0).term(y, 3.0).constant(7.0));
    model
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

fn certified_lp(model: &LinearTriadModel) -> ospf_rust_core::Result<SolveReport<f64>> {
    let mut report = incumbent(model, vec![2.0, 1.0])?;
    report.proof = Some(SolveProof::optimality());
    report.solution_presence = SolutionPresence::Optimal;
    report.solution.as_mut().unwrap().dual_solution = Some(vec![-1.0, 0.0, -3.0, 0.0]);
    report.validate()?;
    Ok(report)
}

fn terminal(model: &LinearTriadModel, reason: TerminationReason) -> ospf_rust_core::Result<SolveReport<f64>> {
    SolveReport::builder(ProblemStatus::Unknown, reason)
        .fingerprints(SolveFingerprints { model: Some(linear_model_fingerprint(model)?), ..Default::default() })
        .model_mapping(Some(linear_model_mapping(model)?)).build()
}

#[test]
fn actual_signed_split_duals_full_vectors_and_constants_are_retained() {
    let execution = executor(|model, _, _| certified_lp(model));
    let result = execution.solve(&lp_model(), 11, LinearExecutionMode::Lp).unwrap();
    assert_eq!(result.objective, Some(12.0));
    assert_eq!(result.original_duals, Some(vec![1.0, 3.0, 0.0]));
    assert_eq!(result.source_values.unwrap().len(), 2);
    let record = &result.stage_report;
    assert_eq!(record.stage, 11);
    assert_eq!(record.objective_constant, 7.0);
    assert_eq!(record.report.solution.as_ref().unwrap().objective_value, Some(5.0));
    assert_eq!(record.report.fingerprints.model, Some(linear_model_fingerprint(&record.model).unwrap()));
    assert_eq!(record.report.model_mapping, Some(linear_model_mapping(&record.model).unwrap()));
    let mut observed = execution.reports();
    observed[0].model.basic.constraint_names.clear();
    assert!(!execution.reports()[0].model.basic.constraint_names.is_empty());
}

#[test]
fn nonoptimal_lp_incumbent_is_validated_but_never_delivers_pricing() {
    let execution = executor(|model, _, _| incumbent(model, vec![2.0, 1.0]));
    let result = execution.solve(&lp_model(), 0, LinearExecutionMode::Lp).unwrap();
    assert_eq!(result.objective, Some(12.0));
    assert!(result.source_values.is_some());
    assert!(result.original_duals.is_none());
    assert!(!result.stage_report.report.is_optimal());
}

#[test]
fn invalid_lp_certificates_and_reports_do_not_enter_accepted_history() {
    for variant in 0..9 {
        let execution = executor(move |model, _, _| {
            let mut report = certified_lp(model)?;
            match variant {
                0 => report.proof = None,
                1 => report.solution.as_mut().unwrap().dual_solution = Some(vec![0.0; 4]),
                2 => report.solution.as_mut().unwrap().dual_solution = None,
                3 => report.solution.as_mut().unwrap().dual_solution = Some(vec![-1.0]),
                4 => report.fingerprints.model = Some(linear_model_fingerprint(&LinearTriadModel::new("foreign"))?),
                5 => { report.solution.as_mut().unwrap().values.pop(); },
                6 => report.solution.as_mut().unwrap().objective = Some(0.0),
                7 => { report.solution.as_mut().unwrap().stable_values.insert("foreign".into(), 2.0); },
                _ => report.solution.as_mut().unwrap().values[0] = f64::NAN,
            }
            Ok(report)
        });
        assert!(execution.solve(&lp_model(), 0, LinearExecutionMode::Lp).is_err(), "variant {variant}");
        assert!(execution.reports().is_empty());
    }
}

#[test]
fn original_snapshot_integrality_bounds_and_business_rules_cannot_be_bypassed() {
    let mut model = MetaModel::new("integer");
    let x = model.register_auto_variable_with_range::<UInteger>("x", VariableRange::bounded(0.0, 3.0)).unwrap();
    model.add_ge_constraint(&[(x, 1.0)], 2.0, "business").unwrap();
    model.add_linear_objective_input(LinearObjectiveInput::minimize("cost").term(x, 1.0));
    for value in [0.0, 1.5, 4.0, f64::INFINITY] {
        let execution = executor(move |model, _, _| incumbent(model, vec![value]));
        assert!(execution.solve(&model, 0, LinearExecutionMode::Milp).is_err());
        assert!(execution.reports().is_empty());
    }
    let execution = executor(|model, _, _| incumbent(model, vec![2.0]));
    assert!(execution.solve(&model, 0, LinearExecutionMode::Lp).is_err());
    assert_eq!(execution.solve(&model, 1, LinearExecutionMode::Milp).unwrap().objective, Some(2.0));
}

#[test]
fn identity_completion_is_limited_to_empty_proof_free_cancellation() {
    let model = lp_model().linear_model_snapshot().unwrap().compile_flt64().unwrap().model;
    let mut cancelled = SolveReport::builder(ProblemStatus::Unknown, TerminationReason::Cancelled).build().unwrap();
    validate_linear_execution_report(&model, &mut cancelled, 1e-7).unwrap();
    assert!(cancelled.proof.is_none());
    assert!(!cancelled.has_incumbent());
    assert!(validate_linear_execution_report(&LinearTriadModel::new("foreign"), &mut cancelled, 1e-7).is_err());
    let mut timeout = SolveReport::builder(ProblemStatus::Unknown, TerminationReason::TimeLimit).build().unwrap();
    assert!(validate_linear_execution_report(&model, &mut timeout, 1e-7).is_err());
}

#[test]
fn infeasibility_requires_matching_reliable_complete_evidence() {
    for certified in [false, true] {
        let execution = executor(move |model, _, _| {
            let mut report = SolveReport::builder(ProblemStatus::Infeasible, TerminationReason::Completed)
                .fingerprints(SolveFingerprints { model: Some(linear_model_fingerprint(model)?), ..Default::default() })
                .model_mapping(Some(linear_model_mapping(model)?)).build()?;
            if certified { report.proof = Some(SolveProof::infeasibility()); }
            Ok(report)
        });
        assert_eq!(execution.solve(&lp_model(), 0, LinearExecutionMode::Lp).is_ok(), certified);
    }
}

#[test]
fn precancel_timeout_and_limits_never_call_backend_or_invent_incumbents() {
    for cancel in [false, true] {
        let mut execution = executor(|_, _, _| panic!("must not run"));
        execution.options.time_limit = Duration::ZERO;
        execution.options.report_limit = 1;
        if cancel { execution.options.cancellation.cancel(CancellationOrigin::User); }
        let result = execution.solve(&lp_model(), 0, LinearExecutionMode::Lp).unwrap();
        assert_eq!(result.stage_report.report.termination_reason, if cancel { TerminationReason::Cancelled } else { TerminationReason::TimeLimit });
        assert!(result.source_values.is_none());
        assert!(result.objective.is_none());
        assert!(result.original_duals.is_none());
        assert!(result.stage_report.report.proof.is_none());
        assert!(execution.solve(&lp_model(), 1, LinearExecutionMode::Milp).is_err());
    }
}

#[test]
fn later_child_has_independent_completion_and_observes_live_cancellation() {
    let options = LinearExecutionOptions::default();
    let parent = options.cancellation.clone();
    let mut execution = executor(move |model, options, index| {
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
    });
    execution.options = options;
    execution.solve(&lp_model(), 0, LinearExecutionMode::Lp).unwrap();
    let result = execution.solve(&lp_model(), 1, LinearExecutionMode::Milp).unwrap();
    assert_eq!(result.stage_report.report.termination_reason, TerminationReason::Cancelled);
    assert!(execution.is_cancelled());
    assert_eq!(execution.reports().len(), 2);
}

#[test]
fn completed_child_report_is_not_rewritten_by_late_parent_cancellation() {
    let options = LinearExecutionOptions::default();
    let parent = options.cancellation.clone();
    let mut execution = executor(move |model, options, index| {
        assert_eq!(index, 0);
        let child = options.cancellation_handle.unwrap();
        child.mark_completed();
        parent.cancel(CancellationOrigin::User);
        assert!(child.is_cancelled());
        assert!(!child.cancellation_preceded_completion());
        certified_lp(model)
    });
    execution.options = options;
    let completed = execution.solve(&lp_model(), 0, LinearExecutionMode::Lp).unwrap();
    assert!(completed.stage_report.report.is_optimal());
    assert_eq!(completed.stage_report.report.termination_reason, TerminationReason::Completed);
    assert_eq!(completed.objective, Some(12.0));
    let cancelled = execution.solve(&lp_model(), 1, LinearExecutionMode::Milp).unwrap();
    assert_eq!(cancelled.stage_report.report.termination_reason, TerminationReason::Cancelled);
    assert!(cancelled.source_values.is_none());
    assert_eq!(execution.reports().len(), 2);
}

#[test]
fn one_total_budget_decreases_across_children_and_includes_time_between_calls() {
    let limits = Arc::new(Mutex::new(Vec::new()));
    let observed = limits.clone();
    let execution = executor(move |model, options, _| {
        lock(&observed).push(options.time_limit.unwrap());
        terminal(model, TerminationReason::TimeLimit)
    });
    execution.solve(&lp_model(), 0, LinearExecutionMode::Lp).unwrap();
    std::thread::sleep(Duration::from_millis(10));
    execution.solve(&lp_model(), 1, LinearExecutionMode::Milp).unwrap();
    let limits = lock(&limits);
    assert!(limits[1] < limits[0]);
    assert!(limits[0] <= Duration::from_secs(60));
}

#[test]
fn concurrent_calls_fail_promptly_without_blocking_history_observation() {
    let started = Arc::new(std::sync::Barrier::new(2));
    let release = Arc::new(std::sync::Barrier::new(2));
    let (worker_started, worker_release) = (started.clone(), release.clone());
    let execution = Arc::new(executor(move |model, _, _| {
        worker_started.wait(); worker_release.wait();
        terminal(model, TerminationReason::TimeLimit)
    }));
    let worker_execution = execution.clone();
    let worker = std::thread::spawn(move || worker_execution.solve(&lp_model(), 0, LinearExecutionMode::Lp));
    started.wait();
    let concurrent = execution.solve(&lp_model(), 1, LinearExecutionMode::Milp);
    let reports = execution.reports();
    release.wait();
    worker.join().unwrap().unwrap();
    assert!(concurrent.unwrap_err().to_string().contains("already executing"));
    assert!(reports.is_empty());
    assert_eq!(execution.reports().len(), 1);
}

#[test]
fn invalid_options_fail_before_any_execution() {
    for options in [
        LinearExecutionOptions { tolerance: f64::NAN, ..Default::default() },
        LinearExecutionOptions { tolerance: -1.0, ..Default::default() },
        LinearExecutionOptions { tolerance: 0.5, ..Default::default() },
        LinearExecutionOptions { report_limit: 0, ..Default::default() },
        LinearExecutionOptions { report_limit: 100_001, ..Default::default() },
    ] {
        let solver = Arc::new(FunctionSolver { handler: |_: &LinearTriadModel, _: &SolveOptions<'_>, _: usize| panic!("invalid options"), calls: AtomicUsize::new(0) });
        assert!(LinearReportExecutor::<u32>::new(solver, options).is_err());
    }
}

#[test]
fn raw_progress_and_callback_errors_are_forwarded_without_fallback() {
    use ospf_rust_core::solver::{ProgressValue, SolveProgressSnapshot, SolveStage};

    for fail in [false, true] {
        let observed = Arc::new(AtomicUsize::new(0));
        let events = observed.clone();
        let mut execution = executor(|model, options, _| {
            let snapshot = SolveProgressSnapshot::new(
                "actual-child", SolveStage::Solving, vec!["backend".into()], ProgressValue::Indeterminate,
                ProgressValue::Indeterminate, Duration::ZERO, None, None, None, false,
            )?;
            options.progress_reporter.unwrap()(&snapshot)?;
            terminal(model, TerminationReason::TimeLimit)
        });
        execution.options.progress = Some(Arc::new(move |snapshot| {
            assert_eq!(snapshot.attempt_id, "actual-child");
            assert_eq!(snapshot.stage_path, vec!["backend"]);
            events.fetch_add(1, Ordering::SeqCst);
            if fail { Err(invalid("callback failed")) } else { Ok(()) }
        }));
        let result = execution.solve(&lp_model(), 0, LinearExecutionMode::Lp);
        assert_eq!(result.is_err(), fail);
        assert_eq!(observed.load(Ordering::SeqCst), 1);
        assert_eq!(execution.reports().len(), if fail { 0 } else { 1 });
    }
}

#[test]
fn backend_errors_do_not_create_reports_or_fallback_solutions() {
    let execution = executor(|_, _, _| Err(invalid("backend failed")));
    assert!(execution.solve(&lp_model(), 0, LinearExecutionMode::Lp).is_err());
    assert!(execution.reports().is_empty());
}
