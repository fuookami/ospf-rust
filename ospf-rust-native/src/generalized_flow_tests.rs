//! 广义流入口的原生端契约：产率与单位转换必须真的生效，且弧/节点结果分开交付。
//! Native contract for the generalized-flow entry: production yield and unit conversion must really take effect, and
//! arc/node results are delivered separately.

use crate::*;
use ospf_rust_bridge::{GeneralizedFlowArc, GeneralizedFlowNode, GeneralizedFlowProblem, PhysicalQuantity, QuantityUnit};

fn mass(value: f64) -> PhysicalQuantity { PhysicalQuantity { value, unit: QuantityUnit::Kilogram } }
fn pure(value: f64) -> PhysicalQuantity { PhysicalQuantity { value, unit: QuantityUnit::Dimensionless } }

/// `a` 供 10 kg、`b` 需 8 kg；弧产率 0.8，投入 10 产出 8，转换差额为 0。
/// `a` supplies 10 kg and `b` demands 8 kg; the arc yields 0.8, so 10 in gives 8 out with no conversion difference.
fn request() -> SolveRequest { let problem = GeneralizedFlowProblem {
        nodes: vec![GeneralizedFlowNode { id: "a".into(), net_supply: mass(10.0) },
            GeneralizedFlowNode { id: "b".into(), net_supply: mass(-8.0) }],
        arcs: vec![GeneralizedFlowArc { id: "a-b".into(), from: "a".into(), to: "b".into(),
            input_lower: mass(0.0), input_upper: mass(20.0),
            // 成本按"每投入单位"计，因此 costUnit 等于投入单位时成本无量纲。
            // Cost is per input unit, so with costUnit equal to the input unit the cost itself is dimensionless.
            cost: pure(1.0), production_yield: pure(0.8), input_to_output_factor: Some(pure(1.0)) }],
        cost_unit: QuantityUnit::Kilogram,
    };
    SolveRequest { schedule_log_probe: None, operator_capability_probe: None, hypervolume_probe: None, acceptance_recovery_probe: None, training_split_probe: None, non_stationary_probe: None, selection_policy_probe: None, solve_value_probe: None, native_lowering_probe: None, native_terminal_proof_probe: None, predicate_scope_probe: None, persistence_schema_probe: None, persistence_sort_probe: None, effectiveness_ranking_probe: None, funnel_ranking: None, sensitivity_probe: None, correction_probe: None, blocking_set: None, fingerprint_probe: None, diagnostics_probe: None, gap_probe: None, proof_probe: None, evidence_probe: None, band_probe: None, group_summary: None, critical_report: None, effectiveness_probe: None, funnel: None, solution_status: None, diversity: None, piecewise: None, archive_probe: None, migration: None, topology: None, ranking_probe: None, acceptance: None, boundary: None, population_config: None, constraint_priority: None, constraint_ranking: None, pareto_archive: None, reference_points: None, dispersion: None, dominance: None, evidence_remap: None, tier_selection: None, activation_set: None, container_surface: None, identity_wrap: None, domain_surface: None, expression_bounds: None, assignment_eval: None, constraint_canon: None, snapshot_gate: None, cp_model_build: None, no_good_oracle: None, subproblem_result: None, cp_support_merge: None, capability_matrix: None, analysis_options: None, model_view: None, polynomial_probe: None, matrix_rows: None, sparse_triad: None, master_assignment: None, stable_value: None, quadratic_elastic: None, dump_batch: None, lazy_flatten_init: None, candidate_shortlist: None, writer_registry: None, native_write_scope: None, dump_options: None, assignment_composition: None, cut_validity: None, source_identity: None, binding_assignment: None, auxiliary_domain: None, activity_report_check: None, evidence_predicates: None, activity_bands: None, encoding_label: None, expression_key: None, canonical_monomial: None, backend_features: None, deferred_materialize: None, quadratic_monomial: None, polarity_encoding: None, tier_classification: None, assumption_binding: None, effectiveness_aggregate: None, bounded_no_good: None, polynomial_lift: None, scalar_function: None, field_resolution: None, boolean_fold: None, master_cut: None, flatten_identity: None, critical_assembly: None, variable_kind_domain: None, sort_by: None, predicate_schema: None, search_budget: None, conditional_semantics: None, big_m_policy: None, number_text: None, combine_status: None, elastic_model: None, native_write: None, input_domain: None, function_usage: None, repository_call: None, unsupported_predicate: None, boolean_scope: None, update_assignment: None, flatten_cache: None, cache_key_address: None, cache_key_identity: None, model_configuration: None, predicate_field_filter: None, predicate_field_resolution: None, analysis_group_aggregate: None, master_cut_encoding: None, q_table: None, heuristic_schedule: None, symbol_core_relations: None, symbol_linearization: None, symbol_condition: None, benders_cut_request: None, benders_metrics: None, cp_checkpoint_request: None, lbb_resume_identity: None, linear_mapping_request: None, checkpoint_request: None, reward_policy: None, hyper_heuristic_config: None, criticality_request: None, multi_target_report: None, relaxation_policy: None, target_spec: None, target_report: None, perturbation_policy: None, fixed_integer_report: None, fixed_integer_config: None, relaxation_request: None, critical_analysis: None, effectiveness_ranking: None, shortlist_limit: None, candidate_funnel: None, conflict_explanation: None, remote_artifact: None, logic_based_benders_plan: None, master_solution: None, benders_quadratic_cuts: None, pipeline_steps: None, repository_query: None, assignments: None, sort_plan: None, predicate: None, benders_cuts: None, relational_query_plan: None, linear_lifecycle: None,  indexed_registration: None, column_batch: None, column_scope: None, heart_beat: None, response_envelope: None, quadratic_orchestration: None, vrp_problem: None, network_resilience_problem: None, allocation_problem: None, network_structure_problem: None, network_design_problem: None, network_generalized_flow_problem: Some(problem), network_flow_problem: None, sweep_cases: None,
        incumbent_values: None, solution_pool_options: None, gantt_activity_options: None, bpp3d_options: None,
        protocol_version: 1, solver: "scip".into(),
        model: ospf_rust_bridge::encode_model(&ospf_rust_core::model::MetaModel::<f64>::new("generalized-flow-placeholder")
            .try_to_linear_triad_model().unwrap()).unwrap(),
        tuning_options: None, model_kind: Some(ModelKind::NetworkGeneralizedFlow), time_limit_ms: Some(30_000),
        cp_options: None, serial_options: None, parallel_options: None, fixed_recourse_options: None,
        column_generation_options: None, bpp1d_options: None, bpp2d_options: None, bpp_loading_options: None,
        csp2d_options: None, csp1d_options: None, iis_options: None }
}

