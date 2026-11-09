use ospf_rust_core::model::MetaModel;
use ospf_rust_core::solver::{ProblemStatus, SolveFingerprints, SolveProof, SolveReport, SolveSolution, TerminationReason, linear_model_fingerprint, linear_model_mapping};
use ospf_rust_framework_bpp2d::domain::layer_assignment::{LayerAssignmentContext, LayerAssignmentModelInput};
use crate::{Bpp2dDemand, Bpp2dProblem, encode_bpp2d_problem};
use crate::bpp2d::tests::{layer, problem};
use super::*;

fn options() -> Bpp2dOptions {
    Bpp2dOptions { backend: NativeBackend::Scip, generators: None, iteration_limit: None, candidates_per_iteration: None, pool_limit: None,
        final_milp_enabled: None, reduced_cost_tolerance: None, report_limit: None }
}

fn child(problem: &Bpp2dProblem, layers: &[Bpp2dLayer], kind: Bpp2dStageKind, values: Vec<f64>, duals: Option<Vec<f64>>) -> Bpp2dStageReportArtifact {
    let input = problem.domain().unwrap();
    let layers = decode_bpp2d_layers(&input.request, &input.demands, layers).unwrap();
    let mut model = MetaModel::new("actual-bpp2d-child");
    LayerAssignmentContext::new(input.objective).register_model(&mut model, LayerAssignmentModelInput { layers: &layers, demands: &input.demands, stage: kind.domain() }).unwrap();
    let compiled = model.linear_model_snapshot().unwrap().compile_flt64().unwrap();
    let mut solution = SolveSolution::vector(values);
    let objective = compiled.model.c.iter().zip(&solution.values).map(|(cost, value)| cost * value).sum();
    solution.objective = Some(objective); solution.objective_value = Some(objective);
    solution.dual_solution = duals;
    let report: SolveReport<f64> = SolveReport::builder(ProblemStatus::Feasible, TerminationReason::Completed).solution(solution).proof(SolveProof::optimality())
        .fingerprints(SolveFingerprints { model: Some(linear_model_fingerprint(&compiled.model).unwrap()), ..Default::default() })
        .model_mapping(Some(linear_model_mapping(&compiled.model).unwrap())).build().unwrap();
    Bpp2dStageReportArtifact { stage: kind, pool_size: layers.len() as u32, model: encode_compiled_linear_model(&compiled.model).unwrap(),
        solver_report_json: serde_json::to_string(&report).unwrap(), objective_constant: compiled.objective_constant }
}

fn execution() -> (ModelArtifact, Bpp2dExecutionReportArtifact) {
    let source = problem();
    let columns = vec![layer(1), layer(2)];
    let stages = vec![child(&source, &columns[..1], Bpp2dStageKind::Rmp, vec![3.0], Some(vec![-2.0])),
        child(&source, &columns, Bpp2dStageKind::Rmp, vec![0.0, 1.5], Some(vec![-1.0])),
        child(&source, &columns, Bpp2dStageKind::Integer, vec![0.0, 2.0], None)];
    let source = encode_bpp2d_problem(source).unwrap();
    let artifact = Bpp2dExecutionReportArtifact { schema_version: "1.0".into(), source_fingerprint: source.fingerprint.clone(), configuration: options(), columns, stages,
        iterations: vec![Bpp2dPricingIteration { stage_index: 0, candidates_evaluated: 1, candidate_limit_reached: false, added_columns: vec![Bpp2dPricedColumn { index: 1, reduced_cost: -2.0 }] },
            Bpp2dPricingIteration { stage_index: 1, candidates_evaluated: 1, candidate_limit_reached: false, added_columns: Vec::new() }],
        stop: Bpp2dExecutionStop::NoImprovingColumns, final_stage: Some(2) };
    (source, artifact)
}

fn read(source: &ModelArtifact, artifact: &Bpp2dExecutionReportArtifact) -> Result<Bpp2dExecutionReport> {
    decode_bpp2d_execution_report(&serde_json::to_string(artifact).unwrap(), source)
}

