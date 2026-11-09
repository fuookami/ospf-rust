use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
use ospf_rust_quantities::quantity::Quantity;
use ospf_rust_core::model::intermediate::LinearTriadModel;
use ospf_rust_core::solver::{CancellationOrigin, SolutionPresence, SolveFingerprints, SolveOptions, SolveProof, SolveReport, SolveSolution, SolverCapability, SolverInfo, SolverOutput, linear_model_fingerprint, linear_model_mapping};
use ospf_rust_framework_geometry2d::{PackingShape2, Placement2, QuantityPoint2, rectangle_shape2};
use crate::domain::material::{DemandValue2, Material2, MaterialId2, Pattern2, PatternId2, Product2, ProductId2};
use crate::infrastructure::Region2;
use super::*;

struct FunctionSolver<F> { handler: F, calls: AtomicUsize }
impl<F: Send + Sync> SolverInfo for FunctionSolver<F> {
    fn name(&self) -> &str { "csp2d-generation-contract" }
    fn capabilities(&self) -> Vec<SolverCapability> { vec![SolverCapability::Linear] }
}
impl<F> LinearSolver for FunctionSolver<F>
where F: Fn(&LinearTriadModel, &SolveOptions<'_>, usize) -> ospf_rust_core::Result<SolveReport<f64>> + Send + Sync {
    fn solve_linear(&self, _: &LinearTriadModel) -> ospf_rust_core::Result<SolverOutput> { panic!("report path required") }
    fn solve_linear_report_with_options(&self, model: &LinearTriadModel, options: &SolveOptions<'_>) -> ospf_rust_core::Result<SolveReport<f64>> {
        (self.handler)(model, options, self.calls.fetch_add(1, Ordering::SeqCst))
    }
}

fn solver(handler: impl Fn(&LinearTriadModel, &SolveOptions<'_>, usize) -> ospf_rust_core::Result<SolveReport<f64>> + Send + Sync + 'static) -> Arc<dyn LinearSolver> {
    Arc::new(FunctionSolver { handler, calls: AtomicUsize::new(0) })
}

struct Generator<F>(F);
impl<F> std::fmt::Debug for Generator<F> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { formatter.write_str("ContractGenerator") }
}
impl<F> PatternGenerator2<f64, Meter> for Generator<F>
where F: Fn(&PatternGenerationRequest2<f64, Meter>) -> Vec<Pattern2<f64, Meter>> + Send + Sync {
    fn name(&self) -> &str { "contract" }
    fn generate(&self, request: &PatternGenerationRequest2<f64, Meter>) -> Vec<Pattern2<f64, Meter>> { (self.0)(request) }
}

fn meters(value: f64) -> Quantity<f64, Meter> { Quantity::new_ct(value) }

fn input() -> Csp2dColumnGenerationInput {
    let material = Material2::sheet(MaterialId2::new("s"), "sheet", meters(4.0), meters(2.0), 10);
    let product = Product2::new(ProductId2::new("a"), "A", meters(1.0), meters(1.0)).with_allow_rotate(false);
    Csp2dColumnGenerationInput {
        requests: vec![PatternGenerationRequest2::new(material, vec![(product, 4)], meters(0.0))],
        demands: vec![Csp2dMasterDemand::new(ProductId2::new("a"), DemandValue2::Amount(3))],
        initial_columns: Vec::new(), objective: Default::default(), config: Default::default(),
    }
}

fn column(amount: usize) -> CuttingPlan2<f64, Meter> {
    let input = input();
    CuttingPlan2::from_pattern(Pattern2::new(PatternId2::new(format!("p{amount}")), input.requests[0].material.clone(),
        Region2::new(meters(0.0), meters(0.0), meters(if amount == 1 { 4.0 } else { amount as f64 }), meters(1.0)),
        (0..amount).map(|index| Placement2::new(PackingShape2::Rectangle(rectangle_shape2(meters(1.0), meters(1.0)).unwrap()),
            QuantityPoint2 { x: meters(index as f64), y: meters(0.0) }).with_item_id("a")).collect(), input.requests[0].process))
}

