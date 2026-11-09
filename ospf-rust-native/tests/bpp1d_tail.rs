//! 真实后端的 BPP1D 尾箱优化验收：只重排尾部、目标不变差，并撤回全局最优性主张。
//! Real-backend BPP1D tail-optimization acceptance: only the tail is re-arranged, the objective never gets worse,
//! and the global optimality claim is withdrawn.

#![cfg(any(feature = "scip", feature = "gurobi10", feature = "gurobi11", feature = "gurobi12"))]

use std::time::{Duration, Instant};
use ospf_rust_bridge::*;

fn problem() -> Bpp1dProblem {
    serde_json::from_str(r#"{"items":[
        {"id":"a","length":{"value":60,"unit":"cm"},"weight":{"value":0.1,"unit":"kg"},"amount":"4","materials":{}},
        {"id":"b","length":{"value":30,"unit":"cm"},"weight":{"value":0.1,"unit":"kg"},"amount":"4","materials":{}}],
        "binTypes":[{"id":"bin","length":{"value":100,"unit":"cm"},"maxWeight":{"value":10,"unit":"kg"},"cost":1,"inventory":null}],
        "spacing":{"value":0,"unit":"cm"},"lengthUnit":"cm","weightUnit":"kg","objective":"binCount"}"#).unwrap()
}

fn options(backend: NativeBackend, tail_window: Option<u32>) -> Bpp1dOptions {
    Bpp1dOptions { backend, algorithm: Bpp1dAlgorithm::Milp, generation: None, iteration_limit: None,
        columns_per_iteration: None, expansion_limit: None, tolerance: None, tail_window }
}

fn run(backend: NativeBackend, tail_window: Option<u32>) -> Bpp1dReport {
    let engine = ospf_rust_native::NativeEngine::new(1);
    let source = encode_bpp1d_problem(problem()).unwrap();
    let request = SolveRequest { schedule_log_probe: None, operator_capability_probe: None, hypervolume_probe: None, acceptance_recovery_probe: None, training_split_probe: None, non_stationary_probe: None, selection_policy_probe: None, solve_value_probe: None, native_lowering_probe: None, native_terminal_proof_probe: None, predicate_scope_probe: None, persistence_schema_probe: None, persistence_sort_probe: None, effectiveness_ranking_probe: None, funnel_ranking: None, sensitivity_probe: None, correction_probe: None, blocking_set: None, fingerprint_probe: None, diagnostics_probe: None, gap_probe: None, proof_probe: None, evidence_probe: None, band_probe: None, group_summary: None, critical_report: None, effectiveness_probe: None, funnel: None, solution_status: None, diversity: None, piecewise: None, archive_probe: None, migration: None, topology: None, ranking_probe: None, boundary: None, population_config: None, constraint_priority: None, constraint_ranking: None, pareto_archive: None, reference_points: None, dispersion: None, dominance: None, evidence_remap: None, tier_selection: None, activation_set: None, container_surface: None, identity_wrap: None, domain_surface: None, expression_bounds: None, assignment_eval: None, constraint_canon: None, snapshot_gate: None, cp_model_build: None, no_good_oracle: None, subproblem_result: None, cp_support_merge: None, capability_matrix: None, analysis_options: None, model_view: None, polynomial_probe: None, matrix_rows: None, sparse_triad: None, master_assignment: None, stable_value: None, quadratic_elastic: None, dump_batch: None, lazy_flatten_init: None, candidate_shortlist: None, writer_registry: None, native_write_scope: None, dump_options: None, assignment_composition: None, cut_validity: None, source_identity: None, binding_assignment: None, auxiliary_domain: None, activity_report_check: None, evidence_predicates: None, activity_bands: None, encoding_label: None, expression_key: None, canonical_monomial: None, backend_features: None, deferred_materialize: None, quadratic_monomial: None, polarity_encoding: None, tier_classification: None, assumption_binding: None, effectiveness_aggregate: None, bounded_no_good: None, polynomial_lift: None, scalar_function: None, field_resolution: None, boolean_fold: None, master_cut: None, flatten_identity: None, critical_assembly: None, variable_kind_domain: None, sort_by: None, predicate_schema: None, search_budget: None, conditional_semantics: None, big_m_policy: None, number_text: None, combine_status: None, elastic_model: None, native_write: None, input_domain: None, function_usage: None, repository_call: None, unsupported_predicate: None, boolean_scope: None, update_assignment: None, flatten_cache: None, cache_key_address: None, cache_key_identity: None, model_configuration: None, predicate_field_filter: None, predicate_field_resolution: None, analysis_group_aggregate: None, master_cut_encoding: None, q_table: None, heuristic_schedule: None, symbol_core_relations: None, symbol_linearization: None, symbol_condition: None, benders_cut_request: None, benders_metrics: None, cp_checkpoint_request: None, lbb_resume_identity: None, linear_mapping_request: None, checkpoint_request: None, reward_policy: None, hyper_heuristic_config: None, criticality_request: None, multi_target_report: None, relaxation_policy: None, target_spec: None, target_report: None, perturbation_policy: None, fixed_integer_report: None, fixed_integer_config: None, relaxation_request: None, critical_analysis: None, effectiveness_ranking: None, shortlist_limit: None, candidate_funnel: None, conflict_explanation: None, remote_artifact: None, logic_based_benders_plan: None, master_solution: None, benders_quadratic_cuts: None, pipeline_steps: None, repository_query: None, assignments: None, sort_plan: None, predicate: None, benders_cuts: None, relational_query_plan: None, linear_lifecycle: None,  indexed_registration: None, column_batch: None, column_scope: None, heart_beat: None, response_envelope: None, quadratic_orchestration: None, vrp_problem: None, network_resilience_problem: None, allocation_problem: None, network_structure_problem: None, network_design_problem: None, network_generalized_flow_problem: None, network_flow_problem: None, sweep_cases: None,  incumbent_values: None, solution_pool_options: None, gantt_activity_options: None, iis_options: None, bpp3d_options: None, protocol_version: 1, solver: "bpp1d".into(), model: source.clone(),
        model_kind: Some(ModelKind::Bpp1d), time_limit_ms: Some(30_000), tuning_options: None,
        cp_options: None, serial_options: None, parallel_options: None, fixed_recourse_options: None, column_generation_options: None,
        bpp1d_options: Some(options(backend, tail_window)), csp1d_options: None, bpp_loading_options: None, csp2d_options: None, bpp2d_options: None };
    let submitted = engine.submit(request).unwrap();
    let deadline = Instant::now() + Duration::from_secs(60);
    let view = loop {
        let view = engine.get(&submitted.id).unwrap();
        if view.status != JobStatus::Running { break view; }
        assert!(Instant::now() < deadline, "tail solve did not finish in time");
        std::thread::sleep(Duration::from_millis(5));
    };
    assert_eq!(view.status, JobStatus::Completed, "{:?}", view.error);
    decode_bpp1d_report(view.report_json.as_ref().unwrap(), &source).unwrap()
}

