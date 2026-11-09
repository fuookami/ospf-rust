use ospf_rust_core::model::MetaModel;
use ospf_rust_core::solver::{ProblemStatus, SolveFingerprints, SolveProof, SolveReport, SolveSolution, TerminationReason, linear_model_fingerprint, linear_model_mapping};
use ospf_rust_framework_csp2d::domain::produce::{Csp2dMasterProblem, Csp2dPatternAssignment, ProduceContext, ProduceModelInput};
use crate::{Csp2dDemand, Csp2dProblem, encode_csp2d_problem};
use crate::csp2d::tests::problem;
use super::*;

fn options() -> Csp2dOptions {
    Csp2dOptions { backend: NativeBackend::Scip, generators: None, iteration_limit: None, candidates_per_iteration: None, pool_limit: None,
        final_integer_enabled: None, reduced_cost_tolerance: None, report_limit: None }
}

fn column(amount: usize) -> Csp2dColumn {
    let mut column = problem().initial_columns.remove(0); column.id = format!("column-{amount}"); column.placements.truncate(amount); column
}

fn child(problem: &Csp2dProblem, columns: &[Csp2dColumn], kind: Csp2dStageKind, values: Vec<f64>, duals: Option<Vec<f64>>) -> Csp2dStageReportArtifact {
    let input = problem.domain().unwrap(); let columns = decode_csp2d_columns(&input, columns).unwrap();
    let master = Csp2dMasterProblem { assignment: Csp2dPatternAssignment { usage_upper_bounds: vec![0; columns.len()], plans: columns },
        demands: input.demands, material_availability: input.requests.iter().map(|request| (request.material.id.as_str().into(), request.material.available_batches)).collect() };
    let mut model = MetaModel::new("actual-csp2d-child");
    ProduceContext { objective: input.objective }.register_model(&mut model, ProduceModelInput { master: &master, stage: kind.domain(), kerf: &input.requests[0].kerf }).unwrap();
    let compiled = model.linear_model_snapshot().unwrap().compile_flt64().unwrap();
    let mut solution = SolveSolution::vector(values);
    let objective = compiled.model.c.iter().zip(&solution.values).map(|(cost, value)| cost * value).sum();
    solution.objective = Some(objective); solution.objective_value = Some(objective); solution.dual_solution = duals;
    let report: SolveReport<f64> = SolveReport::builder(ProblemStatus::Feasible, TerminationReason::Completed).solution(solution).proof(SolveProof::optimality())
        .fingerprints(SolveFingerprints { model: Some(linear_model_fingerprint(&compiled.model).unwrap()), ..Default::default() })
        .model_mapping(Some(linear_model_mapping(&compiled.model).unwrap())).build().unwrap();
    Csp2dStageReportArtifact { stage: kind, pool_size: master.assignment.len() as u32, model: encode_compiled_linear_model(&compiled.model).unwrap(),
        solver_report_json: serde_json::to_string(&report).unwrap(), objective_constant: compiled.objective_constant }
}

pub(crate) fn execution() -> (ModelArtifact, Csp2dExecutionReportArtifact) {
    let mut problem = problem(); problem.initial_columns = vec![column(1)];
    let columns = vec![column(1), column(2)];
    let stages = vec![child(&problem, &columns[..1], Csp2dStageKind::Rmp, vec![3.0], Some(vec![-2.0, 0.0])),
        child(&problem, &columns, Csp2dStageKind::Rmp, vec![0.0, 1.5], Some(vec![-1.0, 0.0])),
        child(&problem, &columns, Csp2dStageKind::Integer, vec![0.0, 2.0], None)];
    let source = encode_csp2d_problem(problem).unwrap();
    let artifact = Csp2dExecutionReportArtifact { schema_version: "1.0".into(), source_fingerprint: source.fingerprint.clone(), configuration: options(), columns, stages,
        iterations: vec![Csp2dPricingIteration { stage_index: 0, candidates_evaluated: 1, candidate_limit_reached: false, added_columns: vec![Csp2dPricedColumn { index: 1, reduced_cost: -2.0 }] },
            Csp2dPricingIteration { stage_index: 1, candidates_evaluated: 1, candidate_limit_reached: false, added_columns: Vec::new() }],
        stop: Csp2dExecutionStop::NoImprovingColumns, final_stage: Some(2) };
    (source, artifact)
}

fn read(source: &ModelArtifact, artifact: &Csp2dExecutionReportArtifact) -> Result<Csp2dExecutionReport> {
    decode_csp2d_execution_report(&serde_json::to_string(artifact).unwrap(), source)
}

