use std::sync::atomic::{AtomicUsize, Ordering};
use ospf_rust_quantities::quantity::Quantity;
use ospf_rust_quantities::unit::derived::Meter;
use ospf_rust_core::model::intermediate::LinearTriadModel;
use ospf_rust_core::solver::{ProblemStatus, SolutionPresence, SolveFingerprints, SolveOptions, SolveProof, SolveReport, SolveSolution, SolverCapability, SolverInfo, SolverOutput, TerminationReason, linear_model_fingerprint, linear_model_mapping};
use ospf_rust_framework_geometry2d::{PackingShape2, Placement2, QuantityPoint2, Sheet2Shape, rectangle_shape2};
use crate::domain::item::model::{Layer, SheetType, SheetTypeId};
use crate::domain::layer_assignment::{Bpp2dAssignmentDemandEntry, LayerAssignmentContext, LayerAssignmentModelInput, layer_reduced_cost};
use super::*;

struct FunctionSolver<F> { handler: F, calls: AtomicUsize }
impl<F: Send + Sync> SolverInfo for FunctionSolver<F> {
    fn name(&self) -> &str { "bpp2d-contract" }
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

fn backend(handler: impl Fn(&LinearTriadModel, &SolveOptions<'_>, usize) -> ospf_rust_core::Result<SolveReport<f64>> + Send + Sync + 'static) -> Bpp2dNativeBackend {
    Bpp2dNativeBackend::new(Arc::new(FunctionSolver { handler, calls: AtomicUsize::new(0) }), Default::default()).unwrap()
}

fn column() -> Layer<f64, Meter> {
    let meters = |value| Quantity::<f64, Meter>::new_ct(value);
    let sheet = SheetType::new(meters(10.0), meters(8.0), meters(0.0), SheetTypeId::new("s"));
    let shape = PackingShape2::Rectangle(rectangle_shape2(meters(2.0), meters(2.0)).unwrap());
    let units = (0..2).map(|index| Placement2::new(shape.clone(), QuantityPoint2 { x: meters(index as f64 * 2.0), y: meters(0.0) }).with_item_id("a")).collect();
    Layer::new(0, "test", Some(sheet), Sheet2Shape::new(meters(4.0), meters(2.0))).with_units(units)
}

fn master(
    layers: &[Layer<f64, Meter>],
    demands: &[Bpp2dAssignmentDemandEntry],
    stage: LayerAssignmentStage,
) -> (MetaModel<f64>, LayerAssignmentModel) {
    let mut model = MetaModel::new("bpp2d-master");
    let assignment = LayerAssignmentContext::default().register_model(&mut model, LayerAssignmentModelInput { layers, demands, stage }).unwrap();
    (model, assignment)
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

#[test]
fn checked_lp_prices_and_integer_copies_reuse_the_same_backend() {
    let backend = backend(|model, _, index| {
        let mut report = incumbent(model, vec![if index == 0 { 1.5 } else { 2.0 }])?;
        if index == 0 {
            report.proof = Some(SolveProof::optimality());
            report.solution_presence = SolutionPresence::Optimal;
            report.solution.as_mut().unwrap().dual_solution = Some(vec![-40.0]);
        }
        Ok(report)
    });
    let demands = [Bpp2dAssignmentDemandEntry::item_amount("a", 3)];
    let (lp, assignment) = master(&[column()], &demands, LayerAssignmentStage::Rmp);
    let result = backend.solve_master(&lp, &assignment).unwrap();
    assert_eq!(result.copies, Some(vec![1.5]));
    assert_eq!(result.objective, Some(120.0));
    assert!(result.deficits.is_none());
    assert_eq!(layer_reduced_cost(&column(), &demands, result.shadow_prices.as_ref().unwrap(), 1.0), 0.0);
    let (integer, assignment) = master(&[column()], &demands, LayerAssignmentStage::Integer);
    let result = backend.solve_master(&integer, &assignment).unwrap();
    assert_eq!(result.copies, Some(vec![2.0]));
    assert!(result.shadow_prices.is_none());
    assert_eq!(backend.reports().len(), 2);
}

#[test]
fn empty_pool_phase_one_retains_actual_shortages_and_zero_cost_pricing() {
    let backend = backend(|model, _, _| {
        let mut report = incumbent(model, vec![3.0])?;
        report.proof = Some(SolveProof::optimality());
        report.solution_presence = SolutionPresence::Optimal;
        report.solution.as_mut().unwrap().dual_solution = Some(vec![-1.0 / 3.0]);
        Ok(report)
    });
    let demands = [Bpp2dAssignmentDemandEntry::item_amount("a", 3)];
    let (phase, assignment) = master(&[], &demands, LayerAssignmentStage::FeasibilityLp);
    let result = backend.solve_master(&phase, &assignment).unwrap();
    assert_eq!(result.copies, Some(Vec::new()));
    assert_eq!(result.deficits, Some(vec![3.0]));
    assert_eq!(result.objective, Some(1.0));
    assert!(layer_reduced_cost(&column(), &demands, result.shadow_prices.as_ref().unwrap(), 0.0) < 0.0);
}

#[test]
fn nonoptimal_lp_has_no_shadow_prices_and_never_becomes_a_pricing_success() {
    let backend = backend(|model, _, _| incumbent(model, vec![1.5]));
    let (model, assignment) = master(&[column()], &[Bpp2dAssignmentDemandEntry::item_amount("a", 3)], LayerAssignmentStage::Rmp);
    let result = backend.solve_master(&model, &assignment).unwrap();
    assert_eq!(result.copies, Some(vec![1.5]));
    assert!(result.shadow_prices.is_none());
    assert!(!result.stage_report.report.is_optimal());
}

#[test]
fn foreign_registration_and_unsafe_values_fail_without_replacement_output() {
    let (model, assignment) = master(&[column()], &[Bpp2dAssignmentDemandEntry::item_amount("a", 3)], LayerAssignmentStage::Integer);
    let backend = backend(|_, _, _| panic!("foreign assignment must fail before execution"));
    assert!(backend.solve_master(&MetaModel::new("foreign"), &assignment).is_err());
    assert!(backend.reports().is_empty());
    for value in [1.0, 1.5, f64::NAN, f64::INFINITY] {
        let solver = Arc::new(FunctionSolver { handler: move |model: &LinearTriadModel, _: &SolveOptions<'_>, _: usize| incumbent(model, vec![value]), calls: AtomicUsize::new(0) });
        let backend = Bpp2dNativeBackend::new(solver, Default::default()).unwrap();
        assert!(backend.solve_master(&model, &assignment).is_err());
        assert!(backend.reports().is_empty());
    }
}

#[test]
fn extra_business_rules_are_revalidated_before_extracting_copies() {
    let backend = backend(|model, _, _| incumbent(model, vec![2.0]));
    let (mut model, assignment) = master(&[column()], &[Bpp2dAssignmentDemandEntry::item_amount("a", 3)], LayerAssignmentStage::Integer);
    model.add_ge_constraint(&[(assignment.copy_variables()[0], 1.0)], 3.0, "business_rule").unwrap();
    assert!(backend.solve_master(&model, &assignment).is_err());
    assert!(backend.reports().is_empty());
}

#[test]
fn timeout_and_precancel_preserve_terminal_reports_without_fabricated_domain_values() {
    for cancelled in [false, true] {
        let options = Bpp2dExecutionOptions { time_limit: Duration::ZERO, ..Default::default() };
        if cancelled { options.cancellation.cancel(ospf_rust_core::solver::CancellationOrigin::User); }
        let solver = Arc::new(FunctionSolver { handler: |_: &LinearTriadModel, _: &SolveOptions<'_>, _: usize| panic!("must not solve"), calls: AtomicUsize::new(0) });
        let backend = Bpp2dNativeBackend::new(solver, options).unwrap();
        let (model, assignment) = master(&[], &[Bpp2dAssignmentDemandEntry::item_amount("a", 3)], LayerAssignmentStage::FeasibilityLp);
        let result = backend.solve_master(&model, &assignment).unwrap();
        assert!(result.copies.is_none() && result.deficits.is_none() && result.objective.is_none() && result.shadow_prices.is_none());
        assert_eq!(result.stage_report.report.termination_reason, if cancelled { TerminationReason::Cancelled } else { TerminationReason::TimeLimit });
        assert_eq!(backend.reports().len(), 1);
    }
}

#[cfg(any(feature = "scip", feature = "scip-bundled", feature = "scip-from-source", feature = "gurobi10", feature = "gurobi11", feature = "gurobi12"))]
fn real_backend_contract(solver: Arc<dyn LinearSolver>) {
    use ospf_rust_core::model::LinearObjectiveInput;
    use ospf_rust_core::solver::{require_infeasibility_certificate_for_model, require_optimal_lp_certificate_for_linear_model};
    use ospf_rust_framework::solver::remote::{decode_compiled_linear_model, encode_compiled_linear_model};
    use crate::domain::layer_assignment::Bpp2dAssignmentDemandValue;

    let progress = Arc::new(AtomicUsize::new(0));
    let observed = progress.clone();
    let backend = Bpp2dNativeBackend::new(solver, Bpp2dExecutionOptions {
        progress: Some(Arc::new(move |_| { observed.fetch_add(1, Ordering::SeqCst); Ok(()) })), ..Default::default()
    }).unwrap();
    let demands = [Bpp2dAssignmentDemandEntry::item_amount("a", 3), Bpp2dAssignmentDemandEntry::item_area("a", 12.0)];
    let (phase, assignment) = master(&[], &demands, LayerAssignmentStage::FeasibilityLp);
    let phase = backend.solve_master(&phase, &assignment).unwrap();
    assert_eq!(phase.copies, Some(Vec::new()));
    assert_eq!(phase.deficits, Some(vec![3.0, 12.0]));
    assert_eq!(phase.objective, Some(2.0));
    assert!(layer_reduced_cost(&column(), &demands, phase.shadow_prices.as_ref().unwrap(), 0.0) < 0.0);
    for stage in [LayerAssignmentStage::FeasibilityLp, LayerAssignmentStage::Rmp, LayerAssignmentStage::Integer] {
        let (model, assignment) = master(&[column()], &demands, stage);
        let result = backend.solve_master(&model, &assignment).unwrap();
        assert!(result.stage_report.report.is_optimal());
        assert_eq!(result.copies, Some(vec![if stage == LayerAssignmentStage::Integer { 2.0 } else { 1.5 }]));
        if stage == LayerAssignmentStage::FeasibilityLp {
            assert!(result.objective.unwrap().abs() < 1e-7);
            assert_eq!(result.deficits, Some(vec![0.0, 0.0]));
        }
        if stage == LayerAssignmentStage::Rmp {
            assert!((result.objective.unwrap() - 120.0).abs() < 1e-7);
            let prices = result.shadow_prices.as_ref().unwrap();
            assert_eq!(prices.len(), 2);
            assert!(layer_reduced_cost(&column(), &demands, prices, 1.0).abs() < 1e-7);
        }
    }
    let (mut model, assignment) = master(&[column()], &demands, LayerAssignmentStage::Integer);
    model.add_ge_constraint(&[(assignment.copy_variables()[0], 1.0)], 3.0, "business_rule").unwrap();
    model.add_linear_objective_input(LinearObjectiveInput::minimize("constant").constant(7.0));
    let result = backend.solve_master(&model, &assignment).unwrap();
    assert_eq!(result.copies, Some(vec![3.0]));
    assert!((result.objective.unwrap() - 247.000003).abs() < 1e-7);
    assert_eq!(result.stage_report.objective_constant, 7.0);
    let (model, assignment) = master(&[column()], &[Bpp2dAssignmentDemandEntry::item_amount("a", 3).with_upper_bound(Bpp2dAssignmentDemandValue::Amount(3))], LayerAssignmentStage::Integer);
    let result = backend.solve_master(&model, &assignment).unwrap();
    assert_eq!(result.stage_report.report.problem_status, ProblemStatus::Infeasible);
    assert!(result.copies.is_none() && result.shadow_prices.is_none());
    require_infeasibility_certificate_for_model(&result.stage_report.report, &linear_model_fingerprint(&result.stage_report.model).unwrap()).unwrap();
    assert!(progress.load(Ordering::SeqCst) > 0);
    assert_eq!(backend.reports().len(), 6);
    for stage in backend.reports() {
        let encoded = encode_compiled_linear_model(&stage.model).unwrap();
        let restored = decode_compiled_linear_model(&encoded).unwrap();
        assert_eq!(stage.report.fingerprints.model, Some(linear_model_fingerprint(&restored).unwrap()));
        assert_eq!(stage.report.model_mapping, Some(linear_model_mapping(&restored).unwrap()));
        if stage.stage != LayerAssignmentStage::Integer && stage.report.is_optimal() {
            require_optimal_lp_certificate_for_linear_model(&stage.report, &restored).unwrap();
        }
    }
}

#[cfg(any(feature = "scip", feature = "scip-bundled", feature = "scip-from-source"))]
#[test]
fn real_scip_report_driven_masters() {
    use ospf_rust_core::solver::solvers::scip::{SCIPConfig, SCIPSolver};
    real_backend_contract(Arc::new(SCIPSolver::with_config(SCIPConfig::default().with_output(false).with_lp_subproblem_defaults())));
}

#[cfg(any(feature = "gurobi10", feature = "gurobi11", feature = "gurobi12"))]
#[test]
fn real_gurobi_report_driven_masters() {
    use ospf_rust_core::solver::solvers::gurobi::{GurobiConfig, GurobiSolver};
    real_backend_contract(Arc::new(GurobiSolver::with_config(GurobiConfig::default().with_output(false))));
}