fn contract(backend: NativeBackend) {
    let baseline = run(backend, None);
    let optimized = run(backend, Some(2));
    let baseline_packing = baseline.packing.as_ref().expect("baseline packing");
    let optimized_packing = optimized.packing.as_ref().expect("optimized packing");
    // 尾箱优化只能在有效排样之间移动，且不得让目标变差。
    // Tail optimization may only move between valid packings and must never make the objective worse.
    assert!((optimized_packing.objective - baseline_packing.objective).abs() < 1e-9,
        "objective changed: {} -> {}", baseline_packing.objective, optimized_packing.objective);
    // 只重排尾部：前 plans.len()-window 个箱必须与首轮解逐字一致。
    // Only the tail is re-arranged: the first plans.len()-window bins must match the first solve exactly.
    let prefix = baseline_packing.plans.len().saturating_sub(2);
    assert!(prefix > 0, "fixture must produce more than one prefix bin");
    for index in 0..prefix {
        assert_eq!(
            serde_json::to_string(&optimized_packing.plans[index]).unwrap(),
            serde_json::to_string(&baseline_packing.plans[index]).unwrap(),
            "tail optimization must preserve prefix bin {index}");
    }
    assert!(optimized.summary.tail_window == Some(2));
    assert_eq!(baseline.summary.tail_window, None);
    // 排样真的被改动时不得再主张全局最优；没改动时保留原主张。
    // A changed packing must not claim global optimality; an unchanged one keeps the original claim.
    if optimized.summary.tail_optimized {
        assert!(!optimized.summary.globally_optimal, "tail-optimized packing must not claim global optimality");
    } else {
        assert_eq!(optimized.summary.globally_optimal, baseline.summary.globally_optimal);
    }
}

#[cfg(feature = "scip")]
#[test]
fn real_scip_tail_optimization_keeps_valid_packing_and_withdraws_optimality() { contract(NativeBackend::Scip); }

#[cfg(any(feature = "gurobi10", feature = "gurobi11", feature = "gurobi12"))]
#[test]
fn real_gurobi_tail_optimization_keeps_valid_packing_and_withdraws_optimality() { contract(NativeBackend::Gurobi); }
