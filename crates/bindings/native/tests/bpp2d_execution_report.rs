//! 真实后端到共享源绑定 codec 的验收，不以编译代替求解。 / Acceptance from real backends through the shared source-bound codec, not compilation alone.

#![cfg(any(feature = "scip", feature = "gurobi10", feature = "gurobi11", feature = "gurobi12"))]

use std::sync::Arc;
use std::time::Duration;
use ospf_rust_bridge::*;
use ospf_rust_core::solver::{CancellationOrigin, LinearSolver, SolveHandle};
use ospf_rust_framework_bpp2d::application::{Bpp2dExecutionOptions, solve_bpp2d_column_generation};

fn problem() -> Bpp2dProblem {
    serde_json::from_str(r#"{"items":[{"id":"a","name":"A","shape":{"kind":"rectangle","width":{"value":100,"unit":"cm"},"height":{"value":1,"unit":"m"}},"allowRotate":false,"generationCount":"3"}],"sheets":[{"id":"s","geometry":{"width":{"value":2,"unit":"m"},"height":{"value":1,"unit":"m"}},"isMain":true}],"demands":[{"kind":"count","itemId":"a","count":"3","upperBound":null}],"initialLayers":[],"kerf":{"value":0,"unit":"m"},"objective":{"sheetAreaWeight":1,"sheetAmountWeight":0.000001,"sheetTypeCostWeight":0,"sheetTypeCosts":{},"rmpAreaCoefficient":1}}"#).unwrap()
}

fn configuration(backend: NativeBackend) -> Bpp2dOptions {
    Bpp2dOptions { backend, generators: None, iteration_limit: None, candidates_per_iteration: None, pool_limit: None,
        final_milp_enabled: None, reduced_cost_tolerance: None, report_limit: None }
}

fn contract(solver: Arc<dyn LinearSolver>, backend: NativeBackend) {
    let mut fixtures = Vec::new();
    for variant in 0..12 {
        let mut problem = problem();
        let mut configuration = configuration(backend);
        let mut execution = Bpp2dExecutionOptions::default();
        match variant {
            1 => {
                problem.initial_layers = serde_json::from_str(r#"[{"sheetId":"s","envelope":{"width":{"value":1,"unit":"m"},"height":{"value":1,"unit":"m"}},"placements":[{"shape":{"kind":"rectangle","width":{"value":1,"unit":"m"},"height":{"value":1,"unit":"m"}},"position":{"x":{"value":0,"unit":"m"},"y":{"value":0,"unit":"m"}},"rotated":false,"itemId":"a"}]}]"#).unwrap();
                problem.initial_layers.push(problem.initial_layers[0].clone());
            },
            2 => problem.demands.push(Bpp2dDemand::Area { item_id: "a".into(), area: PhysicalQuantity { value: 30_000.0, unit: QuantityUnit::SquareCentimeter }, upper_bound: None }),
            3 => { if let Bpp2dDemand::Count { upper_bound, .. } = &mut problem.demands[0] { *upper_bound = Some("3".into()); } },
            4 => {
                problem.items[0].shape = Geometry2dShape::Circle { radius: PhysicalQuantity { value: 0.5, unit: QuantityUnit::Meter } };
                configuration.generators = Some(vec![Bpp2dGeneratorKind::CircleGrid]);
            },
            5 => {
                let mut sheet = problem.sheets[0].clone(); sheet.id = "small".into(); sheet.geometry.width.value = 1.0; sheet.is_main = false;
                problem.sheets.push(sheet);
            },
            6 => problem.demands = vec![Bpp2dDemand::Count { item_id: "a".into(), count: "0".into(), upper_bound: None }],
            7 => { let handle = SolveHandle::new(); handle.cancel(CancellationOrigin::External); execution.cancellation = handle; },
            8 => execution.time_limit = Duration::ZERO,
            10 => { if let Bpp2dDemand::Count { upper_bound, .. } = &mut problem.demands[0] { *upper_bound = Some("4".into()); } },
            11 => {
                let mut other = problem.items[0].clone(); other.id = "b".into(); problem.items.push(other);
                problem.demands.push(Bpp2dDemand::Count { item_id: "b".into(), count: "3".into(), upper_bound: None });
                problem.initial_layers = serde_json::from_str(r#"[{"sheetId":"s","envelope":{"width":{"value":1,"unit":"m"},"height":{"value":1,"unit":"m"}},"placements":[{"shape":{"kind":"rectangle","width":{"value":1,"unit":"m"},"height":{"value":1,"unit":"m"}},"position":{"x":{"value":0,"unit":"m"},"y":{"value":0,"unit":"m"}},"rotated":false,"itemId":"a"}]}]"#).unwrap();
            },
            _ => {},
        }
        if variant == 9 { configuration.report_limit = Some(1); }
        let source = encode_bpp2d_problem(problem).unwrap();
        let (_, mut input) = decode_bpp2d_problem(&source).unwrap(); input.config = configuration.config().unwrap();
        execution.report_limit = configuration.report_limit.unwrap_or(196) as usize;
        let result = solve_bpp2d_column_generation(solver.clone(), execution, &input, &configuration.generators().unwrap()).unwrap();
        let text = encode_bpp2d_execution_report(&source, configuration, &result).unwrap_or_else(|error| panic!("variant {variant}: {error}"));
        let decoded = decode_bpp2d_execution_report(&text, &source).unwrap();
        assert!(!decoded.summary.candidate_complete);
        assert_eq!(decoded.summary.column_count as usize, result.columns.len());
        assert_eq!(decoded.artifact.stages.len(), result.stages.len());
        for (actual, original) in decoded.artifact.stages.iter().zip(&result.stages) {
            assert_eq!(actual.solver_report_json, serde_json::to_string(&original.result.stage_report.report).unwrap());
        }
        if variant == 3 {
            // 只有两件列时，精确三件需求在当前池内整数不可行，不代表原问题不可行。 / With only two-piece columns, exact-three demand is integer-infeasible in this pool, not in the source problem.
            assert!(decoded.production.is_none());
            assert!(decoded.summary.integer.as_ref().unwrap().restricted_infeasible);
        } else if variant < 7 || variant >= 10 {
            let production = decoded.production.as_ref().unwrap_or_else(|| panic!("variant {variant}: {:?}", decoded.summary));
            if variant == 6 { assert_eq!(production.produced_counts["a"], "0"); assert!(production.usages.is_empty()); }
            else { assert!(production.produced_counts["a"].parse::<u128>().unwrap() >= 3); }
            if variant == 10 { assert_eq!(production.produced_counts["a"], "4"); }
            if variant == 5 { assert!((production.objective_value - 3.000002).abs() < 1e-9); }
            if variant == 11 {
                assert!(production.produced_counts["b"].parse::<u128>().unwrap() >= 3);
                assert!(decoded.stages[0].restricted_infeasible);
                assert_eq!(decoded.stages[1].stage, Bpp2dStageKind::FeasibilityLp);
            }
        } else {
            assert!(decoded.production.is_none());
            assert_eq!(decoded.summary.stop, match variant { 7 => Bpp2dExecutionStop::Cancelled, 8 => Bpp2dExecutionStop::TimeLimit, _ => Bpp2dExecutionStop::ReportLimit });
        }
        fixtures.push(serde_json::json!({ "source": source, "reportJson": text,
            "productionJson": decoded.production.as_ref().map(|value| serde_json::to_string(value).unwrap()) }));
    }
    // 显式联调证据只写调用方指定目录，不入库。 / Explicit integration evidence is written only to a caller-specified directory and stays out of Git.
    if let Some(directory) = std::env::var_os("OSPF_BPP2D_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory); std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(directory.join(format!("bpp2d-execution-{}.json", backend.id())), serde_json::to_string(&fixtures).unwrap()).unwrap();
    }
}

fn engine_contract(backend: NativeBackend) {
    use std::time::Instant;
    let engine = ospf_rust_native::NativeEngine::new(1);
    let source = encode_bpp2d_problem(problem()).unwrap();
    for variant in 0..4 {
        let mut options = configuration(backend);
        if variant == 1 { options.report_limit = Some(1); }
        if variant == 2 { options.final_milp_enabled = Some(false); }
        let request = SolveRequest { schedule_log_probe: None, operator_capability_probe: None, hypervolume_probe: None, acceptance_recovery_probe: None, training_split_probe: None, non_stationary_probe: None, selection_policy_probe: None, solve_value_probe: None, native_lowering_probe: None, native_terminal_proof_probe: None, predicate_scope_probe: None, persistence_schema_probe: None, persistence_sort_probe: None, effectiveness_ranking_probe: None, funnel_ranking: None, sensitivity_probe: None, correction_probe: None, blocking_set: None, fingerprint_probe: None, diagnostics_probe: None, gap_probe: None, proof_probe: None, evidence_probe: None, band_probe: None, group_summary: None, critical_report: None, effectiveness_probe: None, funnel: None, solution_status: None, diversity: None, piecewise: None, archive_probe: None, migration: None, topology: None, ranking_probe: None, acceptance: None, boundary: None, population_config: None, constraint_priority: None, constraint_ranking: None, pareto_archive: None, reference_points: None, dispersion: None, dominance: None, evidence_remap: None, tier_selection: None, activation_set: None, container_surface: None, identity_wrap: None, domain_surface: None, expression_bounds: None, assignment_eval: None, constraint_canon: None, snapshot_gate: None, cp_model_build: None, no_good_oracle: None, subproblem_result: None, cp_support_merge: None, capability_matrix: None, analysis_options: None, model_view: None, polynomial_probe: None, matrix_rows: None, sparse_triad: None, master_assignment: None, stable_value: None, quadratic_elastic: None, dump_batch: None, lazy_flatten_init: None, candidate_shortlist: None, writer_registry: None, native_write_scope: None, dump_options: None, assignment_composition: None, cut_validity: None, source_identity: None, binding_assignment: None, auxiliary_domain: None, activity_report_check: None, evidence_predicates: None, activity_bands: None, encoding_label: None, expression_key: None, canonical_monomial: None, backend_features: None, deferred_materialize: None, quadratic_monomial: None, polarity_encoding: None, tier_classification: None, assumption_binding: None, effectiveness_aggregate: None, bounded_no_good: None, polynomial_lift: None, scalar_function: None, field_resolution: None, boolean_fold: None, master_cut: None, flatten_identity: None, critical_assembly: None, variable_kind_domain: None, sort_by: None, predicate_schema: None, search_budget: None, conditional_semantics: None, big_m_policy: None, number_text: None, combine_status: None, elastic_model: None, native_write: None, input_domain: None, function_usage: None, repository_call: None, unsupported_predicate: None, boolean_scope: None, update_assignment: None, flatten_cache: None, cache_key_address: None, cache_key_identity: None, model_configuration: None, predicate_field_filter: None, predicate_field_resolution: None, analysis_group_aggregate: None, master_cut_encoding: None, q_table: None, heuristic_schedule: None, symbol_core_relations: None, symbol_linearization: None, symbol_condition: None, benders_cut_request: None, benders_metrics: None, cp_checkpoint_request: None, lbb_resume_identity: None, linear_mapping_request: None, checkpoint_request: None, reward_policy: None, hyper_heuristic_config: None, criticality_request: None, multi_target_report: None, relaxation_policy: None, target_spec: None, target_report: None, perturbation_policy: None, fixed_integer_report: None, fixed_integer_config: None, relaxation_request: None, critical_analysis: None, effectiveness_ranking: None, shortlist_limit: None, candidate_funnel: None, conflict_explanation: None, remote_artifact: None, logic_based_benders_plan: None, master_solution: None, benders_quadratic_cuts: None, pipeline_steps: None, repository_query: None, assignments: None, sort_plan: None, predicate: None, benders_cuts: None, relational_query_plan: None, linear_lifecycle: None,  indexed_registration: None, column_batch: None, column_scope: None, heart_beat: None, response_envelope: None, quadratic_orchestration: None, vrp_problem: None, network_resilience_problem: None, allocation_problem: None, network_structure_problem: None, network_design_problem: None, network_generalized_flow_problem: None, network_flow_problem: None, sweep_cases: None,  incumbent_values: None, solution_pool_options: None, gantt_activity_options: None, iis_options: None, bpp3d_options: None, protocol_version: 1, solver: "bpp2d".into(), model: source.clone(), model_kind: Some(ModelKind::Bpp2d), time_limit_ms: Some(10_000),
            tuning_options: None,
            cp_options: None, serial_options: None, parallel_options: None, fixed_recourse_options: None, column_generation_options: None, bpp1d_options: None, csp1d_options: None,
            bpp_loading_options: None, csp2d_options: None, bpp2d_options: Some(options) };
        let submitted = engine.submit(request).unwrap();
        if variant == 3 { engine.cancel(&submitted.id).unwrap(); }
        let deadline = Instant::now() + Duration::from_secs(15);
        let view = loop { let view = engine.get(&submitted.id).unwrap(); if view.status != JobStatus::Running { break view; }
            assert!(Instant::now() < deadline); std::thread::sleep(Duration::from_millis(1)); };
        assert_eq!(view.status, JobStatus::Completed, "{:?}", view.error);
        let report = decode_bpp2d_execution_report(view.report_json.as_ref().unwrap(), &source).unwrap();
        assert!(!report.summary.candidate_complete);
        if variant == 0 { assert!(report.production.is_some()); assert!(view.progress_json.is_some()); }
        if variant == 1 { assert_eq!(report.summary.stop, Bpp2dExecutionStop::ReportLimit); assert!(report.production.is_none()); }
        if variant == 2 { assert!(report.summary.integer.is_none()); assert!(report.production.is_none()); }
        let retained = engine.cancel(&submitted.id).unwrap(); assert_eq!(retained.report_json, view.report_json);
        engine.release(&submitted.id).unwrap(); assert_eq!(engine.get(&submitted.id).unwrap_err().code, "NotFound");
    }
}

#[cfg(feature = "scip")]
#[test]
fn real_scip_full_execution_codec_revalidates_source_pricing_and_production() {
    use ospf_rust_core_plugin_scip::{SCIPConfig, SCIPSolver};
    contract(Arc::new(SCIPSolver::with_config(SCIPConfig::default().with_output(false).with_lp_subproblem_defaults())), NativeBackend::Scip);
    engine_contract(NativeBackend::Scip);
}

#[cfg(any(feature = "gurobi10", feature = "gurobi11", feature = "gurobi12"))]
#[test]
fn real_gurobi_full_execution_codec_revalidates_source_pricing_and_production() {
    use ospf_rust_core_plugin_gurobi::{GurobiConfig, GurobiSolver};
    contract(Arc::new(GurobiSolver::with_config(GurobiConfig::default().with_output(false))), NativeBackend::Gurobi);
    engine_contract(NativeBackend::Gurobi);
}
