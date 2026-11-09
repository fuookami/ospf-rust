//! 扫描入口的原生端契约：只接受自己的选项，case 结果按 ID 稳定排序，且每个 case 恰好一个结论。
//! Native contract for the sweep entry: it accepts only its own options, results sort stably by case ID, and every case
//! reaches exactly one verdict.

use crate::*;
use ospf_rust_bridge::LinearSweepCase;

/// `x + y <= 4`，最大化 `x + y`：同一模型在不同预算/后端下的对比。
/// `x + y <= 4` maximizing `x + y`: one model compared across budgets and backends.
fn sweep_request(cases: Vec<LinearSweepCase>) -> SolveRequest { use ospf_rust_framework::solver::remote::{SerializedConstraint, SerializedConstraintCell, SerializedConstraintSign, SerializedVariable, SerializedVariableType};
    let mut input = ospf_rust_framework::solver::remote::SerializedLinearModel::empty("sweep-native");
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
    SolveRequest { schedule_log_probe: None, operator_capability_probe: None, hypervolume_probe: None, acceptance_recovery_probe: None, training_split_probe: None, non_stationary_probe: None, selection_policy_probe: None, solve_value_probe: None, native_lowering_probe: None, native_terminal_proof_probe: None, predicate_scope_probe: None, persistence_schema_probe: None, persistence_sort_probe: None, effectiveness_ranking_probe: None, funnel_ranking: None, sensitivity_probe: None, correction_probe: None, blocking_set: None, fingerprint_probe: None, diagnostics_probe: None, gap_probe: None, proof_probe: None, evidence_probe: None, band_probe: None, group_summary: None, critical_report: None, effectiveness_probe: None, funnel: None, solution_status: None, diversity: None, piecewise: None, archive_probe: None, migration: None, topology: None, ranking_probe: None, acceptance: None, boundary: None, population_config: None, constraint_priority: None, constraint_ranking: None, pareto_archive: None, reference_points: None, dispersion: None, dominance: None, evidence_remap: None, tier_selection: None, activation_set: None, container_surface: None, identity_wrap: None, domain_surface: None, expression_bounds: None, assignment_eval: None, constraint_canon: None, snapshot_gate: None, cp_model_build: None, no_good_oracle: None, subproblem_result: None, cp_support_merge: None, capability_matrix: None, analysis_options: None, model_view: None, polynomial_probe: None, matrix_rows: None, sparse_triad: None, master_assignment: None, stable_value: None, quadratic_elastic: None, dump_batch: None, lazy_flatten_init: None, candidate_shortlist: None, writer_registry: None, native_write_scope: None, dump_options: None, assignment_composition: None, cut_validity: None, source_identity: None, binding_assignment: None, auxiliary_domain: None, activity_report_check: None, evidence_predicates: None, activity_bands: None, encoding_label: None, expression_key: None, canonical_monomial: None, backend_features: None, deferred_materialize: None, quadratic_monomial: None, polarity_encoding: None, tier_classification: None, assumption_binding: None, effectiveness_aggregate: None, bounded_no_good: None, polynomial_lift: None, scalar_function: None, field_resolution: None, boolean_fold: None, master_cut: None, flatten_identity: None, critical_assembly: None, variable_kind_domain: None, sort_by: None, predicate_schema: None, search_budget: None, conditional_semantics: None, big_m_policy: None, number_text: None, combine_status: None, elastic_model: None, native_write: None, input_domain: None, function_usage: None, repository_call: None, unsupported_predicate: None, boolean_scope: None, update_assignment: None, flatten_cache: None, cache_key_address: None, cache_key_identity: None, model_configuration: None, predicate_field_filter: None, predicate_field_resolution: None, analysis_group_aggregate: None, master_cut_encoding: None, q_table: None, heuristic_schedule: None, symbol_core_relations: None, symbol_linearization: None, symbol_condition: None, benders_cut_request: None, benders_metrics: None, cp_checkpoint_request: None, lbb_resume_identity: None, linear_mapping_request: None, checkpoint_request: None, reward_policy: None, hyper_heuristic_config: None, criticality_request: None, multi_target_report: None, relaxation_policy: None, target_spec: None, target_report: None, perturbation_policy: None, fixed_integer_report: None, fixed_integer_config: None, relaxation_request: None, critical_analysis: None, effectiveness_ranking: None, shortlist_limit: None, candidate_funnel: None, conflict_explanation: None, remote_artifact: None, logic_based_benders_plan: None, master_solution: None, benders_quadratic_cuts: None, pipeline_steps: None, repository_query: None, assignments: None, sort_plan: None, predicate: None, benders_cuts: None, relational_query_plan: None, linear_lifecycle: None,  indexed_registration: None, column_batch: None, column_scope: None, heart_beat: None, response_envelope: None, quadratic_orchestration: None, vrp_problem: None, network_resilience_problem: None, allocation_problem: None, network_structure_problem: None, network_design_problem: None, network_generalized_flow_problem: None, network_flow_problem: None, sweep_cases: Some(cases), incumbent_values: None, solution_pool_options: None, gantt_activity_options: None,
        bpp3d_options: None, protocol_version: 1, solver: "scip".into(), model: artifact, tuning_options: None,
        model_kind: Some(ModelKind::LinearSweep), time_limit_ms: Some(10_000), cp_options: None, serial_options: None,
        parallel_options: None, fixed_recourse_options: None, column_generation_options: None, bpp1d_options: None,
        bpp2d_options: None, bpp_loading_options: None, csp2d_options: None, csp1d_options: None, iis_options: None }
}

