//! 网络流入口的原生端契约：领域问题由桥接层校验，求解复用共享预算/取消，解按领域结构交付。
//! Native contract for the network-flow entry: the bridge validates the domain problem, solving reuses the shared
//! budget and cancellation, and the solution is delivered in the domain structure.

use crate::*;
use ospf_rust_bridge::{NetworkFlowArc, NetworkFlowCommodity, NetworkFlowNode, NetworkFlowNodeQuantity, NetworkFlowProblem};
use ospf_rust_bridge::{PhysicalQuantity, QuantityUnit};

fn mass(value: f64) -> PhysicalQuantity { PhysicalQuantity { value, unit: QuantityUnit::Kilogram } }

/// 两节点单弧：`a` 供 5 kg、`b` 需 5 kg，容量 [0, 10] kg，成本 1 kg 当量/单位。
/// Two nodes and one arc: `a` supplies 5 kg, `b` demands 5 kg, capacity [0, 10] kg, unit cost 1.
fn flow_request() -> SolveRequest { let problem = NetworkFlowProblem {
        nodes: vec![NetworkFlowNode { id: "a".into() }, NetworkFlowNode { id: "b".into() }],
        arcs: vec![NetworkFlowArc { id: "a-b".into(), from: "a".into(), to: "b".into(),
            capacity_lower: mass(0.0), capacity_upper: mass(10.0), cost: mass(1.0) }],
        commodities: vec![NetworkFlowCommodity { id: "kg".into(),
            balances: vec![NetworkFlowNodeQuantity { node: "a".into(), quantity: mass(5.0) },
                NetworkFlowNodeQuantity { node: "b".into(), quantity: mass(-5.0) }],
            unit: None, allowed_arcs: None, costs: Vec::new(), capacity_consumption: Vec::new(), integer_step: None }],
    };
    SolveRequest { schedule_log_probe: None, operator_capability_probe: None, hypervolume_probe: None, acceptance_recovery_probe: None, training_split_probe: None, non_stationary_probe: None, selection_policy_probe: None, solve_value_probe: None, native_lowering_probe: None, native_terminal_proof_probe: None, predicate_scope_probe: None, persistence_schema_probe: None, persistence_sort_probe: None, effectiveness_ranking_probe: None, funnel_ranking: None, sensitivity_probe: None, correction_probe: None, blocking_set: None, fingerprint_probe: None, diagnostics_probe: None, gap_probe: None, proof_probe: None, evidence_probe: None, band_probe: None, group_summary: None, critical_report: None, effectiveness_probe: None, funnel: None, solution_status: None, diversity: None, piecewise: None, archive_probe: None, migration: None, topology: None, ranking_probe: None, acceptance: None, boundary: None, population_config: None, constraint_priority: None, constraint_ranking: None, pareto_archive: None, reference_points: None, dispersion: None, dominance: None, evidence_remap: None, tier_selection: None, activation_set: None, container_surface: None, identity_wrap: None, domain_surface: None, expression_bounds: None, assignment_eval: None, constraint_canon: None, snapshot_gate: None, cp_model_build: None, no_good_oracle: None, subproblem_result: None, cp_support_merge: None, capability_matrix: None, analysis_options: None, model_view: None, polynomial_probe: None, matrix_rows: None, sparse_triad: None, master_assignment: None, stable_value: None, quadratic_elastic: None, dump_batch: None, lazy_flatten_init: None, candidate_shortlist: None, writer_registry: None, native_write_scope: None, dump_options: None, assignment_composition: None, cut_validity: None, source_identity: None, binding_assignment: None, auxiliary_domain: None, activity_report_check: None, evidence_predicates: None, activity_bands: None, encoding_label: None, expression_key: None, canonical_monomial: None, backend_features: None, deferred_materialize: None, quadratic_monomial: None, polarity_encoding: None, tier_classification: None, assumption_binding: None, effectiveness_aggregate: None, bounded_no_good: None, polynomial_lift: None, scalar_function: None, field_resolution: None, boolean_fold: None, master_cut: None, flatten_identity: None, critical_assembly: None, variable_kind_domain: None, sort_by: None, predicate_schema: None, search_budget: None, conditional_semantics: None, big_m_policy: None, number_text: None, combine_status: None, elastic_model: None, native_write: None, input_domain: None, function_usage: None, repository_call: None, unsupported_predicate: None, boolean_scope: None, update_assignment: None, flatten_cache: None, cache_key_address: None, cache_key_identity: None, model_configuration: None, predicate_field_filter: None, predicate_field_resolution: None, analysis_group_aggregate: None, master_cut_encoding: None, q_table: None, heuristic_schedule: None, symbol_core_relations: None, symbol_linearization: None, symbol_condition: None, benders_cut_request: None, benders_metrics: None, cp_checkpoint_request: None, lbb_resume_identity: None, linear_mapping_request: None, checkpoint_request: None, reward_policy: None, hyper_heuristic_config: None, criticality_request: None, multi_target_report: None, relaxation_policy: None, target_spec: None, target_report: None, perturbation_policy: None, fixed_integer_report: None, fixed_integer_config: None, relaxation_request: None, critical_analysis: None, effectiveness_ranking: None, shortlist_limit: None, candidate_funnel: None, conflict_explanation: None, remote_artifact: None, logic_based_benders_plan: None, master_solution: None, benders_quadratic_cuts: None, pipeline_steps: None, repository_query: None, assignments: None, sort_plan: None, predicate: None, benders_cuts: None, relational_query_plan: None, linear_lifecycle: None,  indexed_registration: None, column_batch: None, column_scope: None, heart_beat: None, response_envelope: None, quadratic_orchestration: None, vrp_problem: None, network_resilience_problem: None, allocation_problem: None, network_structure_problem: None, network_design_problem: None, network_generalized_flow_problem: None, network_flow_problem: Some(problem), sweep_cases: None, incumbent_values: None, solution_pool_options: None,
        gantt_activity_options: None, bpp3d_options: None, protocol_version: 1, solver: "scip".into(),
        // 网络流不使用模型工件：领域问题自带全部结构，这里给一个空模型占位。
        // Network flow does not use the model artifact: the domain problem carries all the structure, so an empty
        // model is only a placeholder.
        model: ospf_rust_bridge::encode_model(&ospf_rust_core::model::MetaModel::<f64>::new("network-flow-placeholder")
            .try_to_linear_triad_model().unwrap()).unwrap(),
        tuning_options: None, model_kind: Some(ModelKind::NetworkFlow), time_limit_ms: Some(30_000), cp_options: None,
        serial_options: None, parallel_options: None, fixed_recourse_options: None, column_generation_options: None,
        bpp1d_options: None, bpp2d_options: None, bpp_loading_options: None, csp2d_options: None, csp1d_options: None,
        iis_options: None }
}

