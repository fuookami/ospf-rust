//! 解池入口的原生端契约：只接受自己的选项，且必须真的返回多个解。
//! Native contract for the solution-pool entry: it accepts only its own options and must really return several solutions.

use crate::*;
use ospf_rust_bridge::LinearSolutionPoolOptions;

/// `x + y <= 4`，最大化 `x + y`：最优面有多个整数解，足以让解池给出不止一个向量。
/// `x + y <= 4` maximizing `x + y`: the optimal face has several integer solutions, so a pool can return more than one
/// vector.
fn multi_solution_request(amount: u32) -> SolveRequest { use ospf_rust_framework::solver::remote::{SerializedConstraint, SerializedConstraintCell, SerializedConstraintSign, SerializedVariable, SerializedVariableType};
    let mut input = ospf_rust_framework::solver::remote::SerializedLinearModel::empty("pool-native");
    for (index, name) in [(0usize, "x"), (1, "y")] {
        input.variables.push(SerializedVariable { stable_id: name.into(), index, name: name.into(),
            lower_bound: 0.0, upper_bound: 4.0, variable_type: SerializedVariableType::Integer });
    }
    input.constraints.push(SerializedConstraint { stable_id: "capacity".into(), name: "capacity".into(), rhs: 4.0,
        sign: SerializedConstraintSign::LessEqual,
        cells: vec![SerializedConstraintCell { row_index: 0, col_index: 0, coefficient: 1.0 },
            SerializedConstraintCell { row_index: 0, col_index: 1, coefficient: 1.0 }] });
    let mut model = ospf_rust_bridge::compile_linear_meta(&input).unwrap();
    // 最大化 x + y。 / Maximize x + y.
    model.set_linear_objective(vec![1.0, 1.0], ospf_rust_core::model::ObjectiveCategory::Maximum);
    let artifact = ospf_rust_bridge::encode_model(&model.try_to_linear_triad_model().unwrap()).unwrap();
    SolveRequest { schedule_log_probe: None, operator_capability_probe: None, hypervolume_probe: None, acceptance_recovery_probe: None, training_split_probe: None, non_stationary_probe: None, selection_policy_probe: None, solve_value_probe: None, native_lowering_probe: None, native_terminal_proof_probe: None, predicate_scope_probe: None, persistence_schema_probe: None, persistence_sort_probe: None, effectiveness_ranking_probe: None, funnel_ranking: None, sensitivity_probe: None, correction_probe: None, blocking_set: None, fingerprint_probe: None, diagnostics_probe: None, gap_probe: None, proof_probe: None, evidence_probe: None, band_probe: None, group_summary: None, critical_report: None, effectiveness_probe: None, funnel: None, solution_status: None, diversity: None, piecewise: None, archive_probe: None, migration: None, topology: None, ranking_probe: None, acceptance: None, boundary: None, population_config: None, constraint_priority: None, constraint_ranking: None, pareto_archive: None, reference_points: None, dispersion: None, dominance: None, evidence_remap: None, tier_selection: None, activation_set: None, container_surface: None, identity_wrap: None, domain_surface: None, expression_bounds: None, assignment_eval: None, constraint_canon: None, snapshot_gate: None, cp_model_build: None, no_good_oracle: None, subproblem_result: None, cp_support_merge: None, capability_matrix: None, analysis_options: None, model_view: None, polynomial_probe: None, matrix_rows: None, sparse_triad: None, master_assignment: None, stable_value: None, quadratic_elastic: None, dump_batch: None, lazy_flatten_init: None, candidate_shortlist: None, writer_registry: None, native_write_scope: None, dump_options: None, assignment_composition: None, cut_validity: None, source_identity: None, binding_assignment: None, auxiliary_domain: None, activity_report_check: None, evidence_predicates: None, activity_bands: None, encoding_label: None, expression_key: None, canonical_monomial: None, backend_features: None, deferred_materialize: None, quadratic_monomial: None, polarity_encoding: None, tier_classification: None, assumption_binding: None, effectiveness_aggregate: None, bounded_no_good: None, polynomial_lift: None, scalar_function: None, field_resolution: None, boolean_fold: None, master_cut: None, flatten_identity: None, critical_assembly: None, variable_kind_domain: None, sort_by: None, predicate_schema: None, search_budget: None, conditional_semantics: None, big_m_policy: None, number_text: None, combine_status: None, elastic_model: None, native_write: None, input_domain: None, function_usage: None, repository_call: None, unsupported_predicate: None, boolean_scope: None, update_assignment: None, flatten_cache: None, cache_key_address: None, cache_key_identity: None, model_configuration: None, predicate_field_filter: None, predicate_field_resolution: None, analysis_group_aggregate: None, master_cut_encoding: None, q_table: None, heuristic_schedule: None, symbol_core_relations: None, symbol_linearization: None, symbol_condition: None, benders_cut_request: None, benders_metrics: None, cp_checkpoint_request: None, lbb_resume_identity: None, linear_mapping_request: None, checkpoint_request: None, reward_policy: None, hyper_heuristic_config: None, criticality_request: None, multi_target_report: None, relaxation_policy: None, target_spec: None, target_report: None, perturbation_policy: None, fixed_integer_report: None, fixed_integer_config: None, relaxation_request: None, critical_analysis: None, effectiveness_ranking: None, shortlist_limit: None, candidate_funnel: None, conflict_explanation: None, remote_artifact: None, logic_based_benders_plan: None, master_solution: None, benders_quadratic_cuts: None, pipeline_steps: None, repository_query: None, assignments: None, sort_plan: None, predicate: None, benders_cuts: None, relational_query_plan: None, linear_lifecycle: None,  indexed_registration: None, column_batch: None, column_scope: None, heart_beat: None, response_envelope: None, quadratic_orchestration: None, vrp_problem: None, network_resilience_problem: None, allocation_problem: None, network_structure_problem: None, network_design_problem: None, network_generalized_flow_problem: None, network_flow_problem: None, sweep_cases: None,  incumbent_values: None, solution_pool_options: Some(LinearSolutionPoolOptions { amount }), gantt_activity_options: None,
        bpp3d_options: None, protocol_version: 1, solver: "scip".into(), model: artifact, tuning_options: None,
        model_kind: Some(ModelKind::LinearSolutionPool), time_limit_ms: Some(10_000), cp_options: None, serial_options: None,
        parallel_options: None, fixed_recourse_options: None, column_generation_options: None, bpp1d_options: None,
        bpp2d_options: None, bpp_loading_options: None, csp2d_options: None, csp1d_options: None, iis_options: None }
}

