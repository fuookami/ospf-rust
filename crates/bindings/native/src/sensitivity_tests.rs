//! 敏感性分析入口的原生端契约：只接受自己的选项，且必须真的从 incumbent 推出局部有效性。
//! Native contract for the sensitivity entry: it accepts only its own options and must really derive local
//! effectiveness from an incumbent.

use crate::*;

/// 小规模整数模型：`x + y <= 4`，最大化 `x + y`；incumbent `(2,2)` 可行且最优。
/// Small integer model: `x + y <= 4` maximizing `x + y`; the incumbent `(2, 2)` is feasible and optimal.
fn sensitivity_request(incumbent: Option<Vec<f64>>) -> SolveRequest { use ospf_rust_framework::solver::remote::{SerializedConstraint, SerializedConstraintCell, SerializedConstraintSign, SerializedVariable, SerializedVariableType};
    let mut input = ospf_rust_framework::solver::remote::SerializedLinearModel::empty("sensitivity-native");
    for (index, name) in [(0usize, "x"), (1, "y")] {
        input.variables.push(SerializedVariable { stable_id: name.into(), index, name: name.into(),
            lower_bound: 0.0, upper_bound: 4.0, variable_type: SerializedVariableType::Integer });
    }
    input.constraints.push(SerializedConstraint { stable_id: "capacity".into(), name: "capacity".into(), rhs: 4.0,
        sign: SerializedConstraintSign::LessEqual,
        cells: vec![SerializedConstraintCell { row_index: 0, col_index: 0, coefficient: 1.0 },
            SerializedConstraintCell { row_index: 0, col_index: 1, coefficient: 1.0 }] });
    let mut model = ospf_rust_bridge::compile_linear_meta(&input).unwrap();
    model.set_linear_objective(vec![1.0, 1.0], ospf_rust_core::model::ObjectiveCategory::Maximum);
    let artifact = ospf_rust_bridge::encode_model(&model.try_to_linear_triad_model().unwrap()).unwrap();
    SolveRequest { schedule_log_probe: None, operator_capability_probe: None, hypervolume_probe: None, acceptance_recovery_probe: None, training_split_probe: None, non_stationary_probe: None, selection_policy_probe: None, solve_value_probe: None, native_lowering_probe: None, native_terminal_proof_probe: None, predicate_scope_probe: None, persistence_schema_probe: None, persistence_sort_probe: None, effectiveness_ranking_probe: None, funnel_ranking: None, sensitivity_probe: None, correction_probe: None, blocking_set: None, fingerprint_probe: None, diagnostics_probe: None, gap_probe: None, proof_probe: None, evidence_probe: None, band_probe: None, group_summary: None, critical_report: None, effectiveness_probe: None, funnel: None, solution_status: None, diversity: None, piecewise: None, archive_probe: None, migration: None, topology: None, ranking_probe: None, acceptance: None, boundary: None, population_config: None, constraint_priority: None, constraint_ranking: None, pareto_archive: None, reference_points: None, dispersion: None, dominance: None, evidence_remap: None, tier_selection: None, activation_set: None, container_surface: None, identity_wrap: None, domain_surface: None, expression_bounds: None, assignment_eval: None, constraint_canon: None, snapshot_gate: None, cp_model_build: None, no_good_oracle: None, subproblem_result: None, cp_support_merge: None, capability_matrix: None, analysis_options: None, model_view: None, polynomial_probe: None, matrix_rows: None, sparse_triad: None, master_assignment: None, stable_value: None, quadratic_elastic: None, dump_batch: None, lazy_flatten_init: None, candidate_shortlist: None, writer_registry: None, native_write_scope: None, dump_options: None, assignment_composition: None, cut_validity: None, source_identity: None, binding_assignment: None, auxiliary_domain: None, activity_report_check: None, evidence_predicates: None, activity_bands: None, encoding_label: None, expression_key: None, canonical_monomial: None, backend_features: None, deferred_materialize: None, quadratic_monomial: None, polarity_encoding: None, tier_classification: None, assumption_binding: None, effectiveness_aggregate: None, bounded_no_good: None, polynomial_lift: None, scalar_function: None, field_resolution: None, boolean_fold: None, master_cut: None, flatten_identity: None, critical_assembly: None, variable_kind_domain: None, sort_by: None, predicate_schema: None, search_budget: None, conditional_semantics: None, big_m_policy: None, number_text: None, combine_status: None, elastic_model: None, native_write: None, input_domain: None, function_usage: None, repository_call: None, unsupported_predicate: None, boolean_scope: None, update_assignment: None, flatten_cache: None, cache_key_address: None, cache_key_identity: None, model_configuration: None, predicate_field_filter: None, predicate_field_resolution: None, analysis_group_aggregate: None, master_cut_encoding: None, q_table: None, heuristic_schedule: None, symbol_core_relations: None, symbol_linearization: None, symbol_condition: None, benders_cut_request: None, benders_metrics: None, cp_checkpoint_request: None, lbb_resume_identity: None, linear_mapping_request: None, checkpoint_request: None, reward_policy: None, hyper_heuristic_config: None, criticality_request: None, multi_target_report: None, relaxation_policy: None, target_spec: None, target_report: None, perturbation_policy: None, fixed_integer_report: None, fixed_integer_config: None, relaxation_request: None, critical_analysis: None, effectiveness_ranking: None, shortlist_limit: None, candidate_funnel: None, conflict_explanation: None, remote_artifact: None, logic_based_benders_plan: None, master_solution: None, benders_quadratic_cuts: None, pipeline_steps: None, repository_query: None, assignments: None, sort_plan: None, predicate: None, benders_cuts: None, relational_query_plan: None, linear_lifecycle: None,  indexed_registration: None, column_batch: None, column_scope: None, heart_beat: None, response_envelope: None, quadratic_orchestration: None, vrp_problem: None, network_resilience_problem: None, allocation_problem: None, network_structure_problem: None, network_design_problem: None, network_generalized_flow_problem: None, network_flow_problem: None, sweep_cases: None,  incumbent_values: incumbent, solution_pool_options: None, gantt_activity_options: None,
        bpp3d_options: None, protocol_version: 1, solver: "scip".into(), model: artifact, tuning_options: None,
        model_kind: Some(ModelKind::LinearSensitivity), time_limit_ms: Some(10_000), cp_options: None, serial_options: None,
        parallel_options: None, fixed_recourse_options: None, column_generation_options: None, bpp1d_options: None,
        bpp2d_options: None, bpp_loading_options: None, csp2d_options: None, csp1d_options: None, iis_options: None }
}