fn case(case_id: &str, solver: &str, time_limit_ms: Option<u64>) -> LinearSweepCase {
    LinearSweepCase { case_id: case_id.into(), solver: solver.into(), time_limit_ms }
}

#[test]
fn sweep_entry_rejects_option_mixing_and_broken_case_lists() {
    let engine = NativeEngine::default();
    let request = sweep_request(vec![case("a", "scip", Some(1000))]);
    // 与敏感性/解池/IIS 选项混用都必须拒绝。 / Mixing with sensitivity, pool or IIS options must fail.
    let mut with_incumbent = request.clone();
    with_incumbent.incumbent_values = Some(vec![1.0, 1.0]);
    assert_eq!(engine.submit(with_incumbent).unwrap_err().code, "InvalidInput");
    let mut with_pool = request.clone();
    with_pool.solution_pool_options = Some(ospf_rust_bridge::LinearSolutionPoolOptions { amount: 3 });
    assert_eq!(engine.submit(with_pool).unwrap_err().code, "InvalidInput");
    // 缺 modelKind、缺 case、重复 case ID 都必须在执行前拒绝。
    // A missing modelKind, missing cases and repeated case IDs must all fail before execution.
    assert_eq!(engine.submit(SolveRequest { schedule_log_probe: None, operator_capability_probe: None, hypervolume_probe: None, acceptance_recovery_probe: None, training_split_probe: None, non_stationary_probe: None, selection_policy_probe: None, solve_value_probe: None, native_lowering_probe: None, native_terminal_proof_probe: None, predicate_scope_probe: None, persistence_schema_probe: None, persistence_sort_probe: None, effectiveness_ranking_probe: None, funnel_ranking: None, sensitivity_probe: None, correction_probe: None, blocking_set: None, fingerprint_probe: None, diagnostics_probe: None, gap_probe: None, proof_probe: None, evidence_probe: None, band_probe: None, group_summary: None, critical_report: None, effectiveness_probe: None, funnel: None, solution_status: None, diversity: None, piecewise: None, archive_probe: None, migration: None, topology: None, ranking_probe: None, acceptance: None, boundary: None, population_config: None, constraint_priority: None, constraint_ranking: None, pareto_archive: None, reference_points: None, dispersion: None, dominance: None, evidence_remap: None, tier_selection: None, activation_set: None, container_surface: None, identity_wrap: None, domain_surface: None, expression_bounds: None, assignment_eval: None, constraint_canon: None, snapshot_gate: None, cp_model_build: None, no_good_oracle: None, subproblem_result: None, cp_support_merge: None, capability_matrix: None, analysis_options: None, model_view: None, polynomial_probe: None, matrix_rows: None, sparse_triad: None, master_assignment: None, stable_value: None, quadratic_elastic: None, dump_batch: None, lazy_flatten_init: None, candidate_shortlist: None, writer_registry: None, native_write_scope: None, dump_options: None, assignment_composition: None, cut_validity: None, source_identity: None, binding_assignment: None, auxiliary_domain: None, activity_report_check: None, evidence_predicates: None, activity_bands: None, encoding_label: None, expression_key: None, canonical_monomial: None, backend_features: None, deferred_materialize: None, quadratic_monomial: None, polarity_encoding: None, tier_classification: None, assumption_binding: None, effectiveness_aggregate: None, bounded_no_good: None, polynomial_lift: None, scalar_function: None, field_resolution: None, boolean_fold: None, master_cut: None, flatten_identity: None, critical_assembly: None, variable_kind_domain: None, sort_by: None, predicate_schema: None, search_budget: None, conditional_semantics: None, big_m_policy: None, number_text: None, combine_status: None, elastic_model: None, native_write: None, input_domain: None, function_usage: None, repository_call: None, unsupported_predicate: None, boolean_scope: None, update_assignment: None, flatten_cache: None, cache_key_address: None, cache_key_identity: None, model_configuration: None, predicate_field_filter: None, predicate_field_resolution: None, analysis_group_aggregate: None, master_cut_encoding: None, q_table: None, heuristic_schedule: None, symbol_core_relations: None, symbol_linearization: None, symbol_condition: None, benders_cut_request: None, benders_metrics: None, cp_checkpoint_request: None, lbb_resume_identity: None, linear_mapping_request: None, checkpoint_request: None, reward_policy: None, hyper_heuristic_config: None, criticality_request: None, multi_target_report: None, relaxation_policy: None, target_spec: None, target_report: None, perturbation_policy: None, fixed_integer_report: None, fixed_integer_config: None, relaxation_request: None, critical_analysis: None, effectiveness_ranking: None, shortlist_limit: None, candidate_funnel: None, conflict_explanation: None, remote_artifact: None, logic_based_benders_plan: None, master_solution: None, benders_quadratic_cuts: None, pipeline_steps: None, repository_query: None, assignments: None, sort_plan: None, predicate: None, benders_cuts: None, relational_query_plan: None, linear_lifecycle: None,  indexed_registration: None, column_batch: None, column_scope: None, heart_beat: None, response_envelope: None, quadratic_orchestration: None, model_kind: None, ..request.clone() }).unwrap_err().code, "InvalidInput");
    assert_eq!(engine.submit(sweep_request(vec![])).unwrap_err().code, "InvalidInput");
    assert_eq!(engine.submit(sweep_request(vec![case("a", "scip", None), case("a", "scip", None)])).unwrap_err().code, "InvalidInput");
}