#[test]
fn full_pool_source_binding_and_final_production_preserve_original_reports_and_u64() {
    let (source, mut artifact) = execution();
    let mut report: SolveReport<f64, u64> = serde_json::from_str(&artifact.stages[0].solver_report_json).unwrap();
    report.statistics.nodes = Some(u64::MAX); report.provenance.thread_count = Some(9_007_199_254_740_993);
    artifact.stages[0].solver_report_json = serde_json::to_string(&report).unwrap();
    let result = read(&source, &artifact).unwrap();
    assert!(!result.summary.candidate_complete); assert_eq!(result.summary.column_count, 2);
    assert_eq!(result.artifact.stages[0].solver_report_json, artifact.stages[0].solver_report_json);
    let production = result.production.unwrap();
    assert_eq!(production.produced_counts["a"], "4"); assert_eq!(production.sheet_counts["s"], "2");
    assert_eq!(production.usages.len(), 1); assert_eq!(production.usages[0].column_index, 1);
    assert_eq!(production.consumed_area.value, 4.0); assert_eq!(production.objective_value, 4.000002);
}

#[test]
fn lifecycle_rejects_tampered_prefix_batches_costs_indices_phases_and_final() {
    for variant in 0..20 {
        let (source, mut artifact) = execution();
        match variant {
            0 => artifact.columns.swap(0, 1),
            1 => artifact.columns.push(layer(2)),
            2 => artifact.iterations[0].added_columns[0].reduced_cost = -1.0,
            3 => artifact.iterations[0].added_columns[0].index = 0,
            4 => artifact.iterations[0].stage_index = 1,
            5 => artifact.iterations[1].stage_index = 0,
            6 => artifact.iterations[0].candidate_limit_reached = true,
            7 => artifact.iterations[0].candidates_evaluated = 0,
            8 => artifact.iterations[0].candidates_evaluated = 513,
            9 => artifact.stages[0].stage = Bpp2dStageKind::Integer,
            10 => artifact.stages[1].pool_size = 1,
            11 => artifact.final_stage = Some(u32::MAX),
            12 => artifact.final_stage = Some(0),
            13 => artifact.final_stage = None,
            14 => artifact.configuration.final_milp_enabled = Some(false),
            15 => artifact.stop = Bpp2dExecutionStop::IterationLimit,
            16 => artifact.stop = Bpp2dExecutionStop::ReportLimit,
            17 => artifact.iterations.clear(),
            18 => artifact.configuration.iteration_limit = Some(1),
            _ => artifact.stages[0].objective_constant = 1.0,
        }
        assert!(read(&source, &artifact).is_err(), "variant {variant}");
    }
}

#[test]
fn stage_gate_rejects_foreign_identity_wrong_vectors_stable_values_objectives_and_duals() {
    for variant in 0..11 {
        let (source, mut artifact) = execution();
        let mut report: SolveReport<f64, u64> = serde_json::from_str(&artifact.stages[0].solver_report_json).unwrap();
        match variant {
            0 => report.fingerprints.model = None,
            1 => report.model_mapping = None,
            2 => report.solution.as_mut().unwrap().values.clear(),
            3 => report.solution.as_mut().unwrap().values[0] = 1.0,
            4 => report.solution.as_mut().unwrap().objective = Some(100.0),
            5 => report.solution.as_mut().unwrap().stable_values.insert("foreign".into(), 3.0).map(|_| ()).unwrap_or(()),
            6 => report.solution.as_mut().unwrap().dual_solution = Some(vec![0.0]),
            7 => report.schema_version = "2.0".into(),
            8 => report.solution.as_mut().unwrap().value = Some(3.0),
            9 => { let id = report.model_mapping.as_ref().unwrap().columns_by_variable.keys().next().unwrap().clone(); report.solution.as_mut().unwrap().stable_values.insert(id, 4.0); },
            _ => report.solution.as_mut().unwrap().objective_value = Some(100.0),
        }
        artifact.stages[0].solver_report_json = serde_json::to_string(&report).unwrap();
        assert!(read(&source, &artifact).is_err(), "variant {variant}");
    }
}

