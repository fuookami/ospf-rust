use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use ospf_rust_quantities::quantity::Quantity;
use ospf_rust_quantities::unit::derived::Meter;
use ospf_rust_core::model::intermediate::LinearTriadModel;
use ospf_rust_core::solver::{CancellationOrigin, ProblemStatus, SolutionPresence, SolveFingerprints, SolveOptions, SolveProof, SolveReport, SolveSolution, SolverCapability, SolverInfo, SolverOutput, TerminationReason, linear_model_fingerprint, linear_model_mapping};
use ospf_rust_framework_geometry2d::{PackingShape2, Placement2, QuantityPoint2, rectangle_shape2};
use crate::domain::material::{CuttingPlan2, DemandMode2, DemandValue2, Material2, MaterialId2, Pattern2, PatternId2, ProductId2};
use crate::domain::produce::{Csp2dDemandShadowPriceKey, Csp2dMasterDemand, Csp2dMasterProblem, Csp2dPatternAssignment, ProduceContext, ProduceModelInput};
use crate::infrastructure::{CuttingProcess2, Region2};
use super::*;

struct Fake<F>(F);
impl<F: Send + Sync> SolverInfo for Fake<F> {
    fn name(&self) -> &str { "csp2d-contract" }
    fn capabilities(&self) -> Vec<SolverCapability> { vec![SolverCapability::Linear] }
}
impl<F> LinearSolver for Fake<F>
where F: Fn(&LinearTriadModel, &SolveOptions<'_>) -> ospf_rust_core::Result<SolveReport<f64>> + Send + Sync {
    fn solve_linear(&self, _: &LinearTriadModel) -> ospf_rust_core::Result<SolverOutput> { panic!("report path required") }
    fn solve_linear_report_with_options(&self, model: &LinearTriadModel, options: &SolveOptions<'_>) -> ospf_rust_core::Result<SolveReport<f64>> { (self.0)(model, options) }
}

fn meters(value: f64) -> Quantity<f64, Meter> { Quantity::new_ct(value) }

fn master(empty: bool) -> Csp2dMasterProblem {
    let plan = CuttingPlan2::from_pattern(Pattern2::new(PatternId2::new("p"), Material2::sheet(MaterialId2::new("s"), "s", meters(4.0), meters(2.0), 10),
        Region2::new(meters(0.0), meters(0.0), meters(4.0), meters(2.0)), (0..2).map(|index| Placement2::new(
            PackingShape2::Rectangle(rectangle_shape2(meters(1.0), meters(1.0)).unwrap()), QuantityPoint2 { x: meters(index as f64), y: meters(0.0) }).with_item_id("a")).collect(), CuttingProcess2::Freeform));
    Csp2dMasterProblem { assignment: Csp2dPatternAssignment { plans: if empty { Vec::new() } else { vec![plan] }, usage_upper_bounds: if empty { Vec::new() } else { vec![1] } },
        demands: vec![Csp2dMasterDemand::new(ProductId2::new("a"), DemandValue2::Amount(3))], material_availability: BTreeMap::from([("s".into(), 10)]) }
}

fn registered(master: &Csp2dMasterProblem, stage: ProduceStage) -> (MetaModel<f64>, ProduceModel) {
    let mut model = MetaModel::new("csp2d-report");
    let registered = ProduceContext::default().register_model(&mut model, ProduceModelInput { master, stage, kerf: &meters(0.0) }).unwrap();
    (model, registered)
}

fn incumbent(model: &LinearTriadModel, values: Vec<f64>) -> ospf_rust_core::Result<SolveReport<f64>> {
    let objective = model.c.iter().zip(&values).map(|(cost, value)| cost * value).sum();
    let mut solution = SolveSolution::vector(values); solution.objective_value = Some(objective);
    SolveReport::builder(ProblemStatus::Feasible, TerminationReason::Completed).solution(solution)
        .fingerprints(SolveFingerprints { model: Some(linear_model_fingerprint(model)?), ..Default::default() })
        .model_mapping(Some(linear_model_mapping(model)?)).build()
}

fn optimal_lp(model: &LinearTriadModel, values: Vec<f64>, duals: Vec<f64>) -> ospf_rust_core::Result<SolveReport<f64>> {
    let mut report = incumbent(model, values)?;
    report.proof = Some(SolveProof::optimality()); report.solution_presence = SolutionPresence::Optimal;
    report.solution.as_mut().unwrap().dual_solution = Some(duals);
    Ok(report)
}

#[test]
fn actual_models_lp_certificates_and_integer_incumbents_share_explicit_backend_without_rounding() {
    let calls = Arc::new(AtomicUsize::new(0)); let observed = calls.clone();
    let solver = Fake(move |model: &LinearTriadModel, _: &SolveOptions<'_>| {
        if observed.fetch_add(1, Ordering::SeqCst) == 0 { optimal_lp(model, vec![1.5], vec![-4.0, 0.0]) }
        else { incumbent(model, vec![2.0]) }
    });
    let backend = Csp2dNativeBackend::new(Arc::new(solver), Default::default()).unwrap();
    let (model, registration) = registered(&master(false), ProduceStage::Rmp);
    let result = backend.solve_master(&model, &registration).unwrap();
    assert_eq!(result.usages, Some(vec![1.5])); assert_eq!(result.objective, Some(12.0));
    let prices = result.shadow_prices.unwrap();
    assert_eq!(prices.demands[&Csp2dDemandShadowPriceKey { product_id: "a".into(), mode: DemandMode2::Amount }], 4.0);
    assert_eq!(prices.materials["s"], 0.0);
    let (model, registration) = registered(&master(false), ProduceStage::Integer);
    let result = backend.solve_master(&model, &registration).unwrap();
    assert_eq!(result.usages, Some(vec![2.0])); assert!(result.shadow_prices.is_none());
    assert_eq!(calls.load(Ordering::SeqCst), 2); assert_eq!(backend.reports().len(), 2);
}