#[test]
fn full_pool_child_identity_production_and_u64_metadata_are_revalidated_without_reencoding_raw_reports() {
    let (source, mut artifact) = execution();
    let mut report: SolveReport<f64, u64> = serde_json::from_str(&artifact.stages[0].solver_report_json).unwrap();
    report.statistics.nodes = Some(u64::MAX); report.provenance.thread_count = Some(9_007_199_254_740_993);
    artifact.stages[0].solver_report_json = serde_json::to_string(&report).unwrap();
    let result = read(&source, &artifact).unwrap();
    assert!(!result.summary.candidate_complete); assert_eq!(result.summary.column_count, 2);
    assert_eq!(result.artifact.stages[0].solver_report_json, artifact.stages[0].solver_report_json);
    let production = result.production.unwrap();
    assert_eq!(production.produced_counts["a"], "4"); assert_eq!(production.material_batches["s"], "2");
    assert_eq!(production.usages.len(), 1); assert_eq!(production.usages[0].column_index, 1);
    assert_eq!(production.consumed_area.value, 4.0); assert_eq!(production.objective_value, 4.0); assert_eq!(production.waste_area.value, 0.0);
}

#[test]
fn lifecycle_rejects_changed_prefix_duplicates_batches_costs_indices_phases_and_final() {
    for variant in 0..21 {
        let (source, mut artifact) = execution();
        match variant {
            0 => artifact.columns.swap(0, 1),
            1 => artifact.columns.push(column(2)),
            2 => artifact.iterations[0].added_columns[0].reduced_cost = -1.0,
            3 => artifact.iterations[0].added_columns[0].index = 0,
            4 => artifact.iterations[0].stage_index = 1,
            5 => artifact.iterations[1].stage_index = 0,
            6 => artifact.iterations[0].candidate_limit_reached = true,
            7 => artifact.iterations[0].candidates_evaluated = 0,
            8 => artifact.iterations[0].candidates_evaluated = 513,
            9 => artifact.stages[0].stage = Csp2dStageKind::Integer,
            10 => artifact.stages[1].pool_size = 1,
            11 => artifact.final_stage = Some(u32::MAX),
            12 => artifact.final_stage = Some(0),
            13 => artifact.final_stage = None,
            14 => artifact.configuration.final_integer_enabled = Some(false),
            15 => artifact.stop = Csp2dExecutionStop::IterationLimit,
            16 => artifact.stop = Csp2dExecutionStop::ReportLimit,
            17 => artifact.iterations.clear(),
            18 => artifact.configuration.iteration_limit = Some(1),
            19 => artifact.columns[1].placements[0].rotated = true,
            _ => artifact.stages[0].objective_constant = 1.0,
        }
        assert!(read(&source, &artifact).is_err(), "variant {variant}");
    }
}