#[test]
fn phase_one_recovery_requires_zero_deficit_then_an_ordinary_rmp() {
    let mut problem = problem(); problem.initial_layers.clear();
    problem.demands = vec![Bpp2dDemand::Count { item_id: "a".into(), count: "2".into(), upper_bound: None }];
    let source = encode_bpp2d_problem(problem.clone()).unwrap();
    let columns = vec![layer(2)];
    let artifact = Bpp2dExecutionReportArtifact { schema_version: "1.0".into(), source_fingerprint: source.fingerprint.clone(), configuration: options(), columns: columns.clone(),
        stages: vec![child(&problem, &[], Bpp2dStageKind::FeasibilityLp, vec![2.0], Some(vec![-0.5])),
            child(&problem, &columns, Bpp2dStageKind::FeasibilityLp, vec![1.0, 0.0], Some(vec![0.0])),
            child(&problem, &columns, Bpp2dStageKind::Rmp, vec![1.0], Some(vec![-1.0])),
            child(&problem, &columns, Bpp2dStageKind::Integer, vec![1.0], None)],
        iterations: vec![Bpp2dPricingIteration { stage_index: 0, candidates_evaluated: 1, candidate_limit_reached: false, added_columns: vec![Bpp2dPricedColumn { index: 0, reduced_cost: -1.0 }] },
            Bpp2dPricingIteration { stage_index: 2, candidates_evaluated: 0, candidate_limit_reached: false, added_columns: Vec::new() }],
        stop: Bpp2dExecutionStop::NoImprovingColumns, final_stage: Some(3) };
    assert_eq!(read(&source, &artifact).unwrap().production.unwrap().produced_counts["a"], "2");
    let mut skipped = artifact; skipped.stages.remove(1); skipped.final_stage = Some(2); skipped.iterations[1].stage_index = 1;
    assert!(read(&source, &skipped).is_err());
}

#[test]
fn external_stops_preserve_partial_pool_and_final_incumbent_without_relabeling_pricing() {
    let (source, original) = execution();
    for stop in [Bpp2dExecutionStop::Cancelled, Bpp2dExecutionStop::TimeLimit, Bpp2dExecutionStop::ReportLimit] {
        let mut partial = original.clone(); partial.stages.truncate(1); partial.iterations.truncate(1); partial.final_stage = None; partial.stop = stop;
        if stop == Bpp2dExecutionStop::ReportLimit { partial.configuration.report_limit = Some(1); }
        let result = read(&source, &partial).unwrap(); assert!(result.production.is_none()); assert_eq!(result.summary.column_count, 2);
    }
    for reason in [TerminationReason::Cancelled, TerminationReason::TimeLimit] {
        let mut artifact = original.clone();
        let mut report: SolveReport<f64, u64> = serde_json::from_str(&artifact.stages[2].solver_report_json).unwrap();
        report.proof = None; report.solution_presence = ospf_rust_core::solver::SolutionPresence::Incumbent; report.termination_reason = reason;
        artifact.stages[2].solver_report_json = serde_json::to_string(&report).unwrap();
        let result = read(&source, &artifact).unwrap(); assert!(result.production.is_some()); assert_eq!(result.summary.stop, Bpp2dExecutionStop::NoImprovingColumns);
    }
}