fn candidates(columns: Vec<CuttingPlan2<f64, Meter>>) -> Vec<Box<dyn PatternGenerator2<f64, Meter>>> {
    vec![Box::new(Generator(move |_: &PatternGenerationRequest2<f64, Meter>| columns.iter().map(|column| column.pattern.clone()).collect()))]
}

fn incumbent(model: &LinearTriadModel, values: Vec<f64>) -> ospf_rust_core::Result<SolveReport<f64>> {
    let objective = model.c.iter().zip(&values).map(|(cost, value)| cost * value).sum();
    let mut solution = SolveSolution::vector(values); solution.objective = Some(objective); solution.objective_value = Some(objective);
    SolveReport::builder(ProblemStatus::Feasible, TerminationReason::Completed).solution(solution)
        .fingerprints(SolveFingerprints { model: Some(linear_model_fingerprint(model)?), ..Default::default() })
        .model_mapping(Some(linear_model_mapping(model)?)).build()
}

fn certified(model: &LinearTriadModel, values: Vec<f64>, duals: Vec<f64>) -> ospf_rust_core::Result<SolveReport<f64>> {
    let mut report = incumbent(model, values)?; report.proof = Some(SolveProof::optimality()); report.solution_presence = SolutionPresence::Optimal;
    report.solution.as_mut().unwrap().dual_solution = Some(duals); Ok(report)
}

fn one_column_solver(model: &LinearTriadModel, _: &SolveOptions<'_>, _: usize) -> ospf_rust_core::Result<SolveReport<f64>> {
    if model.basic.variables[0].var_type().is_integer() { incumbent(model, vec![3.0]) }
    else { certified(model, vec![3.0], vec![-4.0, 0.0]) }
}

#[test]
fn actual_pricing_preserves_full_pool_reports_and_unrounded_lp_before_integer_final() {
    let mut input = input(); input.initial_columns = vec![column(1)];
    let mut duplicate = column(2); duplicate.pattern.id = PatternId2::new("same-physical-other-name"); duplicate.pattern.placements.reverse();
    let result = solve_csp2d_column_generation(solver(|model, _, call| match call {
        0 => certified(model, vec![3.0], vec![-4.0, 0.0]),
        1 => certified(model, vec![0.0, 1.5], vec![-1.0, 0.0]),
        2 => incumbent(model, vec![0.0, 2.0]),
        _ => panic!("unexpected child"),
    }), Default::default(), &input, &candidates(vec![column(2), duplicate])).unwrap();
    assert_eq!(result.columns.len(), 2); assert_eq!(result.columns[0].pattern.placements.len(), 1);
    assert_eq!(result.stages.iter().map(|stage| stage.pool_size).collect::<Vec<_>>(), vec![1, 2, 2]);
    assert_eq!(result.stages[1].result.usages, Some(vec![0.0, 1.5])); assert_eq!(result.stages[2].result.usages, Some(vec![0.0, 2.0]));
    assert_eq!(result.iterations[0].added_columns, vec![(1, -6.0)]); assert_eq!(result.iterations[0].candidates_evaluated, 2); assert!(result.iterations[1].added_columns.is_empty());
    assert_eq!(result.final_stage, Some(2)); assert_eq!(result.stop, Csp2dColumnGenerationStop::NoImprovingColumns); assert!(!result.candidate_complete);
    for stage in result.stages {
        assert_eq!(stage.result.stage_report.report.fingerprints.model, Some(linear_model_fingerprint(&stage.result.stage_report.model).unwrap()));
    }
}