#[test]
fn network_flow_entry_rejects_option_mixing_and_unknown_backends() {
    let engine = NativeEngine::default();
    let request = flow_request();
    // 混入其它策略选项必须拒绝。 / Mixing another strategy option must fail.
    let mut mixed = request.clone();
    mixed.iis_options = Some(ospf_rust_bridge::LinearIisOptions { algorithm: None, include_bounds: None, max_iterations: None });
    assert_eq!(engine.submit(mixed).unwrap_err().code, "InvalidInput");
    // 缺 modelKind 与缺领域问题都必须拒绝。 / A missing modelKind and a missing domain problem must both fail.
    assert_eq!(engine.submit(SolveRequest { schedule_log_probe: None, operator_capability_probe: None, hypervolume_probe: None, acceptance_recovery_probe: None, training_split_probe: None, non_stationary_probe: None, selection_policy_probe: None, solve_value_probe: None, native_lowering_probe: None, native_terminal_proof_probe: None, predicate_scope_probe: None, persistence_schema_probe: None, persistence_sort_probe: None, effectiveness_ranking_probe: None, funnel_ranking: None, sensitivity_probe: None, correction_probe: None, blocking_set: None, fingerprint_probe: None, diagnostics_probe: None, gap_probe: None, proof_probe: None, evidence_probe: None, band_probe: None, group_summary: None, critical_report: None, effectiveness_probe: None, funnel: None, solution_status: None, diversity: None, piecewise: None, archive_probe: None, migration: None, topology: None, ranking_probe: None, acceptance: None, boundary: None, population_config: None, constraint_priority: None, constraint_ranking: None, pareto_archive: None, reference_points: None, dispersion: None, dominance: None, evidence_remap: None, tier_selection: None, activation_set: None, container_surface: None, identity_wrap: None, domain_surface: None, expression_bounds: None, assignment_eval: None, constraint_canon: None, snapshot_gate: None, cp_model_build: None, no_good_oracle: None, subproblem_result: None, cp_support_merge: None, capability_matrix: None, analysis_options: None, model_view: None, polynomial_probe: None, matrix_rows: None, sparse_triad: None, master_assignment: None, stable_value: None, quadratic_elastic: None, dump_batch: None, lazy_flatten_init: None, candidate_shortlist: None, writer_registry: None, native_write_scope: None, dump_options: None, assignment_composition: None, cut_validity: None, source_identity: None, binding_assignment: None, auxiliary_domain: None, activity_report_check: None, evidence_predicates: None, activity_bands: None, encoding_label: None, expression_key: None, canonical_monomial: None, backend_features: None, deferred_materialize: None, quadratic_monomial: None, polarity_encoding: None, tier_classification: None, assumption_binding: None, effectiveness_aggregate: None, bounded_no_good: None, polynomial_lift: None, scalar_function: None, field_resolution: None, boolean_fold: None, master_cut: None, flatten_identity: None, critical_assembly: None, variable_kind_domain: None, sort_by: None, predicate_schema: None, search_budget: None, conditional_semantics: None, big_m_policy: None, number_text: None, combine_status: None, elastic_model: None, native_write: None, input_domain: None, function_usage: None, repository_call: None, unsupported_predicate: None, boolean_scope: None, update_assignment: None, flatten_cache: None, cache_key_address: None, cache_key_identity: None, model_configuration: None, predicate_field_filter: None, predicate_field_resolution: None, analysis_group_aggregate: None, master_cut_encoding: None, q_table: None, heuristic_schedule: None, symbol_core_relations: None, symbol_linearization: None, symbol_condition: None, benders_cut_request: None, benders_metrics: None, cp_checkpoint_request: None, lbb_resume_identity: None, linear_mapping_request: None, checkpoint_request: None, reward_policy: None, hyper_heuristic_config: None, criticality_request: None, multi_target_report: None, relaxation_policy: None, target_spec: None, target_report: None, perturbation_policy: None, fixed_integer_report: None, fixed_integer_config: None, relaxation_request: None, critical_analysis: None, effectiveness_ranking: None, shortlist_limit: None, candidate_funnel: None, conflict_explanation: None, remote_artifact: None, logic_based_benders_plan: None, master_solution: None, benders_quadratic_cuts: None, pipeline_steps: None, repository_query: None, assignments: None, sort_plan: None, predicate: None, benders_cuts: None, relational_query_plan: None, linear_lifecycle: None,  indexed_registration: None, column_batch: None, column_scope: None, heart_beat: None, response_envelope: None, quadratic_orchestration: None, model_kind: None, ..request.clone() }).unwrap_err().code, "InvalidInput");
    let mut missing = request.clone();
    missing.network_flow_problem = None;
    assert_eq!(engine.submit(missing).unwrap_err().code, "InvalidInput");
    // 未知后端在入口之后由求解阶段显式拒绝，不做隐式回退。
    // An unknown backend is rejected explicitly during solving without implicit fallback.
    let mut unknown = request;
    unknown.solver = "no-such-backend".into();
    assert_eq!(engine.submit(unknown).unwrap_err().code, "UnsupportedCapability");
}