#[cfg(feature = "scip")]
#[test]
fn sweep_job_sorts_cases_and_keeps_every_verdict() {
    let engine = NativeEngine::default();
    // 故意用乱序的 case ID，验证结果按 ID 稳定排序。
    // Case IDs are deliberately out of order to verify stable sorting by ID.
    let view = engine.submit(sweep_request(vec![
        case("z-fast", "scip", Some(1_000)),
        case("a-slow", "scip", Some(30_000)),
        // 未知后端必须给出错误结论，而不是让整个扫描失败或被静默丢弃。
        // An unknown backend must yield an error verdict instead of failing the whole sweep or being dropped.
        case("m-unknown", "no-such-backend", None),
    ])).unwrap();
    let finished = std::iter::repeat_with(|| { std::thread::sleep(std::time::Duration::from_millis(20)); engine.get(&view.id).unwrap() })
        .take(3000).find(|job| job.status != JobStatus::Running).expect("sweep job never reached a terminal state");
    assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
    let report: ospf_rust_bridge::LinearSweepReport = serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
    report.validate().unwrap();
    assert_eq!(report.model_fingerprint, finished.fingerprint);
    // 排序稳定且覆盖全部 case。 / Ordering is stable and covers every case.
    let ids: Vec<&str> = report.cases.iter().map(|case| case.case_id.as_str()).collect();
    assert_eq!(ids, vec!["a-slow", "m-unknown", "z-fast"]);
    // 两个 SCIP case 都拿到了真实最优证明，未知后端只拿到错误。
    // Both SCIP cases carry a real optimality proof while the unknown backend only gets an error.
    for id in ["a-slow", "z-fast"] {
        let case = report.cases.iter().find(|case| case.case_id == id).unwrap();
        let summary = case.summary.as_ref().unwrap_or_else(|| panic!("{id} had no summary: {case:?}"));
        assert_eq!(summary.objective_value, Some(4.0), "{id}");
        assert!(summary.has_optimality_proof, "{id}");
        assert_eq!(summary.values.len(), 2, "{id}");
        assert!(case.error_code.is_none());
    }
    let unknown = report.cases.iter().find(|case| case.case_id == "m-unknown").unwrap();
    assert!(unknown.summary.is_none());
    assert_eq!(unknown.error_code.as_deref(), Some("ExecutionFailure"));
    assert!(unknown.error_message.as_deref().is_some_and(|message| !message.is_empty()));
    engine.release(&view.id).unwrap();
}