#[test]
fn generator_area_price_hint_combines_bases_while_actual_pricing_keeps_original_rows() {
    let wider = |amount| {
        let mut column = column(amount); column.pattern.region.width = meters(4.0);
        for (index, placement) in column.pattern.placements.iter_mut().enumerate() {
            placement.shape = PackingShape2::Rectangle(rectangle_shape2(meters(2.0), meters(1.0)).unwrap()); placement.position.x = meters(index as f64 * 2.0);
        }
        column
    };
    let mut input = input(); input.requests[0].products[0].0.width = meters(2.0); input.initial_columns = vec![wider(1)];
    input.demands.push(Csp2dMasterDemand::new(ProductId2::new("a"), DemandValue2::Area(6.0)));
    let pair = wider(2);
    let generators = vec![Box::new(Generator(move |request: &PatternGenerationRequest2<f64, Meter>| {
        assert_eq!(request.shadow_prices["a"], if request.iteration == 0 { 2.0 } else { 1.0 }); vec![pair.pattern.clone()]
    })) as Box<dyn PatternGenerator2<f64, Meter>>];
    let result = solve_csp2d_column_generation(solver(|model, _, call| match call {
        0 => certified(model, vec![3.0], vec![-2.0, -1.0, 0.0]),
        1 => certified(model, vec![0.0, 1.5], vec![-1.0, -0.5, 0.0]),
        _ => incumbent(model, vec![0.0, 2.0]),
    }), Default::default(), &input, &generators).unwrap();
    assert_eq!(result.iterations[0].added_columns, vec![(1, -4.0)]); assert_eq!(result.stages[0].result.shadow_prices.as_ref().unwrap().demands.len(), 2);
}

#[test]
fn empty_pool_phase_one_recovers_before_ordinary_pricing_and_ignores_request_history() {
    let mut input = input();
    input.requests[0].iteration = -999; input.requests[0].shadow_prices.insert("forged".into(), f64::NAN);
    input.requests[0].existing_patterns = vec![column(1).pattern]; input.requests[0].max_candidates = 0;
    let generators = vec![Box::new(Generator(|request: &PatternGenerationRequest2<f64, Meter>| {
        assert_eq!(request.iteration, i64::from(!request.existing_patterns.is_empty()));
        assert!(request.shadow_prices.contains_key("a") && !request.shadow_prices.contains_key("forged"));
        vec![column(2).pattern]
    })) as Box<dyn PatternGenerator2<f64, Meter>>];
    let result = solve_csp2d_column_generation(solver(|model, _, call| match call {
        0 => certified(model, vec![3.0], vec![-1.0 / 3.0, 0.0]),
        1 => certified(model, vec![1.5, 0.0], vec![0.0, 0.0]),
        2 => certified(model, vec![1.5], vec![-1.0, 0.0]),
        3 => incumbent(model, vec![2.0]),
        _ => panic!("unexpected child"),
    }), Default::default(), &input, &generators).unwrap();
    assert_eq!(result.stages.iter().map(|stage| stage.result.stage_report.stage).collect::<Vec<_>>(), vec![ProduceStage::FeasibilityLp, ProduceStage::FeasibilityLp, ProduceStage::Rmp, ProduceStage::Integer]);
    assert_eq!(result.stages[0].result.deficits, Some(vec![3.0])); assert_eq!(result.stages[1].result.deficits, Some(vec![0.0]));
    assert!((result.iterations[0].added_columns[0].1 + 2.0 / 3.0).abs() < 1e-10); assert_eq!(result.final_stage, Some(3));
}

#[test]
fn tiny_normalized_objective_never_hides_nonzero_original_unit_deficits() {
    let mut input = input(); input.demands[0].demand = DemandValue2::Amount(1_000_000_000_000_000); input.config.final_integer_enabled = false;
    input.requests[0].material.available_batches = 999_999_999_999_999; input.config.reduced_cost_tolerance = 0.0;
    let mut candidate = column(1); candidate.pattern.material = input.requests[0].material.clone();
    let result = solve_csp2d_column_generation(solver(|model, _, call| match call {
        0 => certified(model, vec![1e15], vec![-1e-15, 0.0]),
        1 => { assert_eq!(model.c, vec![0.0, 1e-15]); certified(model, vec![999_999_999_999_999.0, 1.0], vec![-1e-15, -1e-15]) },
        _ => panic!("must not declare recovered from tiny normalized shortage"),
    }), Default::default(), &input, &candidates(vec![candidate])).unwrap();
    assert_eq!(result.stages.len(), 2); assert_eq!(result.stages[1].result.deficits, Some(vec![1.0]));
    assert_eq!(result.stages[1].result.objective, Some(1e-15));
    assert_eq!(result.stop, Csp2dColumnGenerationStop::NoImprovingColumns);
}