#[test]
fn solution_pool_entry_rejects_option_mixing_and_out_of_range_amounts() {
    let engine = NativeEngine::default();
    let request = multi_solution_request(4);
    // 混入其它策略选项必须拒绝。 / Mixing another strategy option must fail.
    let mut mixed = request.clone();
    mixed.csp1d_options = Some(ospf_rust_bridge::Csp1dOptions { backend: ospf_rust_bridge::NativeBackend::Scip,
        algorithm: ospf_rust_bridge::Csp1dAlgorithm::ColumnGeneration, generation: None, iteration_limit: None,
        columns_per_iteration: None, pool_limit: None });
    assert_eq!(engine.submit(mixed).unwrap_err().code, "InvalidInput");
    // 与 IIS 选项混用同样拒绝。 / Mixing with IIS options must fail too.
    let mut both = request.clone();
    both.iis_options = Some(ospf_rust_bridge::LinearIisOptions { algorithm: None, include_bounds: None, max_iterations: None });
    assert_eq!(engine.submit(both).unwrap_err().code, "InvalidInput");
    // 缺 modelKind 必须拒绝。 / A missing modelKind must fail.
    assert_eq!(engine.submit(SolveRequest { schedule_log_probe: None, operator_capability_probe: None, hypervolume_probe: None, acceptance_recovery_probe: None, training_split_probe: None, non_stationary_probe: None, selection_policy_probe: None, solve_value_probe: None, native_lowering_probe: None, native_terminal_proof_probe: None, predicate_scope_probe: None, persistence_schema_probe: None, persistence_sort_probe: None, effectiveness_ranking_probe: None, funnel_ranking: None, sensitivity_probe: None, correction_probe: None, blocking_set: None, fingerprint_probe: None, diagnostics_probe: None, gap_probe: None, proof_probe: None, evidence_probe: None, band_probe: None, group_summary: None, critical_report: None, effectiveness_probe: None, funnel: None, solution_status: None, diversity: None, piecewise: None, archive_probe: None, migration: None, topology: None, ranking_probe: None, acceptance: None, boundary: None, population_config: None, constraint_priority: None, constraint_ranking: None, pareto_archive: None, reference_points: None, dispersion: None, dominance: None, evidence_remap: None, tier_selection: None, activation_set: None, container_surface: None, identity_wrap: None, domain_surface: None, expression_bounds: None, assignment_eval: None, constraint_canon: None, snapshot_gate: None, cp_model_build: None, no_good_oracle: None, subproblem_result: None, cp_support_merge: None, capability_matrix: None, analysis_options: None, model_view: None, polynomial_probe: None, matrix_rows: None, sparse_triad: None, master_assignment: None, stable_value: None, quadratic_elastic: None, dump_batch: None, lazy_flatten_init: None, candidate_shortlist: None, writer_registry: None, native_write_scope: None, dump_options: None, assignment_composition: None, cut_validity: None, source_identity: None, binding_assignment: None, auxiliary_domain: None, activity_report_check: None, evidence_predicates: None, activity_bands: None, encoding_label: None, expression_key: None, canonical_monomial: None, backend_features: None, deferred_materialize: None, quadratic_monomial: None, polarity_encoding: None, tier_classification: None, assumption_binding: None, effectiveness_aggregate: None, bounded_no_good: None, polynomial_lift: None, scalar_function: None, field_resolution: None, boolean_fold: None, master_cut: None, flatten_identity: None, critical_assembly: None, variable_kind_domain: None, sort_by: None, predicate_schema: None, search_budget: None, conditional_semantics: None, big_m_policy: None, number_text: None, combine_status: None, elastic_model: None, native_write: None, input_domain: None, function_usage: None, repository_call: None, unsupported_predicate: None, boolean_scope: None, update_assignment: None, flatten_cache: None, cache_key_address: None, cache_key_identity: None, model_configuration: None, predicate_field_filter: None, predicate_field_resolution: None, analysis_group_aggregate: None, master_cut_encoding: None, q_table: None, heuristic_schedule: None, symbol_core_relations: None, symbol_linearization: None, symbol_condition: None, benders_cut_request: None, benders_metrics: None, cp_checkpoint_request: None, lbb_resume_identity: None, linear_mapping_request: None, checkpoint_request: None, reward_policy: None, hyper_heuristic_config: None, criticality_request: None, multi_target_report: None, relaxation_policy: None, target_spec: None, target_report: None, perturbation_policy: None, fixed_integer_report: None, fixed_integer_config: None, relaxation_request: None, critical_analysis: None, effectiveness_ranking: None, shortlist_limit: None, candidate_funnel: None, conflict_explanation: None, remote_artifact: None, logic_based_benders_plan: None, master_solution: None, benders_quadratic_cuts: None, pipeline_steps: None, repository_query: None, assignments: None, sort_plan: None, predicate: None, benders_cuts: None, relational_query_plan: None, linear_lifecycle: None,  indexed_registration: None, column_batch: None, column_scope: None, heart_beat: None, response_envelope: None, quadratic_orchestration: None, vrp_problem: None, network_resilience_problem: None, allocation_problem: None, network_structure_problem: None, network_design_problem: None, network_generalized_flow_problem: None, network_flow_problem: None, sweep_cases: None,  model_kind: None, ..request.clone() }).unwrap_err().code, "InvalidInput");
    // 数量 0/1/257 都必须在入口拒绝；原 core 把 ≤1 视为未请求，不能悄悄降级成单解。
    // Amounts 0/1/257 must fail at the entry; the original core treats ≤1 as "no pool", so it must never silently
    // degrade into a single solution.
    for amount in [0, 1, LinearSolutionPoolOptions::MAX_AMOUNT + 1] {
        let mut invalid = request.clone();
        invalid.solution_pool_options = Some(LinearSolutionPoolOptions { amount });
        assert_eq!(engine.submit(invalid).unwrap_err().code, "InvalidInput", "amount {amount}");
    }
}