#[test]
fn sensitivity_entry_rejects_option_mixing_and_incomplete_incumbents() {
    let engine = NativeEngine::default();
    let request = sensitivity_request(Some(vec![2.0, 2.0]));
    // 与解池/IIS 选项混用都必须拒绝。 / Mixing with pool or IIS options must fail.
    let mut with_pool = request.clone();
    with_pool.solution_pool_options = Some(ospf_rust_bridge::LinearSolutionPoolOptions { amount: 3 });
    assert_eq!(engine.submit(with_pool).unwrap_err().code, "InvalidInput");
    let mut with_iis = request.clone();
    with_iis.iis_options = Some(ospf_rust_bridge::LinearIisOptions { algorithm: None, include_bounds: None, max_iterations: None });
    assert_eq!(engine.submit(with_iis).unwrap_err().code, "InvalidInput");
    // 缺 modelKind、缺 incumbent、空 incumbent、非有限值都必须拒绝。
    // A missing modelKind, missing/empty incumbent and non-finite values must all fail.
    assert_eq!(engine.submit(SolveRequest { schedule_log_probe: None, operator_capability_probe: None, hypervolume_probe: None, acceptance_recovery_probe: None, training_split_probe: None, non_stationary_probe: None, selection_policy_probe: None, solve_value_probe: None, native_lowering_probe: None, native_terminal_proof_probe: None, predicate_scope_probe: None, persistence_schema_probe: None, persistence_sort_probe: None, effectiveness_ranking_probe: None, funnel_ranking: None, sensitivity_probe: None, correction_probe: None, blocking_set: None, fingerprint_probe: None, diagnostics_probe: None, gap_probe: None, proof_probe: None, evidence_probe: None, band_probe: None, group_summary: None, critical_report: None, effectiveness_probe: None, funnel: None, solution_status: None, diversity: None, piecewise: None, archive_probe: None, migration: None, topology: None, ranking_probe: None, acceptance: None, boundary: None, population_config: None, constraint_priority: None, constraint_ranking: None, pareto_archive: None, reference_points: None, dispersion: None, dominance: None, evidence_remap: None, tier_selection: None, activation_set: None, container_surface: None, identity_wrap: None, domain_surface: None, expression_bounds: None, assignment_eval: None, constraint_canon: None, snapshot_gate: None, cp_model_build: None, no_good_oracle: None, subproblem_result: None, cp_support_merge: None, capability_matrix: None, analysis_options: None, model_view: None, polynomial_probe: None, matrix_rows: None, sparse_triad: None, master_assignment: None, stable_value: None, quadratic_elastic: None, dump_batch: None, lazy_flatten_init: None, candidate_shortlist: None, writer_registry: None, native_write_scope: None, dump_options: None, assignment_composition: None, cut_validity: None, source_identity: None, binding_assignment: None, auxiliary_domain: None, activity_report_check: None, evidence_predicates: None, activity_bands: None, encoding_label: None, expression_key: None, canonical_monomial: None, backend_features: None, deferred_materialize: None, quadratic_monomial: None, polarity_encoding: None, tier_classification: None, assumption_binding: None, effectiveness_aggregate: None, bounded_no_good: None, polynomial_lift: None, scalar_function: None, field_resolution: None, boolean_fold: None, master_cut: None, flatten_identity: None, critical_assembly: None, variable_kind_domain: None, sort_by: None, predicate_schema: None, search_budget: None, conditional_semantics: None, big_m_policy: None, number_text: None, combine_status: None, elastic_model: None, native_write: None, input_domain: None, function_usage: None, repository_call: None, unsupported_predicate: None, boolean_scope: None, update_assignment: None, flatten_cache: None, cache_key_address: None, cache_key_identity: None, model_configuration: None, predicate_field_filter: None, predicate_field_resolution: None, analysis_group_aggregate: None, master_cut_encoding: None, q_table: None, heuristic_schedule: None, symbol_core_relations: None, symbol_linearization: None, symbol_condition: None, benders_cut_request: None, benders_metrics: None, cp_checkpoint_request: None, lbb_resume_identity: None, linear_mapping_request: None, checkpoint_request: None, reward_policy: None, hyper_heuristic_config: None, criticality_request: None, multi_target_report: None, relaxation_policy: None, target_spec: None, target_report: None, perturbation_policy: None, fixed_integer_report: None, fixed_integer_config: None, relaxation_request: None, critical_analysis: None, effectiveness_ranking: None, shortlist_limit: None, candidate_funnel: None, conflict_explanation: None, remote_artifact: None, logic_based_benders_plan: None, master_solution: None, benders_quadratic_cuts: None, pipeline_steps: None, repository_query: None, assignments: None, sort_plan: None, predicate: None, benders_cuts: None, relational_query_plan: None, linear_lifecycle: None,  indexed_registration: None, column_batch: None, column_scope: None, heart_beat: None, response_envelope: None, quadratic_orchestration: None, vrp_problem: None, network_resilience_problem: None, allocation_problem: None, network_structure_problem: None, network_design_problem: None, network_generalized_flow_problem: None, network_flow_problem: None, sweep_cases: None,  model_kind: None, ..request.clone() }).unwrap_err().code, "InvalidInput");
    assert_eq!(engine.submit(sensitivity_request(None)).unwrap_err().code, "InvalidInput");
    assert_eq!(engine.submit(sensitivity_request(Some(vec![]))).unwrap_err().code, "InvalidInput");
    assert_eq!(engine.submit(sensitivity_request(Some(vec![2.0, f64::NAN]))).unwrap_err().code, "InvalidInput");
}