#[test]
fn generalized_flow_entry_rejects_option_mixing_and_unknown_backends() {
    let engine = NativeEngine::default();
    let request = request();
    let mut mixed = request.clone();
    mixed.network_flow_problem = Some(ospf_rust_bridge::NetworkFlowProblem { nodes: Vec::new(), arcs: Vec::new(), commodities: Vec::new() });
    assert_eq!(engine.submit(mixed).unwrap_err().code, "InvalidInput");
    assert_eq!(engine.submit(SolveRequest { schedule_log_probe: None, operator_capability_probe: None, hypervolume_probe: None, acceptance_recovery_probe: None, training_split_probe: None, non_stationary_probe: None, selection_policy_probe: None, solve_value_probe: None, native_lowering_probe: None, native_terminal_proof_probe: None, predicate_scope_probe: None, persistence_schema_probe: None, persistence_sort_probe: None, effectiveness_ranking_probe: None, funnel_ranking: None, sensitivity_probe: None, correction_probe: None, blocking_set: None, fingerprint_probe: None, diagnostics_probe: None, gap_probe: None, proof_probe: None, evidence_probe: None, band_probe: None, group_summary: None, critical_report: None, effectiveness_probe: None, funnel: None, solution_status: None, diversity: None, piecewise: None, archive_probe: None, migration: None, topology: None, ranking_probe: None, acceptance: None, boundary: None, population_config: None, constraint_priority: None, constraint_ranking: None, pareto_archive: None, reference_points: None, dispersion: None, dominance: None, evidence_remap: None, tier_selection: None, activation_set: None, container_surface: None, identity_wrap: None, domain_surface: None, expression_bounds: None, assignment_eval: None, constraint_canon: None, snapshot_gate: None, cp_model_build: None, no_good_oracle: None, subproblem_result: None, cp_support_merge: None, capability_matrix: None, analysis_options: None, model_view: None, polynomial_probe: None, matrix_rows: None, sparse_triad: None, master_assignment: None, stable_value: None, quadratic_elastic: None, dump_batch: None, lazy_flatten_init: None, candidate_shortlist: None, writer_registry: None, native_write_scope: None, dump_options: None, assignment_composition: None, cut_validity: None, source_identity: None, binding_assignment: None, auxiliary_domain: None, activity_report_check: None, evidence_predicates: None, activity_bands: None, encoding_label: None, expression_key: None, canonical_monomial: None, backend_features: None, deferred_materialize: None, quadratic_monomial: None, polarity_encoding: None, tier_classification: None, assumption_binding: None, effectiveness_aggregate: None, bounded_no_good: None, polynomial_lift: None, scalar_function: None, field_resolution: None, boolean_fold: None, master_cut: None, flatten_identity: None, critical_assembly: None, variable_kind_domain: None, sort_by: None, predicate_schema: None, search_budget: None, conditional_semantics: None, big_m_policy: None, number_text: None, combine_status: None, elastic_model: None, native_write: None, input_domain: None, function_usage: None, repository_call: None, unsupported_predicate: None, boolean_scope: None, update_assignment: None, flatten_cache: None, cache_key_address: None, cache_key_identity: None, model_configuration: None, predicate_field_filter: None, predicate_field_resolution: None, analysis_group_aggregate: None, master_cut_encoding: None, q_table: None, heuristic_schedule: None, symbol_core_relations: None, symbol_linearization: None, symbol_condition: None, benders_cut_request: None, benders_metrics: None, cp_checkpoint_request: None, lbb_resume_identity: None, linear_mapping_request: None, checkpoint_request: None, reward_policy: None, hyper_heuristic_config: None, criticality_request: None, multi_target_report: None, relaxation_policy: None, target_spec: None, target_report: None, perturbation_policy: None, fixed_integer_report: None, fixed_integer_config: None, relaxation_request: None, critical_analysis: None, effectiveness_ranking: None, shortlist_limit: None, candidate_funnel: None, conflict_explanation: None, remote_artifact: None, logic_based_benders_plan: None, master_solution: None, benders_quadratic_cuts: None, pipeline_steps: None, repository_query: None, assignments: None, sort_plan: None, predicate: None, benders_cuts: None, relational_query_plan: None, linear_lifecycle: None,  indexed_registration: None, column_batch: None, column_scope: None, heart_beat: None, response_envelope: None, quadratic_orchestration: None, model_kind: None, ..request.clone() }).unwrap_err().code, "InvalidInput");
    let mut missing = request.clone();
    missing.network_generalized_flow_problem = None;
    assert_eq!(engine.submit(missing).unwrap_err().code, "InvalidInput");
    let mut unknown = request;
    unknown.solver = "no-such-backend".into();
    assert_eq!(engine.submit(unknown).unwrap_err().code, "UnsupportedCapability");
}