#[cfg(feature = "scip")]
#[test]
fn network_flow_job_solves_the_domain_problem_and_reports_domain_costs() {
    let engine = NativeEngine::default();
    let view = engine.submit(flow_request()).unwrap();
    let finished = std::iter::repeat_with(|| { std::thread::sleep(std::time::Duration::from_millis(20)); engine.get(&view.id).unwrap() })
        .take(3000).find(|job| job.status != JobStatus::Running).expect("network flow job never reached a terminal state");
    assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
    let report: ospf_rust_bridge::NetworkFlowReport = serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
    let solution = report.solution.as_ref().expect("a feasible flow must carry a domain solution");
    // 单弧单商品：流量必须等于供给，费用分项与总费用都要保留。
    // Single arc and single commodity: the flow must equal the supply and every cost component is preserved.
    assert_eq!(solution.flows.len(), 1, "{:?}", solution.flows);
    assert_eq!(solution.flows[0].commodity, "kg");
    assert_eq!(solution.flows[0].arc, "a-b");
    assert!((solution.flows[0].quantity.value - 5.0).abs() < 1e-6, "{:?}", solution.flows[0].quantity);
    // 报告侧保留原领域单位符号（`kg`），不做枚举回填。 / The report keeps the domain unit symbol (`kg`).
    assert_eq!(solution.flows[0].quantity.unit, "kg");
    assert!((solution.total_cost.value - 5.0).abs() < 1e-6, "total cost {}", solution.total_cost.value);
    assert!(solution.spills.is_empty() && solution.shortages.is_empty());
    // 统一摘要保留结论与证明，但 `values` 为空：领域解以领域结构交付，不伪装成列向量。
    // The shared summary keeps the verdict and proof but leaves `values` empty: the domain solution is delivered in
    // domain structure instead of pretending to be a column vector.
    assert_eq!(report.summary.objective_value, Some(5.0));
    assert!(report.summary.has_optimality_proof);
    assert!(report.summary.values.is_empty());
    engine.release(&view.id).unwrap();
}