#[test]
fn empty_phase_one_preserves_actual_shortages_and_original_unit_prices() {
    let backend = Csp2dNativeBackend::new(Arc::new(Fake(|model: &LinearTriadModel, _: &SolveOptions<'_>| optimal_lp(model, vec![3.0], vec![-1.0 / 3.0, 0.0]))), Default::default()).unwrap();
    let (model, registration) = registered(&master(true), ProduceStage::FeasibilityLp);
    let result = backend.solve_master(&model, &registration).unwrap();
    assert_eq!(result.usages, Some(Vec::new())); assert_eq!(result.deficits, Some(vec![3.0])); assert_eq!(result.objective, Some(1.0));
    assert!(result.shadow_prices.is_some());
}

#[test]
fn invalid_identity_incomplete_vectors_bad_certificates_and_extra_rules_have_no_fallback() {
    for variant in 0..6 {
        let backend = Csp2dNativeBackend::new(Arc::new(Fake(move |model: &LinearTriadModel, _: &SolveOptions<'_>| {
            let mut report = incumbent(model, match variant { 1 => Vec::new(), 2 => vec![1.5], 3 => vec![11.0], _ => vec![2.0] })?;
            if variant == 0 { report.fingerprints.model = None; }
            if variant == 4 { report.proof = Some(SolveProof::optimality()); report.solution_presence = SolutionPresence::Optimal; report.solution.as_mut().unwrap().dual_solution = Some(vec![0.0, 0.0]); }
            Ok(report)
        })), Default::default()).unwrap();
        let (mut model, registration) = registered(&master(false), if variant == 4 { ProduceStage::Rmp } else { ProduceStage::Integer });
        if variant == 5 { model.add_ge_constraint(&[(0, 1.0)], 3.0, "extra").unwrap(); }
        assert!(backend.solve_master(&model, &registration).is_err(), "variant {variant}");
        assert!(backend.reports().is_empty());
    }
}

#[test]
fn nonoptimal_lp_keeps_its_incumbent_but_never_delivers_pricing_and_late_cancel_does_not_rewrite_final() {
    let backend = Csp2dNativeBackend::new(Arc::new(Fake(|model: &LinearTriadModel, _: &SolveOptions<'_>| {
        let mut report = incumbent(model, vec![1.5])?; report.termination_reason = TerminationReason::TimeLimit; Ok(report)
    })), Default::default()).unwrap();
    let (model, registration) = registered(&master(false), ProduceStage::Rmp);
    let result = backend.solve_master(&model, &registration).unwrap();
    assert_eq!(result.usages, Some(vec![1.5])); assert!(result.shadow_prices.is_none());
    let options = Csp2dExecutionOptions::default(); let handle = options.cancellation.clone();
    let backend = Csp2dNativeBackend::new(Arc::new(Fake(move |model: &LinearTriadModel, _: &SolveOptions<'_>| {
        handle.cancel(CancellationOrigin::User); incumbent(model, vec![2.0])
    })), options).unwrap();
    let (model, registration) = registered(&master(false), ProduceStage::Integer);
    let result = backend.solve_master(&model, &registration).unwrap();
    assert_eq!(result.usages, Some(vec![2.0])); assert_eq!(result.stage_report.report.termination_reason, TerminationReason::Completed);
    assert!(backend.is_cancelled());
}

