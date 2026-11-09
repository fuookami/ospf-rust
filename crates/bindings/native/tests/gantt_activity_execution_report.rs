//! 真实后端经 native 分派产出 Gantt 活动执行报告的验收。 / Acceptance of Gantt activity execution reports produced through the formal native dispatch with real backends.

#![cfg(any(feature = "scip", feature = "gurobi10", feature = "gurobi11", feature = "gurobi12"))]

use std::time::{Duration, Instant};
use ospf_rust_bridge::*;
use ospf_rust_native::NativeEngine;

fn source() -> ModelArtifact {
    encode_gantt_activity_problem(serde_json::from_str(r#"{
        "horizon": {"start": "2026-01-01T00:00:00Z", "end": "2026-01-02T00:00:00Z"},
        "costUnit": "kg",
        "resources": [{"id": "machine", "capacity": {"value": 1.0, "unit": "kg"}, "unavailableTimes": [], "capacities": []}],
        "activities": [
            {"id": "job", "modes": [{"id": "run", "durationSeconds": 3600, "starts": ["2026-01-01T01:00:00Z", "2026-01-01T02:00:00Z"],
                "resources": [{"resource": "machine", "quantity": {"value": 1.0, "unit": "kg"}, "fromSeconds": 0}], "cost": {"value": 2.0, "unit": "kg"}}]},
            {"id": "job-2", "modes": [{"id": "run", "durationSeconds": 3600, "starts": ["2026-01-01T01:00:00Z", "2026-01-01T02:00:00Z"],
                "resources": [{"resource": "machine", "quantity": {"value": 1.0, "unit": "kg"}, "fromSeconds": 0}], "cost": {"value": 2.0, "unit": "kg"}}]}
        ],
        "dependencies": []
    }"#).unwrap()).unwrap()
}

fn request(backend: NativeBackend) -> SolveRequest { SolveRequest { schedule_log_probe: None, operator_capability_probe: None, hypervolume_probe: None, acceptance_recovery_probe: None, training_split_probe: None, non_stationary_probe: None, selection_policy_probe: None, solve_value_probe: None, native_lowering_probe: None, native_terminal_proof_probe: None, predicate_scope_probe: None, persistence_schema_probe: None, persistence_sort_probe: None, effectiveness_ranking_probe: None, funnel_ranking: None, sensitivity_probe: None, correction_probe: None, blocking_set: None, fingerprint_probe: None, diagnostics_probe: None, gap_probe: None, proof_probe: None, evidence_probe: None, band_probe: None, group_summary: None, critical_report: None, effectiveness_probe: None, funnel: None, solution_status: None, diversity: None, piecewise: None, archive_probe: None, migration: None, topology: None, ranking_probe: None, acceptance: None, boundary: None, population_config: None, constraint_priority: None, constraint_ranking: None, pareto_archive: None, reference_points: None, dispersion: None, dominance: None, evidence_remap: None, tier_selection: None, activation_set: None, container_surface: None, identity_wrap: None, domain_surface: None, expression_bounds: None, assignment_eval: None, constraint_canon: None, snapshot_gate: None, cp_model_build: None, no_good_oracle: None, subproblem_result: None, cp_support_merge: None, capability_matrix: None, analysis_options: None, model_view: None, polynomial_probe: None, matrix_rows: None, sparse_triad: None, master_assignment: None, stable_value: None, quadratic_elastic: None, dump_batch: None, lazy_flatten_init: None, candidate_shortlist: None, writer_registry: None, native_write_scope: None, dump_options: None, assignment_composition: None, cut_validity: None, source_identity: None, binding_assignment: None, auxiliary_domain: None, activity_report_check: None, evidence_predicates: None, activity_bands: None, encoding_label: None, expression_key: None, canonical_monomial: None, backend_features: None, deferred_materialize: None, quadratic_monomial: None, polarity_encoding: None, tier_classification: None, assumption_binding: None, effectiveness_aggregate: None, bounded_no_good: None, polynomial_lift: None, scalar_function: None, field_resolution: None, boolean_fold: None, master_cut: None, flatten_identity: None, critical_assembly: None, variable_kind_domain: None, sort_by: None, predicate_schema: None, search_budget: None, conditional_semantics: None, big_m_policy: None, number_text: None, combine_status: None, elastic_model: None, native_write: None, input_domain: None, function_usage: None, repository_call: None, unsupported_predicate: None, boolean_scope: None, update_assignment: None, flatten_cache: None, cache_key_address: None, cache_key_identity: None, model_configuration: None, predicate_field_filter: None, predicate_field_resolution: None, analysis_group_aggregate: None, master_cut_encoding: None, q_table: None, heuristic_schedule: None, symbol_core_relations: None, symbol_linearization: None, symbol_condition: None, benders_cut_request: None, benders_metrics: None, cp_checkpoint_request: None, lbb_resume_identity: None, linear_mapping_request: None, checkpoint_request: None, reward_policy: None, hyper_heuristic_config: None, criticality_request: None, multi_target_report: None, relaxation_policy: None, target_spec: None, target_report: None, perturbation_policy: None, fixed_integer_report: None, fixed_integer_config: None, relaxation_request: None, critical_analysis: None, effectiveness_ranking: None, shortlist_limit: None, candidate_funnel: None, conflict_explanation: None, remote_artifact: None, logic_based_benders_plan: None, master_solution: None, benders_quadratic_cuts: None, pipeline_steps: None, repository_query: None, assignments: None, sort_plan: None, predicate: None, benders_cuts: None, relational_query_plan: None, linear_lifecycle: None,  indexed_registration: None, column_batch: None, column_scope: None, heart_beat: None, response_envelope: None, quadratic_orchestration: None, vrp_problem: None, network_resilience_problem: None, allocation_problem: None, network_structure_problem: None, network_design_problem: None, network_generalized_flow_problem: None, network_flow_problem: None, sweep_cases: None, solution_pool_options: None, incumbent_values: None, iis_options: None, protocol_version: 1, solver: "gantt-activity".into(), model: source(), model_kind: Some(ModelKind::GanttActivity),
        tuning_options: None,
        time_limit_ms: Some(60_000), cp_options: None, serial_options: None, parallel_options: None, fixed_recourse_options: None,
        column_generation_options: None, bpp1d_options: None, csp1d_options: None, bpp2d_options: None, bpp3d_options: None,
        bpp_loading_options: None, csp2d_options: None, gantt_activity_options: Some(GanttActivityOptions { backend }) }
}

fn contract(backend: NativeBackend) {
    let engine = NativeEngine::new(1);
    let submitted = engine.submit(request(backend)).unwrap();
    let deadline = Instant::now() + Duration::from_secs(120);
    let view = loop {
        let view = engine.get(&submitted.id).unwrap();
        if view.status != JobStatus::Running { break view; }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(2));
    };
    assert_eq!(view.status, JobStatus::Completed, "{:?}", view.error);
    let source = source();
    let report = decode_gantt_activity_execution_report(view.report_json.as_ref().unwrap(), &source).unwrap();
    assert!(!report.problem_status.is_empty() && !report.termination_reason.is_empty());
    assert!(!report.solver_model_only);
    let schedule = report.schedule.as_ref().expect("the feasible fixture source must deliver revalidated placements");
    assert!(report.failure.is_none());
    assert_eq!(schedule.placements.len(), 2);
    assert_eq!(schedule.cost.value, 4.0);
    assert_eq!(schedule.candidate_count, 4);
    // 求解器自己的分配必须能被方案复验独立接受，且费用一致。 / The solver's own placements must be accepted independently by plan replay with the same cost.
    let plan = GanttActivityPlan { selections: schedule.placements.iter()
        .map(|placement| GanttActivitySelection { task: placement.task.clone(), mode: placement.mode.clone(), start: placement.start.clone() }).collect() };
    let replay = replay_gantt_activity_plan(&source, &plan).unwrap();
    assert_eq!(replay.schedule.placements.len(), schedule.placements.len());
    assert_eq!(replay.schedule.cost, schedule.cost);
    assert_eq!(replay.schedule.candidate_count, schedule.candidate_count);
    // 联调证据只写调用方指定目录，不入库。 / Integration evidence is written only to a caller-specified directory and stays out of Git.
    if let Some(directory) = std::env::var_os("OSPF_GANTT_ACTIVITY_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory); std::fs::create_dir_all(&directory).unwrap();
        let fixtures = serde_json::json!([{ "source": source, "reportJson": view.report_json.clone() }]);
        std::fs::write(directory.join(format!("gantt-activity-execution-{}.json", backend.id())), serde_json::to_string(&fixtures).unwrap()).unwrap();
    }
    engine.release(&submitted.id).unwrap();
    assert_eq!(engine.get(&submitted.id).unwrap_err().code, "NotFound");
}

#[cfg(feature = "scip")]
#[test]
fn real_scip_dispatches_gantt_activity_execution_reports() { contract(NativeBackend::Scip); }

#[cfg(any(feature = "gurobi10", feature = "gurobi11", feature = "gurobi12"))]
#[test]
fn real_gurobi_dispatches_gantt_activity_execution_reports() { contract(NativeBackend::Gurobi); }
