use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
use ospf_rust_quantities::quantity::Quantity;
use ospf_rust_core::model::intermediate::LinearTriadModel;
use ospf_rust_core::solver::{CancellationOrigin, SolutionPresence, SolveFingerprints, SolveOptions, SolveProof, SolveReport, SolveSolution, SolverCapability, SolverInfo, SolverOutput, linear_model_fingerprint, linear_model_mapping};
use ospf_rust_framework_geometry2d::{CirclePackingShape2, PackingShape2, Placement2, QuantityPoint2, Sheet2Shape, rectangle_shape2};
use crate::domain::item::model::{ActualItem, ItemId, SheetType, SheetTypeId};
use crate::domain::layer_assignment::Bpp2dAssignmentDemandValue;
use crate::domain::layer_generation::Bpp2dLayerGenerationResult;
use super::*;

struct FunctionSolver<F> { handler: F, calls: AtomicUsize }
impl<F: Send + Sync> SolverInfo for FunctionSolver<F> {
    fn name(&self) -> &str { "bpp2d-generation-contract" }
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

struct Generator<F>(F);
impl<F: Fn(&Bpp2dLayerGenerationRequest<f64, Meter>) -> Vec<Bpp2dLayerGenerationResult<f64, Meter>>> Bpp2dLayerGenerator<f64, Meter> for Generator<F> {
    fn name(&self) -> &str { "contract" }
    fn generate(&self, request: &Bpp2dLayerGenerationRequest<f64, Meter>) -> Vec<Bpp2dLayerGenerationResult<f64, Meter>> { (self.0)(request) }
}

fn meters(value: f64) -> Quantity<f64, Meter> { Quantity::new_ct(value) }

fn input() -> Bpp2dColumnGenerationInput {
    let shape = PackingShape2::Rectangle(rectangle_shape2(meters(1.0), meters(1.0)).unwrap());
    let item = ActualItem::new(ItemId::new("a"), "A", shape, false, meters(0.0));
    let sheet = SheetType::new(meters(2.0), meters(1.0), meters(0.0), SheetTypeId::new("s"));
    Bpp2dColumnGenerationInput {
        request: Bpp2dLayerGenerationRequest::new(999, vec![(item, 3)]).with_sheet(sheet),
        demands: vec![Bpp2dAssignmentDemandEntry::item_amount("a", 3)], objective: Default::default(), config: Default::default(),
    }
}

fn column(amount: usize) -> Layer<f64, Meter> {
    let input = input();
    Layer::new(0, "contract", input.request.sheet.clone(), Sheet2Shape::new(meters(amount as f64), meters(1.0)))
        .with_units((0..amount).map(|index| Placement2::new(input.request.items[0].0.shape.clone(), QuantityPoint2 { x: meters(index as f64), y: meters(0.0) }).with_item_id("a")).collect())
}

fn candidates(layers: Vec<Layer<f64, Meter>>) -> Vec<Box<dyn Bpp2dLayerGenerator<f64, Meter>>> {
    vec![Box::new(Generator(move |_: &Bpp2dLayerGenerationRequest<f64, Meter>| layers.iter().cloned().map(|layer| {
        // 自报检验数不可信，正式路径必须重算。 / Self-reported reduced costs are untrusted and must be recomputed.
        Bpp2dLayerGenerationResult::new(layer, "contract").with_reduced_cost(-1e90)
    }).collect()))]
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

fn certified(model: &LinearTriadModel, values: Vec<f64>, duals: Vec<f64>) -> ospf_rust_core::Result<SolveReport<f64>> {
    let mut report = incumbent(model, values)?;
    report.proof = Some(SolveProof::optimality());
    report.solution_presence = SolutionPresence::Optimal;
    report.solution.as_mut().unwrap().dual_solution = Some(duals);
    Ok(report)
}

fn one_column_solver(model: &LinearTriadModel, _: &SolveOptions<'_>, _: usize) -> ospf_rust_core::Result<SolveReport<f64>> {
    if model.basic.variables[0].var_type().is_integer() { incumbent(model, vec![3.0]) }
    else { certified(model, vec![3.0], vec![-2.0]) }
}

#[test]
fn initial_pool_is_preserved_and_final_subset_never_overwrites_it() {
    let mut input = input();
    input.request.existing_layers = vec![column(1)];
    let result = solve_bpp2d_column_generation(solver(|model, _, call| match call {
        0 => certified(model, vec![3.0], vec![-2.0]),
        1 => certified(model, vec![0.0, 1.5], vec![-1.0]),
        2 => incumbent(model, vec![0.0, 2.0]),
        _ => panic!("unexpected child"),
    }), Default::default(), &input, &candidates(vec![column(2)])).unwrap();
    assert_eq!(result.columns.len(), 2);
    assert_eq!(result.columns[0].units.len(), 1);
    assert_eq!(result.columns[1].units.len(), 2);
    assert_eq!(result.stages.iter().map(|stage| stage.pool_size).collect::<Vec<_>>(), vec![1, 2, 2]);
    assert_eq!(result.stages[2].result.copies, Some(vec![0.0, 2.0]));
    assert_eq!(result.iterations[0].added_columns, vec![(1, -2.0)]);
    assert!(result.iterations[1].added_columns.is_empty());
    assert_eq!(result.final_stage, Some(2));
    assert_eq!(result.stop, Bpp2dColumnGenerationStop::NoImprovingColumns);
    assert!(!result.candidate_complete);
}

#[test]
fn empty_pool_recovers_through_true_phase_one_before_ordinary_pricing() {
    let generators = vec![Box::new(Generator(|request: &Bpp2dLayerGenerationRequest<f64, Meter>| {
        assert_eq!(request.iteration, i64::from(!request.existing_layers.is_empty()));
        assert_eq!(request.demand_entries, generation_demands(&input().demands));
        assert_eq!(request.shadow_prices.len(), 1);
        vec![Bpp2dLayerGenerationResult::new(column(2), "contract")]
    })) as Box<dyn Bpp2dLayerGenerator<f64, Meter>>];
    let result = solve_bpp2d_column_generation(solver(|model, _, call| match call {
        0 => { assert_eq!(model.c, vec![1.0 / 3.0]); certified(model, vec![3.0], vec![-1.0 / 3.0]) },
        1 => { assert_eq!(model.c, vec![0.0, 1.0 / 3.0]); certified(model, vec![1.5, 0.0], vec![0.0]) },
        2 => certified(model, vec![1.5], vec![-1.0]),
        3 => incumbent(model, vec![2.0]),
        _ => panic!("unexpected child"),
    }), Default::default(), &input(), &generators).unwrap();
    assert_eq!(result.stages.iter().map(|stage| stage.result.stage_report.stage).collect::<Vec<_>>(), vec![LayerAssignmentStage::FeasibilityLp, LayerAssignmentStage::FeasibilityLp, LayerAssignmentStage::Rmp, LayerAssignmentStage::Integer]);
    assert_eq!(result.stages[0].result.deficits, Some(vec![3.0]));
    assert_eq!(result.stages[1].result.deficits, Some(vec![0.0]));
    assert!((result.iterations[0].added_columns[0].1 + 2.0 / 3.0).abs() < 1e-10);
    assert_eq!(result.iterations[1].stage_index, 2);
    assert_eq!(result.final_stage, Some(3));
}

#[test]
fn initial_duplicate_positions_survive_but_generated_physical_duplicates_do_not() {
    let mut input = input();
    let mut duplicate = column(1);
    duplicate.from = "another-source".into();
    duplicate.iteration = 27;
    input.request.existing_layers = vec![column(1), duplicate.clone()];
    let result = solve_bpp2d_column_generation(solver(|model, _, call| {
        if call == 0 { certified(model, vec![3.0, 0.0], vec![-2.0]) }
        else { incumbent(model, vec![3.0, 0.0]) }
    }), Default::default(), &input, &candidates(vec![duplicate])).unwrap();
    assert_eq!(result.columns.len(), 2);
    assert_eq!(result.columns[1].from, "another-source");
    assert!(result.iterations[0].added_columns.is_empty());
}

#[test]
fn nonoptimal_or_interrupted_lp_never_prices_or_starts_final() {
    for reason in [TerminationReason::Completed, TerminationReason::TimeLimit, TerminationReason::Cancelled, TerminationReason::Interrupted] {
        let mut input = input();
        input.request.existing_layers = vec![column(1)];
        let generators = vec![Box::new(Generator(|_: &Bpp2dLayerGenerationRequest<f64, Meter>| -> Vec<_> { panic!("must not price") })) as Box<dyn Bpp2dLayerGenerator<f64, Meter>>];
        let result = solve_bpp2d_column_generation(solver(move |model, _, call| {
            assert_eq!(call, 0);
            let mut report = incumbent(model, vec![3.0])?;
            report.termination_reason = reason;
            Ok(report)
        }), Default::default(), &input, &generators).unwrap();
        assert_eq!(result.stages.len(), 1);
        assert!(result.iterations.is_empty() && result.final_stage.is_none());
        assert_eq!(result.stages[0].result.copies, Some(vec![3.0]));
    }
}

#[test]
fn bad_certificates_identity_or_generator_geometry_fail_closed() {
    for variant in 0..4 {
        let mut input = input();
        input.request.existing_layers = vec![column(1)];
        let result = solve_bpp2d_column_generation(solver(move |model, _, _| {
            let mut report = certified(model, vec![3.0], vec![-2.0])?;
            match variant {
                0 => report.solution.as_mut().unwrap().dual_solution = None,
                1 => report.solution.as_mut().unwrap().dual_solution = Some(vec![0.0]),
                2 => report.fingerprints.model = Some(linear_model_fingerprint(&LinearTriadModel::new("foreign"))?),
                _ => report.solution.as_mut().unwrap().values[0] = 1.0,
            }
            Ok(report)
        }), Default::default(), &input, &[]);
        assert!(result.is_err(), "variant {variant}");
    }
    for variant in 0..8 {
        let mut input = input();
        input.request.existing_layers = vec![column(1)];
        let mut bad = column(2);
        match variant {
            0 => bad.units[0].item_id = Some("foreign".into()),
            1 => bad.sheet.as_mut().unwrap().width.value = 99.0,
            2 => bad.units[0].shape = PackingShape2::Rectangle(rectangle_shape2(meters(0.5), meters(1.0)).unwrap()),
            3 => bad.units[0].position.x.value = f64::NAN,
            4 => bad.units[1].position.x.value = 0.5,
            5 => bad.shape.width.value = 0.5,
            6 => { input.request.kerf = Some(meters(0.1)); },
            _ => bad.sheet.as_mut().unwrap().type_code = SheetTypeId::new("foreign"),
        }
        assert!(solve_bpp2d_column_generation(solver(one_column_solver), Default::default(), &input, &candidates(vec![bad])).is_err(), "variant {variant}");
    }
}

#[test]
fn invalid_source_limits_or_unsupported_mass_are_rejected_before_solver() {
    for variant in 0..13 {
        let mut input = input();
        match variant {
            0 => input.request.items.push(input.request.items[0].clone()),
            1 => input.request.items[0].0.weight.value = 1.0,
            2 => input.request.sheet.as_mut().unwrap().max_load.value = 1.0,
            3 => input.request.kerf = Some(meters(f64::NAN)),
            4 => input.request.kerf = Some(meters(-1.0)),
            5 => input.demands = vec![Bpp2dAssignmentDemandEntry::item_amount("foreign", 1)],
            6 => input.demands = vec![Bpp2dAssignmentDemandEntry::item_amount("a", 9_007_199_254_740_992)],
            7 => input.request.demand_entries = generation_demands(&[Bpp2dAssignmentDemandEntry::item_amount("a", 1)]),
            8 => input.config.max_candidates_per_iteration = 0,
            9 => input.config.column_limit = 0,
            10 => input.config.reduced_cost_tolerance = f64::NAN,
            11 => input.objective.rmp_area_coefficient = f64::INFINITY,
            _ => input.demands[0] = input.demands[0].clone().with_upper_bound(Bpp2dAssignmentDemandValue::Amount(1)),
        }
        assert!(solve_bpp2d_column_generation(solver(|_, _, _| panic!("preflight must fail")), Default::default(), &input, &[]).is_err(), "variant {variant}");
    }
}

#[test]
fn iteration_pool_and_report_caps_retain_every_accepted_stage() {
    for variant in 0..3 {
        let mut input = input();
        input.request.existing_layers = vec![column(1)];
        let mut execution = Bpp2dExecutionOptions::default();
        match variant { 0 => input.config.iteration_limit = 0, 1 => input.config.column_limit = 1, _ => execution.report_limit = 1 }
        let result = solve_bpp2d_column_generation(solver(one_column_solver), execution, &input, &[]).unwrap();
        assert_eq!(result.stop, match variant { 0 => Bpp2dColumnGenerationStop::IterationLimit, 1 => Bpp2dColumnGenerationStop::ColumnLimit, _ => Bpp2dColumnGenerationStop::ReportLimit });
        assert_eq!(result.stages.len(), if variant == 2 { 1 } else { 2 });
        assert_eq!(result.final_stage.is_some(), variant != 2);
        assert_eq!(result.columns.len(), 1);
    }
}

#[test]
fn cancelled_or_timed_out_generation_never_starts_a_new_child() {
    for cancel in [false, true] {
        let mut input = input();
        input.request.existing_layers = vec![column(1)];
        let execution = Bpp2dExecutionOptions { time_limit: if cancel { Duration::from_secs(60) } else { Duration::from_secs(1) }, ..Default::default() };
        let handle = execution.cancellation.clone();
        let generators = vec![Box::new(Generator(move |_: &Bpp2dLayerGenerationRequest<f64, Meter>| {
            if cancel { handle.cancel(CancellationOrigin::User); }
            else { std::thread::sleep(Duration::from_millis(1100)); }
            vec![Bpp2dLayerGenerationResult::new(column(2), "contract")]
        })) as Box<dyn Bpp2dLayerGenerator<f64, Meter>>];
        let result = solve_bpp2d_column_generation(solver(|model, _, call| { assert_eq!(call, 0); certified(model, vec![3.0], vec![-2.0]) }), execution, &input, &generators).unwrap();
        assert_eq!(result.stop, if cancel { Bpp2dColumnGenerationStop::Cancelled } else { Bpp2dColumnGenerationStop::TimeLimit });
        assert_eq!(result.stages.len(), 1);
        assert!(result.final_stage.is_none() && result.iterations.is_empty());
        assert_eq!(result.columns.len(), 1);
    }
}

#[test]
fn precancel_zero_budget_and_final_disabled_do_not_invent_reports() {
    for cancel in [false, true] {
        let execution = Bpp2dExecutionOptions { time_limit: Duration::ZERO, ..Default::default() };
        if cancel { execution.cancellation.cancel(CancellationOrigin::User); }
        let result = solve_bpp2d_column_generation(solver(|_, _, _| panic!("must not solve")), execution, &input(), &[]).unwrap();
        assert_eq!(result.stop, if cancel { Bpp2dColumnGenerationStop::Cancelled } else { Bpp2dColumnGenerationStop::TimeLimit });
        assert!(result.stages.is_empty() && result.final_stage.is_none());
    }
    let mut input = input();
    input.request.existing_layers = vec![column(1)];
    input.config.final_milp_enabled = false;
    let result = solve_bpp2d_column_generation(solver(one_column_solver), Default::default(), &input, &[]).unwrap();
    assert_eq!(result.stages.len(), 1);
    assert!(result.final_stage.is_none());
}

#[test]
fn final_incumbent_and_its_original_timeout_are_retained_without_rounding() {
    let mut input = input();
    input.request.existing_layers = vec![column(1)];
    let result = solve_bpp2d_column_generation(solver(|model, _, call| {
        if call == 0 { certified(model, vec![3.0], vec![-2.0]) }
        else {
            let mut report = incumbent(model, vec![3.0])?;
            report.termination_reason = TerminationReason::TimeLimit;
            Ok(report)
        }
    }), Default::default(), &input, &[]).unwrap();
    let final_result = &result.stages[result.final_stage.unwrap()].result;
    assert_eq!(result.stop, Bpp2dColumnGenerationStop::NoImprovingColumns);
    assert_eq!(final_result.stage_report.report.termination_reason, TerminationReason::TimeLimit);
    assert_eq!(final_result.copies, Some(vec![3.0]));
}

#[test]
fn observation_cap_does_not_imply_complete_pricing_or_trust_generator_scores() {
    let mut input = input();
    input.request.existing_layers = vec![column(1)];
    input.config.max_candidates_per_iteration = 1;
    let result = solve_bpp2d_column_generation(solver(one_column_solver), Default::default(), &input, &candidates(vec![column(1), column(2)])).unwrap();
    assert_eq!(result.columns.len(), 1);
    assert_eq!(result.iterations.len(), 1);
    assert!(result.iterations[0].candidate_limit_reached);
    assert_eq!(result.iterations[0].candidates_evaluated, 1);
    assert!(result.iterations[0].added_columns.is_empty());
    assert!(!result.candidate_complete);
}

#[test]
fn source_rotation_policy_and_original_shape_must_both_match() {
    let mut input = input();
    input.request.sheet.as_mut().unwrap().height.value = 2.0;
    input.request.items[0].0.shape = PackingShape2::Rectangle(rectangle_shape2(meters(2.0), meters(1.0)).unwrap());
    let mut rotated = Layer::new(0, "rotated", input.request.sheet.clone(), Sheet2Shape::new(meters(1.0), meters(2.0)))
        .with_units(vec![Placement2::new(input.request.items[0].0.shape.rotated(), QuantityPoint2 { x: meters(0.0), y: meters(0.0) }).with_rotated(true).with_item_id("a")]);
    let catalog = ColumnCatalog::new(&input.request).unwrap();
    assert!(catalog.validate(&rotated).is_err());
    input.request.items[0].0.allow_rotate = true;
    let catalog = ColumnCatalog::new(&input.request).unwrap();
    assert!(catalog.validate(&rotated).is_ok());
    rotated.units[0].rotated = false;
    assert!(catalog.validate(&rotated).is_err());
}

#[test]
fn explicitly_selected_custom_circle_generator_is_not_discarded_by_its_name() {
    let mut input = input();
    input.request.items[0].0.shape = PackingShape2::Circle(CirclePackingShape2::new(meters(0.5)));
    let mut circles = column(2);
    for placement in &mut circles.units { placement.shape = input.request.items[0].0.shape.clone(); }
    let result = solve_bpp2d_column_generation(solver(|model, _, call| match call {
        0 => certified(model, vec![3.0], vec![-1.0 / 3.0]),
        1 => certified(model, vec![1.5, 0.0], vec![0.0]),
        2 => certified(model, vec![1.5], vec![-1.0]),
        3 => incumbent(model, vec![2.0]),
        _ => panic!("unexpected child"),
    }), Default::default(), &input, &candidates(vec![circles])).unwrap();
    assert_eq!(result.columns.len(), 1);
    assert_eq!(result.final_stage, Some(3));
}

#[test]
fn backend_failures_and_unsafe_final_integers_have_no_heuristic_fallback() {
    let mut input = input();
    input.request.existing_layers = vec![column(1)];
    for variant in 0..3 {
        let result = solve_bpp2d_column_generation(solver(move |model, _, call| {
            if call == 0 { certified(model, vec![3.0], vec![-2.0]) }
            else { match variant {
                0 => Err(invalid("backend failed")),
                1 => incumbent(model, vec![9_007_199_254_740_992.0]),
                _ => incumbent(model, vec![3.5]),
            } }
        }), Default::default(), &input, &[]);
        assert!(result.is_err());
    }
}

#[test]
fn cancellation_after_completed_lp_prevents_pricing_but_retains_that_original_lp() {
    let mut input = input();
    input.request.existing_layers = vec![column(1)];
    let execution = Bpp2dExecutionOptions::default();
    let parent = execution.cancellation.clone();
    let result = solve_bpp2d_column_generation(solver(move |model, options, call| {
        assert_eq!(call, 0);
        let report = certified(model, vec![3.0], vec![-2.0])?;
        options.cancellation_handle.unwrap().mark_completed();
        parent.cancel(CancellationOrigin::User);
        Ok(report)
    }), execution, &input, &[]).unwrap();
    assert_eq!(result.stop, Bpp2dColumnGenerationStop::Cancelled);
    assert_eq!(result.stages.len(), 1);
    assert!(result.stages[0].result.stage_report.report.is_optimal());
    assert_eq!(result.stages[0].result.stage_report.report.termination_reason, TerminationReason::Completed);
    assert!(result.iterations.is_empty() && result.final_stage.is_none());
}

#[cfg(any(feature = "scip", feature = "scip-bundled", feature = "scip-from-source", feature = "gurobi10", feature = "gurobi11", feature = "gurobi12"))]
fn real_contract(solver: Arc<dyn LinearSolver>) {
    use ospf_rust_core::solver::{require_infeasibility_certificate_for_model, require_optimal_lp_certificate_for_linear_model};
    use ospf_rust_framework::solver::remote::{decode_compiled_linear_model, encode_compiled_linear_model};

    for existing in [Vec::new(), vec![column(1)]] {
        let mut input = input();
        input.request.existing_layers = existing;
        input.demands.push(Bpp2dAssignmentDemandEntry::item_area("a", 3.0));
        let progress = Arc::new(AtomicUsize::new(0));
        let observed = progress.clone();
        let result = solve_bpp2d_column_generation(solver.clone(), Bpp2dExecutionOptions {
            progress: Some(Arc::new(move |_| { observed.fetch_add(1, Ordering::SeqCst); Ok(()) })), ..Default::default()
        }, &input, &LayerGenerationContext::default_generators()).unwrap();
        assert!(!result.candidate_complete);
        assert_eq!(result.stop, Bpp2dColumnGenerationStop::NoImprovingColumns);
        let final_result = &result.stages[result.final_stage.unwrap()].result;
        assert!(final_result.stage_report.report.is_optimal());
        assert!((final_result.objective.unwrap() - 4.000002).abs() < 1e-7);
        assert_eq!(final_result.copies.as_ref().unwrap().len(), result.columns.len());
        assert!(progress.load(Ordering::SeqCst) > 0);
        assert!(!result.iterations.is_empty());
        for record in &result.stages {
            assert_eq!(record.result.copies.as_ref().unwrap().len(), record.pool_size);
            let model = decode_compiled_linear_model(&encode_compiled_linear_model(&record.result.stage_report.model).unwrap()).unwrap();
            assert_eq!(record.result.stage_report.report.fingerprints.model, Some(linear_model_fingerprint(&model).unwrap()));
            assert_eq!(record.result.stage_report.report.model_mapping, Some(linear_model_mapping(&model).unwrap()));
            if record.result.stage_report.stage != LayerAssignmentStage::Integer {
                require_optimal_lp_certificate_for_linear_model(&record.result.stage_report.report, &model).unwrap();
            }
        }
    }
    let mut exact = input();
    exact.demands[0] = exact.demands[0].clone().with_upper_bound(Bpp2dAssignmentDemandValue::Amount(3));
    let result = solve_bpp2d_column_generation(solver.clone(), Default::default(), &exact, &candidates(vec![column(2)])).unwrap();
    let final_result = &result.stages[result.final_stage.unwrap()].result;
    assert_eq!(final_result.stage_report.report.problem_status, ProblemStatus::Infeasible);
    assert!(final_result.copies.is_none());
    assert!(!result.candidate_complete);
    require_infeasibility_certificate_for_model(&final_result.stage_report.report, &linear_model_fingerprint(&final_result.stage_report.model).unwrap()).unwrap();

    // 非空但缺列的 RMP 必须先保留不可行报告，再进入 Phase I。 / A nonempty deficient RMP must retain its infeasible report before Phase I.
    let mut missing = input();
    let mut other = missing.request.items[0].0.clone();
    other.id = ItemId::new("b");
    missing.request.items.push((other, 3));
    missing.demands.push(Bpp2dAssignmentDemandEntry::item_amount("b", 3));
    missing.request.existing_layers.push(column(1));
    let mut b = column(2);
    for placement in &mut b.units { placement.item_id = Some("b".into()); }
    let result = solve_bpp2d_column_generation(solver.clone(), Default::default(), &missing, &candidates(vec![b])).unwrap();
    assert_eq!(result.stages[0].result.stage_report.stage, LayerAssignmentStage::Rmp);
    assert_eq!(result.stages[0].result.stage_report.report.problem_status, ProblemStatus::Infeasible);
    assert_eq!(result.stages[1].result.stage_report.stage, LayerAssignmentStage::FeasibilityLp);
    assert_eq!(result.columns.len(), 2);
    assert!(result.stages[result.final_stage.unwrap()].result.copies.is_some());

    let mut zero = input();
    zero.demands[0] = Bpp2dAssignmentDemandEntry::item_amount("a", 0).with_upper_bound(Bpp2dAssignmentDemandValue::Amount(0));
    let result = solve_bpp2d_column_generation(solver.clone(), Default::default(), &zero, &[]).unwrap();
    assert_eq!(result.stop, Bpp2dColumnGenerationStop::NoImprovingColumns);
    assert!(result.columns.is_empty());
    assert_eq!(result.stages[result.final_stage.unwrap()].result.copies, Some(Vec::new()));

    let mut circle = input();
    circle.request.items[0].0.shape = PackingShape2::Circle(CirclePackingShape2::new(meters(0.5)));
    circle.demands.push(Bpp2dAssignmentDemandEntry::item_area("a", 3.0 * std::f64::consts::PI / 4.0));
    let result = solve_bpp2d_column_generation(solver.clone(), Default::default(), &circle, &LayerGenerationContext::default_generators()).unwrap();
    assert!((result.stages[result.final_stage.unwrap()].result.objective.unwrap() - 4.000002).abs() < 1e-7);
    assert!(result.columns.iter().flat_map(|column| &column.units).all(|placement| placement.shape.is_circle()));

    let mut multiple = input();
    let small = SheetType::new(meters(1.0), meters(1.0), meters(0.0), SheetTypeId::new("small"));
    multiple.request.sheets = vec![multiple.request.sheet.clone().unwrap(), small];
    let result = solve_bpp2d_column_generation(solver, Default::default(), &multiple, &LayerGenerationContext::default_generators()).unwrap();
    assert!((result.stages[result.final_stage.unwrap()].result.objective.unwrap() - 3.000002).abs() < 1e-7);
    assert!(result.columns.iter().any(|column| column.sheet.as_ref().unwrap().type_code.as_str() == "small"));
}

#[cfg(any(feature = "scip", feature = "scip-bundled", feature = "scip-from-source"))]
#[test]
fn real_scip_column_generation_lifecycle() {
    use ospf_rust_core::solver::solvers::scip::{SCIPConfig, SCIPSolver};
    real_contract(Arc::new(SCIPSolver::with_config(SCIPConfig::default().with_output(false).with_lp_subproblem_defaults())));
}

#[cfg(any(feature = "gurobi10", feature = "gurobi11", feature = "gurobi12"))]
#[test]
fn real_gurobi_column_generation_lifecycle() {
    use ospf_rust_core::solver::solvers::gurobi::{GurobiConfig, GurobiSolver};
    real_contract(Arc::new(GurobiSolver::with_config(GurobiConfig::default().with_output(false))));
}