#[cfg(feature = "scip")]
#[test]
fn sensitivity_job_reports_local_effectiveness_for_a_real_incumbent() {
    let engine = NativeEngine::default();
    let view = engine.submit(sensitivity_request(Some(vec![2.0, 2.0]))).unwrap();
    let finished = std::iter::repeat_with(|| { std::thread::sleep(std::time::Duration::from_millis(20)); engine.get(&view.id).unwrap() })
        .take(1500).find(|job| job.status != JobStatus::Running).expect("sensitivity job never reached a terminal state");
    assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
    let report: serde_json::Value = serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
    // 结构必须来自原 core 的报告：版本、baseline、每条约束的局部证据都在。
    // The shape must come from the original core report: version, baseline and per-constraint local evidence.
    assert!(report["schemaVersion"].as_str().is_some_and(|value| !value.is_empty()));
    assert_eq!(report["baselineObjective"].as_f64(), Some(4.0));
    assert_eq!(report["fixedIntegerCount"].as_u64(), Some(2));
    let constraints = report["constraints"].as_array().expect("constraints must be an array");
    assert!(!constraints.is_empty());
    // 约束身份必须是原模型的稳定 ID，而不是 solver 行号。 / Identities must be the model's stable IDs, not row numbers.
    assert!(constraints.iter().all(|entry| entry["constraintId"].as_str().is_some_and(|id| id.contains("capacity"))));
    engine.release(&view.id).unwrap();
}

#[cfg(feature = "scip")]
#[test]
fn infeasible_incumbent_is_rejected_before_any_sensitivity_claim() {
    let engine = NativeEngine::default();
    let view = engine.submit(sensitivity_request(Some(vec![4.0, 4.0]))).unwrap();
    let finished = std::iter::repeat_with(|| { std::thread::sleep(std::time::Duration::from_millis(20)); engine.get(&view.id).unwrap() })
        .take(1500).find(|job| job.status != JobStatus::Running).expect("sensitivity job never reached a terminal state");
    // 不可行 incumbent 必须失败，而不是给出空的敏感性结论。 / An infeasible incumbent must fail rather than yield an
    // empty sensitivity conclusion.
    assert_eq!(finished.status, JobStatus::Failed);
    assert!(finished.report_json.is_none());
    assert!(finished.error.is_some());
    engine.release(&view.id).unwrap();
}
