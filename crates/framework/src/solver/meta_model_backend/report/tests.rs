use std::sync::atomic::{AtomicUsize, Ordering};
use ospf_rust_core::model::{LinearObjectiveInput, intermediate::LinearTriadModel};
use ospf_rust_core::solver::{ProblemStatus, SolutionPresence, SolveFingerprints, SolveOptions, SolveProof, SolveReport, SolveSolution,
    SolverCapability, SolverInfo, SolverOutput, TerminationReason, linear_model_fingerprint, linear_model_mapping};
use ospf_rust_core::variable::{Continuous, VariableRange};
use super::*;

struct FunctionSolver<F> { handler: F, calls: AtomicUsize }

impl<F: Send + Sync> SolverInfo for FunctionSolver<F> {
    fn name(&self) -> &str { "report-meta-model-contract" }
    fn capabilities(&self) -> Vec<SolverCapability> { vec![SolverCapability::Linear] }
}

impl<F> LinearSolver for FunctionSolver<F>
where
    F: Fn(&LinearTriadModel, &SolveOptions<'_>, usize) -> ospf_rust_core::Result<SolveReport<f64>> + Send + Sync,
{
    fn solve_linear(&self, _: &LinearTriadModel) -> ospf_rust_core::Result<SolverOutput> { panic!("actual reports required") }
    fn solve_linear_report_with_options(&self, model: &LinearTriadModel, options: &SolveOptions<'_>) -> ospf_rust_core::Result<SolveReport<f64>> {
        (self.handler)(model, options, self.calls.fetch_add(1, Ordering::SeqCst))
    }
}

fn backend(handler: impl Fn(&LinearTriadModel, &SolveOptions<'_>, usize) -> ospf_rust_core::Result<SolveReport<f64>> + Send + Sync + 'static) -> ReportMetaModelBackend {
    ReportMetaModelBackend::new(Arc::new(FunctionSolver { handler, calls: AtomicUsize::new(0) }), Default::default()).unwrap()
}

fn model() -> MetaModel<f64> {
    let mut model = MetaModel::new("meta-report");
    let x = model.register_auto_variable_with_range::<Continuous>("x", VariableRange::bounded(0.0, 10.0)).unwrap();
    let y = model.register_auto_variable_with_range::<Continuous>("y", VariableRange::bounded(0.0, 10.0)).unwrap();
    model.add_ge_constraint(&[(x, 1.0)], 2.0, "lower").unwrap();
    model.add_eq_constraint(&[(y, 1.0)], 1.0, "equal").unwrap();
    model.add_le_constraint(&[(x, 1.0)], 5.0, "upper").unwrap();
    model.add_linear_objective_input(LinearObjectiveInput::minimize("cost").term(x, 1.0).term(y, 3.0).constant(7.0)); model
}

fn report(model: &LinearTriadModel, optimal: bool) -> ospf_rust_core::Result<SolveReport<f64>> {
    let mut solution = SolveSolution::vector(vec![2.0, 1.0]); solution.objective_value = Some(5.0);
    if optimal { solution.dual_solution = Some(vec![-1.0, 0.0, -3.0, 0.0]); }
    let mut report = SolveReport::builder(ProblemStatus::Feasible, if optimal { TerminationReason::Completed } else { TerminationReason::TimeLimit }).solution(solution)
        .fingerprints(SolveFingerprints { model: Some(linear_model_fingerprint(model)?), ..Default::default() }).model_mapping(Some(linear_model_mapping(model)?)).build()?;
    if optimal { report.proof = Some(SolveProof::optimality()); report.solution_presence = SolutionPresence::Optimal; }
    report.statistics.nodes = Some(usize::MAX); report.validate()?; Ok(report)
}

fn terminal(model: &LinearTriadModel) -> ospf_rust_core::Result<SolveReport<f64>> {
    SolveReport::builder(ProblemStatus::Unknown, TerminationReason::TimeLimit)
        .fingerprints(SolveFingerprints { model: Some(linear_model_fingerprint(model)?), ..Default::default() }).model_mapping(Some(linear_model_mapping(model)?)).build()
}

#[test]
fn compatibility_uses_original_token_and_row_order_but_keeps_actual_model_report_and_constant_separate() {
    let backend = backend(|model, _, _| report(model, true)); let model = model();
    let solved = backend.solve_rmp(&model, &Default::default()).unwrap();
    assert_eq!(solved.primal_solution, vec![2.0, 1.0]); assert_eq!(solved.dual_solution, vec![1.0, 3.0, 0.0]); assert_eq!(solved.objective, Some(12.0));
    assert_eq!(solved.info["backend_kind"], "linear_report");
    let mut reports = backend.reports(); assert_eq!(reports.len(), 1); assert_eq!(reports[0].stage, MetaModelStage::RestrictedMaster);
    assert_eq!(reports[0].objective_constant, 7.0); assert_eq!(reports[0].report.statistics.nodes, Some(usize::MAX));
    assert_eq!(reports[0].report.solution.as_ref().unwrap().objective_value, Some(5.0));
    assert_eq!(reports[0].report.fingerprints.model, Some(linear_model_fingerprint(&reports[0].model).unwrap()));
    reports[0].model.basic.constraint_names.clear(); assert!(!backend.reports()[0].model.basic.constraint_names.is_empty());
}

#[test]
fn uncertified_lp_stops_legacy_pricing_without_discarding_the_report_or_using_fake_duals() {
    let backend = backend(|model, _, _| report(model, false));
    assert!(backend.solve_rmp(&model(), &Default::default()).unwrap_err().contains("no pricing fallback"));
    assert_eq!(backend.reports().len(), 1); assert!(backend.reports()[0].report.has_incumbent());
    let native = backend.execute(&model(), MetaModelStage::RestrictedMaster).unwrap();
    assert_eq!(native.objective, Some(12.0)); assert!(native.source_values.is_some()); assert!(native.original_duals.is_none());
}

#[test]
fn no_incumbent_stops_legacy_final_without_losing_native_terminal_reports() {
    let backend = backend(|model, _, _| terminal(model));
    assert!(backend.solve_final(&model(), &Default::default()).unwrap_err().contains("no vector fallback"));
    let native = backend.execute(&model(), MetaModelStage::FinalMilp).unwrap();
    assert_eq!(native.stage_report.report.termination_reason, TerminationReason::TimeLimit); assert!(native.source_values.is_none());
    assert_eq!(backend.reports().len(), 2);
}

#[test]
fn actual_nonoptimal_final_stage_incumbents_are_retained_without_promoting_proofs_or_duals() {
    let backend = backend(|model, _, _| report(model, false));
    let solved = backend.solve_final(&model(), &Default::default()).unwrap();
    assert_eq!(solved.primal_solution, vec![2.0, 1.0]); assert!(solved.dual_solution.is_empty());
    let reports = backend.reports(); assert!(!reports[0].report.is_optimal()); assert!(reports[0].report.proof.is_none());
    assert_eq!(reports[0].stage, MetaModelStage::FinalMilp);
}

#[test]
fn shared_clones_keep_one_budget_history_and_independent_child_completion() {
    let options = LinearExecutionOptions::default(); let parent = options.cancellation.clone();
    let backend = ReportMetaModelBackend::new(Arc::new(FunctionSolver { handler: move |model: &LinearTriadModel, options: &SolveOptions<'_>, index| {
        assert_eq!(index, 0); options.cancellation_handle.unwrap().mark_completed(); parent.cancel("after-first-child"); report(model, true)
    }, calls: AtomicUsize::new(0) }), options).unwrap();
    let next = backend.clone(); assert!(backend.solve_rmp(&model(), &Default::default()).is_ok());
    assert!(next.solve_final(&model(), &Default::default()).is_err());
    let history = backend.reports(); assert_eq!(history.len(), 2); assert_eq!(next.reports().len(), 2);
    assert!(history[0].report.is_optimal()); assert_eq!(history[1].report.termination_reason, TerminationReason::Cancelled);
    assert!(backend.is_cancelled()); assert!(next.remaining_time() <= Duration::from_secs(60));
}

#[test]
fn malformed_evidence_and_backend_errors_never_become_accepted_reports() {
    let invalid = backend(|model, _, _| { let mut report = report(model, true)?; report.fingerprints.model = Some(linear_model_fingerprint(&LinearTriadModel::new("foreign"))?); Ok(report) });
    assert!(invalid.solve_rmp(&model(), &Default::default()).is_err()); assert!(invalid.reports().is_empty());
    let failed = backend(|_, _, _| Err(ospf_rust_core::SolverError::NumericalError("actual solver failed".into()).into()));
    assert!(failed.solve_final(&model(), &Default::default()).unwrap_err().contains("actual solver failed")); assert!(failed.reports().is_empty());
}

#[test]
fn rmp_rejects_integer_domains_before_solving_and_final_preserves_them() {
    use ospf_rust_core::variable::Binary;
    let backend = backend(|model, _, index| {
        assert_eq!(index, 0); assert!(model.has_integer_variables());
        let mut solution = SolveSolution::vector(vec![1.0]); solution.objective_value = Some(1.0);
        SolveReport::builder(ProblemStatus::Feasible, TerminationReason::TimeLimit).solution(solution)
            .fingerprints(SolveFingerprints { model: Some(linear_model_fingerprint(model)?), ..Default::default() }).model_mapping(Some(linear_model_mapping(model)?)).build()
    });
    let mut model = MetaModel::new("binary-final"); let x = model.register_auto_variable::<Binary>("x").unwrap();
    model.add_linear_objective_input(LinearObjectiveInput::minimize("cost").term(x, 1.0));
    assert!(backend.solve_rmp(&model, &Default::default()).is_err()); assert!(backend.reports().is_empty());
    let final_result = backend.solve_final(&model, &Default::default()).unwrap(); assert_eq!(final_result.primal_solution, vec![1.0]);
    assert_eq!(backend.reports().len(), 1); assert!(backend.reports()[0].model.has_integer_variables());
}

#[test]
fn empty_actual_vectors_and_rowless_lp_reports_never_become_legacy_fallback_sentinels() {
    let backend = backend(|model, _, _| {
        let mut solution = SolveSolution::vector(vec![0.0; model.num_variables()]);
        solution.objective_value = Some(0.0); solution.dual_solution = Some(Vec::new());
        let mut report = SolveReport::builder(ProblemStatus::Feasible, TerminationReason::Completed).solution(solution)
            .fingerprints(SolveFingerprints { model: Some(linear_model_fingerprint(model)?), ..Default::default() })
            .model_mapping(Some(linear_model_mapping(model)?)).build()?;
        report.proof = Some(SolveProof::optimality()); report.solution_presence = SolutionPresence::Optimal;
        report.validate()?; Ok(report)
    });
    let mut model = MetaModel::new("empty-final");
    let actual = backend.execute(&model, MetaModelStage::FinalMilp).unwrap();
    assert!(actual.source_values.unwrap().is_empty()); assert_eq!(actual.objective, Some(0.0));
    assert!(backend.solve_final(&model, &Default::default()).unwrap_err().contains("empty actual primal"));
    model.register_auto_variable_with_range::<Continuous>("x", VariableRange::bounded(0.0, 1.0)).unwrap();
    assert!(backend.execute(&model, MetaModelStage::RestrictedMaster).unwrap_err().to_string().contains("empty dual solution"));
    assert!(backend.solve_rmp(&model, &Default::default()).unwrap_err().contains("empty dual solution"));
    assert_eq!(backend.reports().len(), 2);
}