#[test]
fn initial_duplicate_positions_survive_and_generated_duplicates_ignore_pattern_names_and_order() {
    let mut input = input(); let mut duplicate = column(1); duplicate.pattern.id = PatternId2::new("other");
    input.initial_columns = vec![column(1), duplicate.clone()];
    let result = solve_csp2d_column_generation(solver(|model, _, call| {
        if call == 0 { certified(model, vec![3.0, 0.0], vec![-4.0, 0.0]) } else { incumbent(model, vec![3.0, 0.0]) }
    }), Default::default(), &input, &candidates(vec![duplicate])).unwrap();
    assert_eq!(result.columns.len(), 2); assert_eq!(result.columns[1].pattern.id.as_str(), "other"); assert!(result.iterations[0].added_columns.is_empty());
}

#[test]
fn nonoptimal_interrupted_and_untrusted_reports_never_price_or_start_final() {
    for reason in [TerminationReason::Completed, TerminationReason::TimeLimit, TerminationReason::Cancelled, TerminationReason::Interrupted] {
        let mut input = input(); input.initial_columns = vec![column(1)];
        let generators = vec![Box::new(Generator(|_: &PatternGenerationRequest2<f64, Meter>| -> Vec<_> { panic!("must not price") })) as Box<dyn PatternGenerator2<f64, Meter>>];
        let result = solve_csp2d_column_generation(solver(move |model, _, call| {
            assert_eq!(call, 0); let mut report = incumbent(model, vec![3.0])?; report.termination_reason = reason; Ok(report)
        }), Default::default(), &input, &generators).unwrap();
        assert_eq!(result.stages.len(), 1); assert!(result.iterations.is_empty() && result.final_stage.is_none());
        assert_eq!(result.stages[0].result.usages, Some(vec![3.0]));
    }
    let mut input = input(); input.initial_columns = vec![column(1)];
    assert!(solve_csp2d_column_generation(solver(|model, _, _| certified(model, vec![3.0], vec![0.0, 0.0])), Default::default(), &input, &[]).is_err());
}

#[test]
fn budgets_final_omission_and_cancellation_do_not_invent_children_or_relabel_final_reports() {
    let mut input = input(); input.initial_columns = vec![column(1)];
    for cancelled in [false, true] {
        let options = Csp2dExecutionOptions { time_limit: Duration::ZERO, ..Default::default() };
        if cancelled { options.cancellation.cancel(CancellationOrigin::User); }
        let result = solve_csp2d_column_generation(solver(|_, _, _| panic!("must not solve")), options, &input, &[]).unwrap();
        assert!(result.stages.is_empty()); assert_eq!(result.stop, if cancelled { Csp2dColumnGenerationStop::Cancelled } else { Csp2dColumnGenerationStop::TimeLimit });
    }
    input.config.iteration_limit = 0;
    let result = solve_csp2d_column_generation(solver(one_column_solver), Default::default(), &input, &[]).unwrap();
    assert_eq!(result.stop, Csp2dColumnGenerationStop::IterationLimit); assert_eq!(result.final_stage, Some(1));
    let result = solve_csp2d_column_generation(solver(one_column_solver), Csp2dExecutionOptions { report_limit: 1, ..Default::default() }, &input, &[]).unwrap();
    assert_eq!(result.stop, Csp2dColumnGenerationStop::ReportLimit); assert!(result.final_stage.is_none());
    input.config.final_integer_enabled = false;
    let result = solve_csp2d_column_generation(solver(one_column_solver), Default::default(), &input, &[]).unwrap();
    assert_eq!(result.stages.len(), 1); assert!(result.final_stage.is_none());
    input.config.final_integer_enabled = true;
    let options = Csp2dExecutionOptions::default(); let handle = options.cancellation.clone();
    let result = solve_csp2d_column_generation(solver(move |model, options, call| {
        let report = one_column_solver(model, options, call)?; if call == 1 { handle.cancel(CancellationOrigin::User); } Ok(report)
    }), options, &input, &[]).unwrap();
    assert_eq!(result.stages[1].result.stage_report.report.termination_reason, TerminationReason::Completed); assert_eq!(result.stages[1].result.usages, Some(vec![3.0]));
}

