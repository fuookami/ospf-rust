use ospf_rust_core::solver::{require_infeasibility_certificate_for_model, require_optimal_lp_certificate_for_linear_model};
use crate::domain::pattern_generation::ShelfPatternGenerator2;
use crate::domain::produce::Csp2dDemandShadowPriceKey;
use super::*;

fn verify_trace(result: &Csp2dColumnGenerationResult) {
    assert!(!result.candidate_complete);
    for stage in &result.stages {
        let report = &stage.result.stage_report;
        assert_eq!(report.report.fingerprints.model, Some(linear_model_fingerprint(&report.model).unwrap()));
        assert_eq!(report.report.model_mapping, Some(linear_model_mapping(&report.model).unwrap()));
        if let Some(usages) = &stage.result.usages { assert_eq!(usages.len(), stage.pool_size); }
        if report.report.is_optimal() && report.stage != ProduceStage::Integer {
            require_optimal_lp_certificate_for_linear_model(&report.report, &report.model).unwrap();
        }
        if report.report.problem_status == ProblemStatus::Infeasible {
            require_infeasibility_certificate_for_model(&report.report, &linear_model_fingerprint(&report.model).unwrap()).unwrap();
        }
    }
    for iteration in &result.iterations {
        let source = &result.stages[iteration.stage_index];
        assert!(source.result.shadow_prices.is_some());
        for (index, price) in &iteration.added_columns {
            assert!(*index >= source.pool_size && *index < result.columns.len() && *price < 0.0);
        }
    }
}