#[cfg(feature = "scip")]
#[test]
fn generalized_flow_job_applies_yield_and_reports_arc_and_node_results() {
    let engine = NativeEngine::default();
    let view = engine.submit(request()).unwrap();
    let finished = std::iter::repeat_with(|| { std::thread::sleep(std::time::Duration::from_millis(20)); engine.get(&view.id).unwrap() })
        .take(3000).find(|job| job.status != JobStatus::Running).expect("generalized flow job never reached a terminal state");
    assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
    let report: ospf_rust_bridge::GeneralizedFlowReport = serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
    let solution = report.solution.as_ref().expect("a feasible generalized flow must carry a domain solution");
    // 产率必须真的生效：投入 10 kg、产出 8 kg，产率损耗 = 未计产率的产出 10 − 实际产出 8 = 2。
    // The yield must really take effect: 10 kg in and 8 kg out, with a yield loss of 10 - 8 = 2.
    assert_eq!(solution.arcs.len(), 1, "{:?}", solution.arcs);
    let arc = &solution.arcs[0];
    assert_eq!(arc.id, "a-b");
    assert!((arc.input.value - 10.0).abs() < 1e-6, "input {}", arc.input.value);
    assert!((arc.output.value - 8.0).abs() < 1e-6, "output {}", arc.output.value);
    assert!((arc.loss_or_gain.value - 2.0).abs() < 1e-6, "yield loss {}", arc.loss_or_gain.value);
    assert!((arc.cost.value - 10.0).abs() < 1e-6, "cost {}", arc.cost.value);
    // 节点结果独立复核净供给，残差必须为零；总费用按 costUnit 计。
    // Node results independently check net supply with a zero residual, and the total cost uses costUnit.
    assert_eq!(solution.nodes.len(), 2, "{:?}", solution.nodes);
    for node in &solution.nodes {
        assert!(node.residual.value.abs() < 1e-6, "node {} residual {}", node.id, node.residual.value);
    }
    assert!((solution.total_cost.value - 10.0).abs() < 1e-6, "total cost {}", solution.total_cost.value);
    assert_eq!(solution.total_cost.unit, "kg");
    assert_eq!(report.summary.objective_value, Some(10.0));
    // 领域报告的摘要不伪装成列向量。 / A domain report summary never pretends to be a column vector.
    assert!(report.summary.values.is_empty());
    engine.release(&view.id).unwrap();
}