#[test]
fn nonoptimal_lp_does_not_price_and_production_keeps_counts_above_js_safe_integer() {
    let mut problem = problem(); problem.initial_layers = vec![layer(2)]; problem.demands = vec![Bpp2dDemand::Count { item_id: "a".into(), count: "0".into(), upper_bound: None }];
    let source = encode_bpp2d_problem(problem.clone()).unwrap();
    let mut configuration = options(); configuration.iteration_limit = Some(0);
    let mut artifact = Bpp2dExecutionReportArtifact { schema_version: "1.0".into(), source_fingerprint: source.fingerprint.clone(), configuration,
        columns: problem.initial_layers.clone(), stages: vec![child(&problem, &problem.initial_layers, Bpp2dStageKind::Rmp, vec![0.0], Some(vec![0.0])),
            child(&problem, &problem.initial_layers, Bpp2dStageKind::Integer, vec![9_007_199_254_740_991.0], None)],
        iterations: Vec::new(), stop: Bpp2dExecutionStop::IterationLimit, final_stage: Some(1) };
    // 大份数只声称可行，不伪造整数最优性。 / Large copies claim feasibility only, not integer optimality.
    let mut report: SolveReport<f64, u64> = serde_json::from_str(&artifact.stages[1].solver_report_json).unwrap();
    report.proof = None; report.solution_presence = ospf_rust_core::solver::SolutionPresence::Incumbent;
    artifact.stages[1].solver_report_json = serde_json::to_string(&report).unwrap();
    assert_eq!(read(&source, &artifact).unwrap().production.unwrap().produced_counts["a"], "18014398509481982");
    artifact.stages.truncate(1); artifact.final_stage = None; artifact.stop = Bpp2dExecutionStop::MasterStopped;
    let mut report: SolveReport<f64, u64> = serde_json::from_str(&artifact.stages[0].solver_report_json).unwrap();
    report.proof = None; report.solution_presence = ospf_rust_core::solver::SolutionPresence::Incumbent;
    artifact.stages[0].solver_report_json = serde_json::to_string(&report).unwrap();
    assert!(read(&source, &artifact).unwrap().production.is_none());
}

#[test]
fn mathematically_valid_foreign_children_cannot_replace_the_source_master() {
    for variant in 0..5 {
        let (source, mut artifact) = execution();
        let mut foreign = problem();
        match variant {
            0 => foreign.objective.rmp_area_coefficient = 2.0,
            1 => foreign.demands = vec![Bpp2dDemand::Count { item_id: "a".into(), count: "4".into(), upper_bound: None }],
            2 => foreign.objective.sheet_amount_weight = 1.0,
            3 => foreign.demands = vec![Bpp2dDemand::Count { item_id: "a".into(), count: "3".into(), upper_bound: Some("4".into()) }],
            _ => {},
        }
        if variant == 2 {
            artifact.stages[2] = child(&foreign, &artifact.columns, Bpp2dStageKind::Integer, vec![0.0, 2.0], None);
        } else if variant == 4 {
            artifact.stages[0] = child(&foreign, &artifact.columns[..1], Bpp2dStageKind::Integer, vec![3.0], None);
            artifact.stages[0].stage = Bpp2dStageKind::Rmp;
        } else {
            let amount = if variant == 1 { 4.0 } else { 3.0 };
            let duals = if variant == 3 { vec![-2.0, 0.0] } else { vec![if variant == 0 { -4.0 } else { -2.0 }] };
            artifact.stages[0] = child(&foreign, &artifact.columns[..1], Bpp2dStageKind::Rmp, vec![amount], Some(duals));
        }
        assert!(read(&source, &artifact).is_err(), "variant {variant}");
    }
}

#[test]
fn preexecution_stops_and_configuration_errors_do_not_fabricate_empty_success() {
    let (source, mut artifact) = execution();
    artifact.columns.truncate(1); artifact.stages.clear(); artifact.iterations.clear(); artifact.final_stage = None;
    for stop in [Bpp2dExecutionStop::Cancelled, Bpp2dExecutionStop::TimeLimit] {
        artifact.stop = stop;
        let result = read(&source, &artifact).unwrap(); assert_eq!(result.summary.stage_count, 0); assert!(result.production.is_none());
    }
    for stop in [Bpp2dExecutionStop::NoImprovingColumns, Bpp2dExecutionStop::MasterStopped, Bpp2dExecutionStop::ReportLimit] {
        artifact.stop = stop; assert!(read(&source, &artifact).is_err());
    }
    for variant in 0..8 {
        let mut options = options();
        match variant {
            0 => options.iteration_limit = Some(65),
            1 => options.candidates_per_iteration = Some(0),
            2 => options.pool_limit = Some(513),
            3 => options.report_limit = Some(0),
            4 => options.reduced_cost_tolerance = Some(f64::NAN),
            5 => options.reduced_cost_tolerance = Some(0.5),
            6 => options.generators = Some(vec![]),
            _ => options.generators = Some(vec![Bpp2dGeneratorKind::Pattern, Bpp2dGeneratorKind::Pattern]),
        }
        assert!(options.config().is_err());
    }
}
