//! 真实后端经 native 正式分派产出 BPP3D 单轮执行报告的验收。 / Acceptance of BPP3D one-round execution reports produced through the formal native dispatch with real backends.

#![cfg(any(feature = "scip", feature = "gurobi10", feature = "gurobi11", feature = "gurobi12"))]

use std::time::{Duration, Instant};
use ospf_rust_bridge::*;
use ospf_rust_native::NativeEngine;

fn source() -> ModelArtifact {
    encode_bpp3d_problem(serde_json::from_str(r#"{
        "items":[{"id":"item","name":"Item","width":{"value":1,"unit":"m"},"height":{"value":1,"unit":"m"},"depth":{"value":1,"unit":"m"},"weight":{"value":500,"unit":"g"},"amount":"2","orientations":["upright"]}],
        "bins":[{"id":"one","typeCode":"type","width":{"value":1,"unit":"m"},"height":{"value":1,"unit":"m"},"depth":{"value":1,"unit":"m"},"capacity":{"value":1,"unit":"kg"},"isMain":true},
                {"id":"two","typeCode":"type","width":{"value":1,"unit":"m"},"height":{"value":1,"unit":"m"},"depth":{"value":1,"unit":"m"},"capacity":{"value":1,"unit":"kg"},"isMain":true}],
        "initialLayers":[{"id":"layer","binId":"one","depth":{"value":1,"unit":"m"},"placements":[{"itemId":"item","position":{"x":{"value":0,"unit":"m"},"y":{"value":0,"unit":"m"},"z":{"value":0,"unit":"m"}},"orientation":"upright"}]}]
    }"#).unwrap()).unwrap()
}

fn request(backend: NativeBackend) -> SolveRequest { SolveRequest { schedule_log_probe: None, operator_capability_probe: None, hypervolume_probe: None, acceptance_recovery_probe: None, training_split_probe: None, non_stationary_probe: None, selection_policy_probe: None, solve_value_probe: None, native_lowering_probe: None, native_terminal_proof_probe: None, predicate_scope_probe: None, persistence_schema_probe: None, persistence_sort_probe: None, effectiveness_ranking_probe: None, funnel_ranking: None, sensitivity_probe: None, correction_probe: None, blocking_set: None, fingerprint_probe: None, diagnostics_probe: None, gap_probe: None, proof_probe: None, evidence_probe: None, band_probe: None, group_summary: None, critical_report: None, effectiveness_probe: None, funnel: None, solution_status: None, diversity: None, piecewise: None, archive_probe: None, migration: None, topology: None, ranking_probe: None, acceptance: None, boundary: None, population_config: None, constraint_priority: None, constraint_ranking: None, pareto_archive: None, reference_points: None, dispersion: None, dominance: None, evidence_remap: None, tier_selection: None, activation_set: None, container_surface: None, identity_wrap: None, domain_surface: None, expression_bounds: None, assignment_eval: None, constraint_canon: None, snapshot_gate: None, cp_model_build: None, no_good_oracle: None, subproblem_result: None, cp_support_merge: None, capability_matrix: None, analysis_options: None, model_view: None, polynomial_probe: None, matrix_rows: None, sparse_triad: None, master_assignment: None, stable_value: None, quadratic_elastic: None, dump_batch: None, lazy_flatten_init: None, candidate_shortlist: None, writer_registry: None, native_write_scope: None, dump_options: None, assignment_composition: None, cut_validity: None, source_identity: None, binding_assignment: None, auxiliary_domain: None, activity_report_check: None, evidence_predicates: None, activity_bands: None, encoding_label: None, expression_key: None, canonical_monomial: None, backend_features: None, deferred_materialize: None, quadratic_monomial: None, polarity_encoding: None, tier_classification: None, assumption_binding: None, effectiveness_aggregate: None, bounded_no_good: None, polynomial_lift: None, scalar_function: None, field_resolution: None, boolean_fold: None, master_cut: None, flatten_identity: None, critical_assembly: None, variable_kind_domain: None, sort_by: None, predicate_schema: None, search_budget: None, conditional_semantics: None, big_m_policy: None, number_text: None, combine_status: None, elastic_model: None, native_write: None, input_domain: None, function_usage: None, repository_call: None, unsupported_predicate: None, boolean_scope: None, update_assignment: None, flatten_cache: None, cache_key_address: None, cache_key_identity: None, model_configuration: None, predicate_field_filter: None, predicate_field_resolution: None, analysis_group_aggregate: None, master_cut_encoding: None, q_table: None, heuristic_schedule: None, symbol_core_relations: None, symbol_linearization: None, symbol_condition: None, benders_cut_request: None, benders_metrics: None, cp_checkpoint_request: None, lbb_resume_identity: None, linear_mapping_request: None, checkpoint_request: None, reward_policy: None, hyper_heuristic_config: None, criticality_request: None, multi_target_report: None, relaxation_policy: None, target_spec: None, target_report: None, perturbation_policy: None, fixed_integer_report: None, fixed_integer_config: None, relaxation_request: None, critical_analysis: None, effectiveness_ranking: None, shortlist_limit: None, candidate_funnel: None, conflict_explanation: None, remote_artifact: None, logic_based_benders_plan: None, master_solution: None, benders_quadratic_cuts: None, pipeline_steps: None, repository_query: None, assignments: None, sort_plan: None, predicate: None, benders_cuts: None, relational_query_plan: None, linear_lifecycle: None,  indexed_registration: None, column_batch: None, column_scope: None, heart_beat: None, response_envelope: None, quadratic_orchestration: None, vrp_problem: None, network_resilience_problem: None, allocation_problem: None, network_structure_problem: None, network_design_problem: None, network_generalized_flow_problem: None, network_flow_problem: None, sweep_cases: None, solution_pool_options: None, incumbent_values: None, gantt_activity_options: None, iis_options: None, bpp3d_options: Some(Bpp3dOptions { backend, generators: None, report_limit: None, max_placements: None }),
        tuning_options: None,
        protocol_version: 1, solver: "bpp3d".into(), model: source(), model_kind: Some(ModelKind::Bpp3d), time_limit_ms: Some(30_000),
        cp_options: None, serial_options: None, parallel_options: None, fixed_recourse_options: None, column_generation_options: None,
        bpp1d_options: None, csp1d_options: None, bpp_loading_options: None, csp2d_options: None, bpp2d_options: None }
}

fn contract(backend: NativeBackend) {
    let engine = NativeEngine::new(1);
    let mut fixtures = Vec::new();
    for variant in 0..4 {
        let mut request = request(backend);
        if variant == 2 { request.bpp3d_options.as_mut().unwrap().report_limit = Some(1); }
        if variant == 3 { request.bpp3d_options.as_mut().unwrap().max_placements = Some(1); }
        let source = request.model.clone();
        let submitted = engine.submit(request).unwrap();
        if variant == 1 { engine.cancel(&submitted.id).unwrap(); }
        let deadline = Instant::now() + Duration::from_secs(60);
        let view = loop {
            let view = engine.get(&submitted.id).unwrap();
            if view.status != JobStatus::Running { break view; }
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(2));
        };
        assert_eq!(view.status, JobStatus::Completed, "variant {variant}: {:?}", view.error);
        let report = decode_bpp3d_execution_report(view.report_json.as_ref().unwrap(), &source).unwrap_or_else(|error| panic!("variant {variant}: {error}"));
        assert!(!report.summary.candidate_complete);
        assert_eq!(report.artifact.source_fingerprint, source.fingerprint);
        match variant {
            0 => {
                assert!(report.summary.completed && report.summary.has_packing, "variant {variant}: {:?}", report.summary.failure);
                assert_eq!(report.summary.stage_count, 3);
                assert!(report.summary.generated_count >= 1, "default chain must generate layers from the source initial layer");
                assert_eq!(report.summary.column_count, 1 + report.summary.generated_count);
                assert_eq!(report.stages[0].iteration, 0); assert_eq!(report.stages[1].iteration, 1);
                assert!(report.stages[1].column_count > report.stages[0].column_count);
                assert!(report.stages[1].model_fingerprint.is_some() && report.stages[1].restricted_optimal);
                assert_ne!(report.artifact.stages[0].pool_json, report.artifact.stages[1].pool_json);
                assert_eq!(report.packing.as_ref().unwrap().bins.len(), 2);
                assert!(view.progress_json.is_some());
            }
            1 => {
                // 取消与完成竞争：两种结局都必须来自真实报告，不得伪造。 / Cancel-versus-finish races must both come from real reports, never fabrication.
                assert!(report.summary.stage_count >= 1);
                assert_eq!(report.summary.completed, report.summary.failure.is_none());
            }
            2 => {
                assert!(!report.summary.completed && report.summary.stage_count == 2);
                assert_eq!(report.stages[1].model_fingerprint, None);
                assert!(report.summary.failure.as_deref().unwrap_or_default().contains("report limit"), "{:?}", report.summary.failure);
                assert!(report.packing.is_none() && report.selection.is_none());
            }
            _ => {
                // 展开预算不足时保留真实 final 分配与原报告，只撤回装箱结论。 / An insufficient expansion budget keeps the actual final selection and original report, retracting only the packing conclusion.
                assert!(!report.summary.completed && report.summary.stage_count == 3);
                assert!(report.stages[2].has_incumbent && report.selection.is_some());
                assert!(report.packing.is_none());
                assert!(report.summary.failure.as_deref().unwrap_or_default().contains("FinalMilp"), "{:?}", report.summary.failure);
            }
        }
        fixtures.push(serde_json::json!({ "source": source, "reportJson": view.report_json.clone(),
            "packingJson": report.packing.as_ref().map(|value| serde_json::to_string(value).unwrap()) }));
        let retained = engine.cancel(&submitted.id).unwrap();
        assert_eq!(retained.report_json, view.report_json);
        engine.release(&submitted.id).unwrap();
        assert_eq!(engine.get(&submitted.id).unwrap_err().code, "NotFound");
    }
    // 显式联调证据只写调用方指定目录，不入库。 / Explicit integration evidence is written only to a caller-specified directory and stays out of Git.
    if let Some(directory) = std::env::var_os("OSPF_BPP3D_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory); std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(directory.join(format!("bpp3d-execution-{}.json", backend.id())), serde_json::to_string(&fixtures).unwrap()).unwrap();
    }
}

#[cfg(feature = "scip")]
#[test]
fn real_scip_dispatches_bpp3d_one_round_execution_reports() { contract(NativeBackend::Scip); }

#[cfg(any(feature = "gurobi10", feature = "gurobi11", feature = "gurobi12"))]
#[test]
fn real_gurobi_dispatches_bpp3d_one_round_execution_reports() { contract(NativeBackend::Gurobi); }