#[cfg(feature = "scip")]
#[test]
fn solution_pool_job_returns_several_complete_vectors_through_the_shared_lifecycle() {
    let engine = NativeEngine::default();
    let view = engine.submit(multi_solution_request(4)).unwrap();
    let finished = std::iter::repeat_with(|| { std::thread::sleep(std::time::Duration::from_millis(20)); engine.get(&view.id).unwrap() })
        .take(1500).find(|job| job.status != JobStatus::Running).expect("solution pool job never reached a terminal state");
    assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
    let report: ospf_rust_bridge::LinearSolutionPoolReport = serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
    assert_eq!(report.model_fingerprint, finished.fingerprint);
    assert_eq!(report.variable_ids.len(), 2);
    assert!(report.variable_ids.iter().any(|id| id.contains('x')) && report.variable_ids.iter().any(|id| id.contains('y')));
    // 解池按原 core 语义可以包含次优解，所以只断言**可行**：长度完整、有限、落在边界内且满足 `x + y <= 4`；
    // 同时必须至少有一个成员达到最优面 4，否则这个池毫无意义。
    // A pool may legitimately contain suboptimal members, so assert **feasibility** only: complete length, finite,
    // within bounds and satisfying `x + y <= 4`; at least one member must reach the optimal face of 4, otherwise the
    // pool would be pointless.
    assert!(report.solutions.len() >= 2, "expected several solutions, got {}", report.solutions.len());
    for solution in &report.solutions {
        assert_eq!(solution.len(), 2);
        assert!(solution.iter().all(|value| value.is_finite()));
        assert!(solution.iter().all(|value| (-1e-9..=4.0 + 1e-9).contains(value)), "out-of-bounds pool member: {solution:?}");
        let total: f64 = solution.iter().sum();
        assert!(total <= 4.0 + 1e-6, "infeasible pool member: {solution:?}");
    }
    let best = report.solutions.iter().map(|solution| solution.iter().sum::<f64>())
        .fold(f64::NEG_INFINITY, f64::max);
    assert!((best - 4.0).abs() < 1e-6, "pool never reaches the optimal face, best {best}");
    assert_eq!(report.best_objective, Some(4.0));
    engine.release(&view.id).unwrap();
}