#[test]
fn raw_report_gate_rejects_missing_identity_wrong_vectors_values_objectives_and_lp_certificates() {
    for variant in 0..11 {
        let (source, mut artifact) = execution();
        let mut report: SolveReport<f64, u64> = serde_json::from_str(&artifact.stages[0].solver_report_json).unwrap();
        match variant {
            0 => report.fingerprints.model = None,
            1 => report.model_mapping = None,
            2 => report.solution.as_mut().unwrap().values.clear(),
            3 => report.solution.as_mut().unwrap().values[0] = 1.0,
            4 => report.solution.as_mut().unwrap().objective = Some(100.0),
            5 => { report.solution.as_mut().unwrap().stable_values.insert("foreign".into(), 3.0); },
            6 => report.solution.as_mut().unwrap().dual_solution = Some(vec![0.0, 0.0]),
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
fn phase_one_recovery_checks_original_unit_deficits_and_requires_an_ordinary_rmp() {
    let mut problem = problem(); problem.initial_columns.clear();
    problem.demands = vec![Csp2dDemand::Count { product_id: "a".into(), count: "2".into(), upper_bound: None }];
    let source = encode_csp2d_problem(problem.clone()).unwrap(); let columns = vec![column(2)];
    let artifact = Csp2dExecutionReportArtifact { schema_version: "1.0".into(), source_fingerprint: source.fingerprint.clone(), configuration: options(), columns: columns.clone(),
        stages: vec![child(&problem, &[], Csp2dStageKind::FeasibilityLp, vec![2.0], Some(vec![-0.5, 0.0])),
            child(&problem, &columns, Csp2dStageKind::FeasibilityLp, vec![1.0, 0.0], Some(vec![0.0, 0.0])),
            child(&problem, &columns, Csp2dStageKind::Rmp, vec![1.0], Some(vec![-1.0, 0.0])),
            child(&problem, &columns, Csp2dStageKind::Integer, vec![1.0], None)],
        iterations: vec![Csp2dPricingIteration { stage_index: 0, candidates_evaluated: 1, candidate_limit_reached: false, added_columns: vec![Csp2dPricedColumn { index: 0, reduced_cost: -1.0 }] },
            Csp2dPricingIteration { stage_index: 2, candidates_evaluated: 0, candidate_limit_reached: false, added_columns: Vec::new() }],
        stop: Csp2dExecutionStop::NoImprovingColumns, final_stage: Some(3) };
    assert_eq!(read(&source, &artifact).unwrap().production.unwrap().produced_counts["a"], "2");
    let mut skipped = artifact; skipped.stages.remove(1); skipped.final_stage = Some(2); skipped.iterations[1].stage_index = 1;
    assert!(read(&source, &skipped).is_err());
}

#[test]
fn partial_stops_and_final_cancelled_incumbents_never_relabel_pricing_or_invent_production() {
    let (source, original) = execution();
    for stop in [Csp2dExecutionStop::Cancelled, Csp2dExecutionStop::TimeLimit, Csp2dExecutionStop::ReportLimit] {
        let mut partial = original.clone(); partial.stages.truncate(1); partial.iterations.truncate(1); partial.final_stage = None; partial.stop = stop;
        if stop == Csp2dExecutionStop::ReportLimit { partial.configuration.report_limit = Some(1); }
        let result = read(&source, &partial).unwrap(); assert!(result.production.is_none()); assert_eq!(result.summary.column_count, 2);
    }
    for reason in [TerminationReason::Cancelled, TerminationReason::TimeLimit] {
        let mut artifact = original.clone();
        let mut report: SolveReport<f64, u64> = serde_json::from_str(&artifact.stages[2].solver_report_json).unwrap();
        report.proof = None; report.solution_presence = ospf_rust_core::solver::SolutionPresence::Incumbent; report.termination_reason = reason;
        artifact.stages[2].solver_report_json = serde_json::to_string(&report).unwrap();
        let result = read(&source, &artifact).unwrap(); assert!(result.production.is_some()); assert_eq!(result.summary.stop, Csp2dExecutionStop::NoImprovingColumns);
    }
}

#[test]
fn unsupported_versions_fingerprints_and_unbounded_configuration_fail_explicitly() {
    let (source, mut artifact) = execution(); artifact.source_fingerprint = "foreign".into();
    assert_eq!(read(&source, &artifact).unwrap_err().code, "FingerprintMismatch");
    artifact.schema_version = "2.0".into(); assert_eq!(read(&source, &artifact).unwrap_err().code, "UnsupportedProtocolVersion");
    assert_eq!(decode_csp2d_execution_report(&" ".repeat(CSP2D_MAX_BYTES + 1), &source).unwrap_err().code, "PayloadTooLarge");
    for change in 0..8 {
        let mut options = options();
        match change { 0 => options.iteration_limit = Some(65), 1 => options.candidates_per_iteration = Some(0), 2 => options.pool_limit = Some(513),
            3 => options.report_limit = Some(0), 4 => options.reduced_cost_tolerance = Some(f64::NAN), 5 => options.reduced_cost_tolerance = Some(0.5),
            6 => options.generators = Some(Vec::new()), _ => options.generators = Some(vec![Csp2dGeneratorKind::Shelf; 2]) }
        assert!(options.config().is_err());
    }
}

#[test]
fn integer_production_keeps_counts_above_js_safe_range_and_nonoptimal_lps_never_price() {
    let mut problem = problem(); problem.materials[0].available_batches = "9007199254740991".into();
    problem.demands = vec![Csp2dDemand::Count { product_id: "a".into(), count: "0".into(), upper_bound: None }];
    let source = encode_csp2d_problem(problem.clone()).unwrap(); let mut configuration = options(); configuration.iteration_limit = Some(0);
    let mut artifact = Csp2dExecutionReportArtifact { schema_version: "1.0".into(), source_fingerprint: source.fingerprint.clone(), configuration,
        columns: problem.initial_columns.clone(), stages: vec![child(&problem, &problem.initial_columns, Csp2dStageKind::Rmp, vec![0.0], Some(vec![0.0, 0.0])),
            child(&problem, &problem.initial_columns, Csp2dStageKind::Integer, vec![9_007_199_254_740_991.0], None)],
        iterations: Vec::new(), stop: Csp2dExecutionStop::IterationLimit, final_stage: Some(1) };
    let mut report: SolveReport<f64, u64> = serde_json::from_str(&artifact.stages[1].solver_report_json).unwrap();
    report.proof = None; report.solution_presence = ospf_rust_core::solver::SolutionPresence::Incumbent;
    artifact.stages[1].solver_report_json = serde_json::to_string(&report).unwrap();
    let production = read(&source, &artifact).unwrap().production.unwrap();
    assert_eq!(production.produced_counts["a"], "18014398509481982"); assert_eq!(production.material_batches["s"], "9007199254740991");

    let (source, mut artifact) = execution(); artifact.columns.truncate(1); artifact.stages.truncate(1); artifact.iterations.clear();
    artifact.final_stage = None; artifact.stop = Csp2dExecutionStop::MasterStopped;
    let mut report: SolveReport<f64, u64> = serde_json::from_str(&artifact.stages[0].solver_report_json).unwrap();
    report.proof = None; report.solution_presence = ospf_rust_core::solver::SolutionPresence::Incumbent; report.solution.as_mut().unwrap().dual_solution = None;
    artifact.stages[0].solver_report_json = serde_json::to_string(&report).unwrap();
    assert!(read(&source, &artifact).unwrap().production.is_none());
    artifact.iterations.push(Csp2dPricingIteration { stage_index: 0, candidates_evaluated: 0, candidate_limit_reached: false, added_columns: Vec::new() });
    assert!(read(&source, &artifact).is_err());
}

#[test]
fn independent_valid_reports_from_other_master_mathematics_cannot_replace_source_children() {
    for variant in 0..3 {
        let (source, mut artifact) = execution(); let (mut foreign, _) = decode_csp2d_problem(&source).unwrap();
        let dual = match variant {
            0 => { foreign.materials[0].available_batches = "11".into(); -2.0 },
            1 => { foreign.objective.area_weight = 2.0; -4.0 },
            _ => { foreign.demands[0] = Csp2dDemand::Count { product_id: "a".into(), count: "2".into(), upper_bound: None }; -2.0 },
        };
        let actual = child(&foreign, &artifact.columns[..1], Csp2dStageKind::Rmp, vec![if variant == 2 { 2.0 } else { 3.0 }], Some(vec![dual, 0.0]));
        crate::decode_csp1d_stage_report(&crate::Csp1dStageReportArtifact { schema_version: "1.0".into(), stage: crate::Csp1dStageKind::Lp,
            model: actual.model.clone(), solver_report_json: actual.solver_report_json.clone(), objective_constant: actual.objective_constant }).unwrap();
        artifact.stages[0] = actual; assert!(read(&source, &artifact).is_err(), "variant {variant}");
    }
}

#[test]
fn a_tiny_normalized_phase_one_objective_never_erases_nonzero_original_deficits() {
    let mut problem = problem(); problem.initial_columns.clear(); problem.materials[0].available_batches = "1000000000000".into();
    problem.demands = vec![Csp2dDemand::Count { product_id: "a".into(), count: "1000000000000".into(), upper_bound: None }];
    let source = encode_csp2d_problem(problem.clone()).unwrap(); let columns = vec![column(1)];
    let mut configuration = options(); configuration.final_integer_enabled = Some(false); configuration.reduced_cost_tolerance = Some(0.0);
    let mut artifact = Csp2dExecutionReportArtifact { schema_version: "1.0".into(), source_fingerprint: source.fingerprint.clone(), configuration,
        columns: columns.clone(), stages: vec![child(&problem, &[], Csp2dStageKind::FeasibilityLp, vec![1e12], Some(vec![-1e-12, 0.0])),
            child(&problem, &columns, Csp2dStageKind::FeasibilityLp, vec![999_999_999_999.0, 1.0], Some(vec![0.0, 0.0]))],
        iterations: vec![Csp2dPricingIteration { stage_index: 0, candidates_evaluated: 1, candidate_limit_reached: false, added_columns: vec![Csp2dPricedColumn { index: 0, reduced_cost: -1e-12 }] },
            Csp2dPricingIteration { stage_index: 1, candidates_evaluated: 0, candidate_limit_reached: false, added_columns: Vec::new() }],
        stop: Csp2dExecutionStop::NoImprovingColumns, final_stage: None };
    let decoded = read(&source, &artifact).unwrap(); assert!(decoded.stages[1].objective_value.unwrap() < 1e-6); assert!(decoded.production.is_none());
    artifact.stages.push(child(&problem, &columns, Csp2dStageKind::Rmp, vec![1e12], Some(vec![-2.0, 0.0])));
    artifact.iterations[1].stage_index = 2;
    assert!(read(&source, &artifact).is_err());
}