fn real_workflows(solver: Arc<dyn LinearSolver>) {
    let progress = Arc::new(AtomicUsize::new(0)); let observed = progress.clone();
    let execution = Csp2dExecutionOptions { progress: Some(Arc::new(move |_| { observed.fetch_add(1, Ordering::SeqCst); Ok(()) })), ..Default::default() };
    let mut request = input(); request.initial_columns = vec![column(1)];
    let result = solve_csp2d_column_generation(solver.clone(), execution, &request, &candidates(vec![column(2)])).unwrap();
    verify_trace(&result);
    assert_eq!(result.stop, Csp2dColumnGenerationStop::NoImprovingColumns); assert_eq!(result.columns.len(), 2);
    let final_result = &result.stages[result.final_stage.unwrap()].result;
    assert_eq!(final_result.usages, Some(vec![0.0, 2.0])); assert!((final_result.objective.unwrap() - 4.0).abs() < 1e-7);
    assert!(progress.load(Ordering::SeqCst) > 0);

    let shelf = vec![Box::new(ShelfPatternGenerator2::new()) as Box<dyn PatternGenerator2<f64, Meter>>];
    let result = solve_csp2d_column_generation(solver.clone(), Default::default(), &input(), &shelf).unwrap();
    verify_trace(&result);
    assert_eq!(result.stages[0].result.stage_report.stage, ProduceStage::FeasibilityLp);
    assert_eq!(result.stages[0].result.deficits, Some(vec![3.0]));
    assert!(result.stages.iter().any(|stage| stage.result.stage_report.stage == ProduceStage::Rmp));
    assert_eq!(result.stages[result.final_stage.unwrap()].result.usages, Some(vec![1.0]));
    assert!((result.stages[result.final_stage.unwrap()].result.objective.unwrap() - 4.0).abs() < 1e-7);

    let result = solve_csp2d_column_generation(solver.clone(), Default::default(), &input(), &[]).unwrap();
    verify_trace(&result);
    assert!(result.columns.is_empty());
    assert_eq!(result.stages[result.final_stage.unwrap()].result.stage_report.report.problem_status, ProblemStatus::Infeasible);
    assert!(!result.candidate_complete);

    let mut uncovered = input(); uncovered.initial_columns = vec![column(1)];
    let b = Product2::new(ProductId2::new("b"), "B", meters(1.0), meters(1.0)).with_allow_rotate(false);
    uncovered.requests[0].products.push((b, 1)); uncovered.demands.push(Csp2dMasterDemand::new(ProductId2::new("b"), DemandValue2::Amount(1)));
    let mut b_column = column(1); b_column.pattern.placements[0].item_id = Some("b".into());
    let result = solve_csp2d_column_generation(solver.clone(), Default::default(), &uncovered, &candidates(vec![b_column])).unwrap();
    verify_trace(&result);
    assert_eq!(result.stages[0].result.stage_report.report.problem_status, ProblemStatus::Infeasible);
    assert_eq!(result.stages[1].result.stage_report.stage, ProduceStage::FeasibilityLp);
    assert_eq!(result.stages[result.final_stage.unwrap()].result.usages, Some(vec![3.0, 1.0]));

    let mut stocks = input(); stocks.demands[0].demand = DemandValue2::Amount(2); stocks.requests[0].material.available_batches = 1;
    let mut expensive = stocks.requests[0].clone(); expensive.material.id = MaterialId2::new("t"); expensive.material.available_batches = 10;
    stocks.requests.push(expensive);
    let mut cheap_column = column(1); cheap_column.pattern.material = stocks.requests[0].material.clone(); cheap_column.pattern.region.width = meters(2.0);
    stocks.initial_columns = vec![cheap_column];
    let generators = vec![Box::new(Generator(|request: &PatternGenerationRequest2<f64, Meter>| {
        let mut column = column(1); column.pattern.material = request.material.clone();
        if request.material.id.as_str() == "s" { column.pattern.region.width = meters(2.0); }
        vec![column.pattern]
    })) as Box<dyn PatternGenerator2<f64, Meter>>];
    let result = solve_csp2d_column_generation(solver.clone(), Default::default(), &stocks, &generators).unwrap();
    verify_trace(&result);
    assert_eq!(result.columns.len(), 2);
    let rmp = result.stages.iter().rev().find(|stage| stage.result.stage_report.stage == ProduceStage::Rmp).unwrap();
    let prices = rmp.result.shadow_prices.as_ref().unwrap();
    assert!((prices.materials["s"] + 2.0).abs() < 1e-7); assert!(prices.materials["t"].abs() < 1e-7);
    assert!((prices.demands[&Csp2dDemandShadowPriceKey { product_id: "a".into(), mode: crate::domain::material::DemandMode2::Amount }] - 4.0).abs() < 1e-7);
    let final_result = &result.stages[result.final_stage.unwrap()].result;
    assert_eq!(final_result.usages, Some(vec![1.0, 1.0])); assert!((final_result.objective.unwrap() - 6.0).abs() < 1e-7);

    let mut exact = input(); exact.initial_columns = vec![column(2)]; exact.demands[0].upper_bound = Some(DemandValue2::Amount(3));
    let result = solve_csp2d_column_generation(solver.clone(), Default::default(), &exact, &[]).unwrap();
    verify_trace(&result);
    assert_eq!(result.stages[0].result.usages, Some(vec![1.5]));
    let final_result = &result.stages[result.final_stage.unwrap()].result;
    assert_eq!(final_result.stage_report.report.problem_status, ProblemStatus::Infeasible); assert!(final_result.usages.is_none());

    let mut coil = input(); coil.requests[0].material.height = None; coil.requests[0].material.length = Some(meters(1.5));
    coil.objective = ProduceObjective { area_weight: 1.0, waste_weight: 2.0, height_weight: 3.0, usage_weight: 4.0 };
    let result = solve_csp2d_column_generation(solver.clone(), Default::default(), &coil, &shelf).unwrap();
    verify_trace(&result);
    assert!(result.columns.iter().all(|column| column.pattern.region.height.value <= 1.5));
    assert!((result.stages[result.final_stage.unwrap()].result.objective.unwrap() - 11.0).abs() < 1e-7);

    let mut guillotine = input(); guillotine.requests[0].kerf = meters(0.25);
    guillotine.requests[0].process = crate::infrastructure::CuttingProcess2::GuillotineTwoStage;
    let mut cut_column = column(2); cut_column.pattern.process = guillotine.requests[0].process;
    cut_column.pattern.region.width = meters(2.25); cut_column.pattern.placements[1].position.x = meters(1.25);
    cut_column.pattern.cut_tree = Some(crate::infrastructure::CutTree2::cut(crate::infrastructure::CutLine2 {
        orientation: crate::infrastructure::CutOrientation2::Vertical, position: meters(1.0), stage: 1,
    }, cut_column.pattern.region.clone(), meters(0.25),
        crate::infrastructure::CutTree2::leaf(Region2::new(meters(0.0), meters(0.0), meters(1.0), meters(1.0)), Some("a".into())),
        crate::infrastructure::CutTree2::leaf(Region2::new(meters(1.25), meters(0.0), meters(1.0), meters(1.0)), Some("a".into()))).unwrap());
    let result = solve_csp2d_column_generation(solver, Default::default(), &guillotine, &candidates(vec![cut_column])).unwrap();
    verify_trace(&result);
    assert_eq!(result.stages[result.final_stage.unwrap()].result.usages, Some(vec![2.0]));
    assert!((result.stages[result.final_stage.unwrap()].result.objective.unwrap() - 4.5).abs() < 1e-7);
}

#[cfg(any(feature = "scip", feature = "scip-bundled", feature = "scip-from-source"))]
#[test]
fn real_scip_runs_phase_one_recovery_inventory_pricing_integer_reports_and_original_shelf() {
    use ospf_rust_core::solver::solvers::scip::{SCIPConfig, SCIPSolver};
    real_workflows(Arc::new(SCIPSolver::with_config(SCIPConfig::default().with_output(false).with_lp_subproblem_defaults())));
}

#[cfg(any(feature = "gurobi10", feature = "gurobi11", feature = "gurobi12"))]
#[test]
fn real_gurobi_runs_phase_one_recovery_inventory_pricing_integer_reports_and_original_shelf() {
    use ospf_rust_core::solver::solvers::gurobi::{GurobiConfig, GurobiSolver};
    real_workflows(Arc::new(GurobiSolver::with_config(GurobiConfig::default().with_output(false))));
}