#[test]
fn pre_cancel_expiry_report_limits_and_backend_errors_are_explicit() {
    for cancelled in [false, true] {
        let options = Csp2dExecutionOptions { time_limit: Duration::ZERO, ..Default::default() };
        if cancelled { options.cancellation.cancel(CancellationOrigin::User); }
        let backend = Csp2dNativeBackend::new(Arc::new(Fake(|_: &LinearTriadModel, _: &SolveOptions<'_>| panic!("must not run"))), options).unwrap();
        let (model, registration) = registered(&master(true), ProduceStage::FeasibilityLp);
        let result = backend.solve_master(&model, &registration).unwrap();
        assert!(result.usages.is_none() && result.deficits.is_none() && result.shadow_prices.is_none());
        assert_eq!(result.stage_report.report.termination_reason, if cancelled { TerminationReason::Cancelled } else { TerminationReason::TimeLimit });
    }
    let backend = Csp2dNativeBackend::new(Arc::new(Fake(|model: &LinearTriadModel, _: &SolveOptions<'_>| incumbent(model, vec![2.0]))),
        Csp2dExecutionOptions { report_limit: 1, ..Default::default() }).unwrap();
    let (model, registration) = registered(&master(false), ProduceStage::Integer);
    backend.solve_master(&model, &registration).unwrap(); assert!(backend.solve_master(&model, &registration).is_err());
    let backend = Csp2dNativeBackend::new(Arc::new(Fake(|_: &LinearTriadModel, _: &SolveOptions<'_>| Err(ospf_rust_core::error::SolverError::ContractViolation("backend failure".into()).into()))), Default::default()).unwrap();
    assert!(backend.solve_master(&model, &registration).is_err()); assert!(backend.reports().is_empty());
}

#[cfg(any(feature = "scip", feature = "scip-bundled", feature = "scip-from-source", feature = "gurobi10", feature = "gurobi11", feature = "gurobi12"))]
fn real_backend(solver: Arc<dyn LinearSolver>) {
    use ospf_rust_core::model::LinearObjectiveInput;
    use ospf_rust_core::solver::{require_infeasibility_certificate_for_model, require_optimal_lp_certificate_for_linear_model};

    let progress = Arc::new(AtomicUsize::new(0)); let observed = progress.clone();
    let backend = Csp2dNativeBackend::new(solver, Csp2dExecutionOptions { progress: Some(Arc::new(move |_| { observed.fetch_add(1, Ordering::SeqCst); Ok(()) })), ..Default::default() }).unwrap();
    let (model, registration) = registered(&master(true), ProduceStage::FeasibilityLp);
    let result = backend.solve_master(&model, &registration).unwrap();
    assert_eq!(result.deficits, Some(vec![3.0])); assert!((result.objective.unwrap() - 1.0).abs() < 1e-7);
    for stage in [ProduceStage::Rmp, ProduceStage::Integer, ProduceStage::FeasibilityLp] {
        let (model, registration) = registered(&master(false), stage);
        let result = backend.solve_master(&model, &registration).unwrap();
        assert!(result.stage_report.report.is_optimal());
        if stage == ProduceStage::FeasibilityLp {
            // 零短缺目标不决定真实列的唯一份数，不能把求解器选点当作最小用量。 / A zero-deficit objective does not uniquely determine real-column usages; the selected point is not a minimum-use claim.
            assert_eq!(result.deficits, Some(vec![0.0])); assert!(result.objective.unwrap().abs() < 1e-7);
            let usage = result.usages.as_ref().unwrap()[0]; assert!((1.5..=10.0).contains(&usage));
        } else { assert_eq!(result.usages, Some(vec![if stage == ProduceStage::Integer { 2.0 } else { 1.5 }])); }
        if stage != ProduceStage::Integer { require_optimal_lp_certificate_for_linear_model(&result.stage_report.report, &result.stage_report.model).unwrap(); }
    }
    let mut exact = master(false); exact.demands[0].upper_bound = Some(DemandValue2::Amount(3));
    let (model, registration) = registered(&exact, ProduceStage::Integer);
    let result = backend.solve_master(&model, &registration).unwrap();
    assert_eq!(result.stage_report.report.problem_status, ProblemStatus::Infeasible); assert!(result.usages.is_none());
    require_infeasibility_certificate_for_model(&result.stage_report.report, &linear_model_fingerprint(&result.stage_report.model).unwrap()).unwrap();
    let (mut model, registration) = registered(&master(false), ProduceStage::Integer);
    model.add_ge_constraint(&[(registration.usage_variables()[0], 1.0)], 3.0, "business").unwrap();
    model.add_linear_objective_input(LinearObjectiveInput::minimize("constant").constant(7.0));
    let result = backend.solve_master(&model, &registration).unwrap();
    assert_eq!(result.usages, Some(vec![3.0])); assert!((result.objective.unwrap() - 31.0).abs() < 1e-7); assert_eq!(result.stage_report.objective_constant, 7.0);
    let mut stocks = master(false);
    stocks.demands[0].demand = DemandValue2::Amount(2);
    stocks.assignment.plans[0].pattern.placements.truncate(1);
    stocks.assignment.plans[0].pattern.region.width = meters(2.0); stocks.assignment.plans[0].pattern.region.height = meters(1.0);
    stocks.assignment.plans[0].pattern.material.available_batches = 1; stocks.material_availability.insert("s".into(), 1);
    let mut expensive = stocks.assignment.plans[0].clone(); expensive.pattern.material.id = MaterialId2::new("t"); expensive.pattern.material.available_batches = 10;
    expensive.pattern.region.width = meters(4.0); stocks.assignment.plans.push(expensive); stocks.assignment.usage_upper_bounds.push(1); stocks.material_availability.insert("t".into(), 10);
    let (model, registration) = registered(&stocks, ProduceStage::Rmp);
    let result = backend.solve_master(&model, &registration).unwrap();
    assert_eq!(result.usages, Some(vec![1.0, 1.0])); assert!((result.objective.unwrap() - 6.0).abs() < 1e-7);
    let prices = result.shadow_prices.unwrap();
    assert!((prices.demands[&Csp2dDemandShadowPriceKey { product_id: "a".into(), mode: DemandMode2::Amount }] - 4.0).abs() < 1e-7);
    assert!((prices.materials["s"] + 2.0).abs() < 1e-7);
    for candidate in &stocks.assignment.plans {
        assert!(ProduceContext::default().reduced_cost(crate::domain::produce::ProducePricingInput { candidate, demands: &stocks.demands, prices: &prices, stage: ProduceStage::Rmp }).unwrap().abs() < 1e-7);
    }
    assert!(progress.load(Ordering::SeqCst) > 0);
    for stage in backend.reports() {
        assert_eq!(stage.report.fingerprints.model, Some(linear_model_fingerprint(&stage.model).unwrap()));
        assert_eq!(stage.report.model_mapping, Some(linear_model_mapping(&stage.model).unwrap()));
    }
}

#[cfg(any(feature = "scip", feature = "scip-bundled", feature = "scip-from-source"))]
#[test]
fn real_scip_masters_preserve_stock_duals_and_original_reports() {
    use ospf_rust_core_plugin_scip::{SCIPConfig, SCIPSolver};
    real_backend(Arc::new(SCIPSolver::with_config(SCIPConfig::default().with_output(false).with_lp_subproblem_defaults())));
}

#[cfg(any(feature = "gurobi10", feature = "gurobi11", feature = "gurobi12"))]
#[test]
fn real_gurobi_masters_preserve_stock_duals_and_original_reports() {
    use ospf_rust_core_plugin_gurobi::{GurobiConfig, GurobiSolver};
    real_backend(Arc::new(GurobiSolver::with_config(GurobiConfig::default().with_output(false))));
}