#[test]
fn source_forgery_unbound_rules_and_invalid_limits_fail_before_native_execution() {
    for variant in 0..21 {
        let mut input = input(); input.initial_columns = vec![column(1)];
        match variant {
            0 => input.requests.clear(),
            1 => input.requests.push(input.requests[0].clone()),
            2 => input.requests[0].kerf = meters(f64::NAN),
            3 => input.requests[0].products[0].0.width = meters(f64::INFINITY),
            4 => input.requests[0].products[0].1 = 9_007_199_254_740_992,
            5 => input.requests[0].material.unit_weight = Some(meters(1.0)),
            6 => input.requests[0].products[0].0.unit_weight = Some(meters(1.0)),
            7 => input.requests[0].max_segment_height = Some(meters(1.0)),
            8 => input.initial_columns[0].pattern.material.width = meters(5.0),
            9 => input.initial_columns[0].pattern.placements[0].rotated = true,
            10 => input.initial_columns[0].pattern.placements[0].item_id = Some("unknown".into()),
            11 => input.initial_columns[0].pattern.process = crate::infrastructure::CuttingProcess2::GuillotineTwoStage,
            12 => input.demands[0].product_id = ProductId2::new("unknown"),
            13 => input.config.column_limit = 0,
            14 => input.config.max_candidates_per_iteration = 0,
            15 => input.config.iteration_limit = 100_001,
            16 => input.config.reduced_cost_tolerance = f64::NAN,
            17 => input.objective.usage_weight = -1.0,
            18 => input.demands[0].upper_bound = Some(DemandValue2::Amount(2)),
            19 => input.initial_columns[0].pattern.placements[0].position.x = meters(5.0),
            _ => { let duplicate = input.requests[0].products[0].clone(); input.requests[0].products.push(duplicate); },
        }
        assert!(solve_csp2d_column_generation(solver(|_, _, _| panic!("invalid source reached backend")), Default::default(), &input, &[]).is_err(), "variant {variant}");
    }
}

#[test]
fn invalid_observed_candidates_fail_even_when_not_improving_and_cancellation_stops_before_insertion() {
    let mut input = input(); input.initial_columns = vec![column(1)];
    let mut invalid = column(2); invalid.pattern.placements[0].shape = PackingShape2::Rectangle(rectangle_shape2(meters(0.5), meters(1.0)).unwrap());
    invalid.pattern.region.width = meters(4.0); invalid.pattern.region.height = meters(2.0);
    assert!(solve_csp2d_column_generation(solver(one_column_solver), Default::default(), &input, &candidates(vec![invalid])).is_err());
    let options = Csp2dExecutionOptions::default(); let cancellation = options.cancellation.clone();
    let generators = vec![Box::new(Generator(move |_: &PatternGenerationRequest2<f64, Meter>| { cancellation.cancel(CancellationOrigin::User); vec![column(2).pattern] })) as Box<dyn PatternGenerator2<f64, Meter>>];
    let result = solve_csp2d_column_generation(solver(one_column_solver), options, &input, &generators).unwrap();
    assert_eq!(result.stop, Csp2dColumnGenerationStop::Cancelled); assert_eq!(result.columns.len(), 1); assert!(result.iterations.is_empty() && result.final_stage.is_none());
}

#[test]
fn generation_candidate_and_column_caps_apply_to_observations_not_global_completeness() {
    let mut input = input(); input.initial_columns = vec![column(1)]; input.config.max_candidates_per_iteration = 1; input.config.column_limit = 2;
    let result = solve_csp2d_column_generation(solver(|model, _, call| match call {
        0 => certified(model, vec![3.0], vec![-4.0, 0.0]),
        1 => certified(model, vec![0.0, 1.5], vec![-1.0, 0.0]),
        _ => incumbent(model, vec![0.0, 2.0]),
    }), Default::default(), &input, &candidates(vec![column(2), column(4)])).unwrap();
    assert_eq!(result.columns.len(), 2); assert_eq!(result.stop, Csp2dColumnGenerationStop::ColumnLimit);
    assert_eq!(result.iterations[0].candidates_evaluated, 1); assert!(result.iterations[0].candidate_limit_reached); assert!(!result.candidate_complete);
}

#[cfg(any(feature = "scip", feature = "scip-bundled", feature = "scip-from-source", feature = "gurobi10", feature = "gurobi11", feature = "gurobi12"))]
mod real;
