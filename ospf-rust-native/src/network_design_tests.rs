//! 网络建设入口的原生端契约：建设选择、名义流与费用必须一起交付且互相一致。
//! Native contract for the network-design entry: construction choices, nominal flow and cost are delivered together and
//! must agree with each other.

use crate::*;
use ospf_rust_bridge::{
    NetworkDesignFacility, NetworkDesignLevel, NetworkDesignProblem, NetworkDesignService, NetworkFlowArc,
    NetworkFlowCommodity, NetworkFlowNode, NetworkFlowNodeQuantity, PhysicalQuantity, QuantityUnit,
};

fn mass(value: f64) -> PhysicalQuantity { PhysicalQuantity { value, unit: QuantityUnit::Kilogram } }
fn pure(value: f64) -> PhysicalQuantity { PhysicalQuantity { value, unit: QuantityUnit::Dimensionless } }

/// `a` 供 5 kg、`b` 需 5 kg；通道 `a-b` 有 8 kg 与 10 kg 两档容量，必须选一档。
/// `a` supplies 5 kg and `b` demands 5 kg; service `a-b` offers 8 kg and 10 kg levels and one must be selected.
fn request() -> SolveRequest { let problem = NetworkDesignProblem {
        flow: ospf_rust_bridge::NetworkFlowProblem {
            nodes: vec![NetworkFlowNode { id: "a".into() }, NetworkFlowNode { id: "b".into() }],
            arcs: vec![NetworkFlowArc { id: "a-b".into(), from: "a".into(), to: "b".into(),
                capacity_lower: mass(0.0), capacity_upper: mass(10.0), cost: pure(1.0) }],
            commodities: vec![NetworkFlowCommodity { id: "c".into(),
                balances: vec![NetworkFlowNodeQuantity { node: "a".into(), quantity: mass(5.0) },
                    NetworkFlowNodeQuantity { node: "b".into(), quantity: mass(-5.0) }],
                unit: None, allowed_arcs: None, costs: Vec::new(), capacity_consumption: Vec::new(), integer_step: None }],
        },
        services: vec![NetworkDesignService { arc: "a-b".into(), required: true, levels: vec![
            NetworkDesignLevel { id: "small".into(), capacity: mass(8.0), cost: mass(3.0) },
            NetworkDesignLevel { id: "large".into(), capacity: mass(10.0), cost: mass(9.0) }] }],
        facilities: Vec::new(),
        budget: None,
    };
    SolveRequest { schedule_log_probe: None, operator_capability_probe: None, hypervolume_probe: None, acceptance_recovery_probe: None, training_split_probe: None, non_stationary_probe: None, selection_policy_probe: None, solve_value_probe: None, native_lowering_probe: None, native_terminal_proof_probe: None, predicate_scope_probe: None, persistence_schema_probe: None, persistence_sort_probe: None, effectiveness_ranking_probe: None, funnel_ranking: None, sensitivity_probe: None, correction_probe: None, blocking_set: None, fingerprint_probe: None, diagnostics_probe: None, gap_probe: None, proof_probe: None, evidence_probe: None, band_probe: None, group_summary: None, critical_report: None, effectiveness_probe: None, funnel: None, solution_status: None, diversity: None, piecewise: None, archive_probe: None, migration: None, topology: None, ranking_probe: None, acceptance: None, boundary: None, population_config: None, constraint_priority: None, constraint_ranking: None, pareto_archive: None, reference_points: None, dispersion: None, dominance: None, evidence_remap: None, tier_selection: None, activation_set: None, container_surface: None, identity_wrap: None, domain_surface: None, expression_bounds: None, assignment_eval: None, constraint_canon: None, snapshot_gate: None, cp_model_build: None, no_good_oracle: None, subproblem_result: None, cp_support_merge: None, capability_matrix: None, analysis_options: None, model_view: None, polynomial_probe: None, matrix_rows: None, sparse_triad: None, master_assignment: None, stable_value: None, quadratic_elastic: None, dump_batch: None, lazy_flatten_init: None, candidate_shortlist: None, writer_registry: None, native_write_scope: None, dump_options: None, assignment_composition: None, cut_validity: None, source_identity: None, binding_assignment: None, auxiliary_domain: None, activity_report_check: None, evidence_predicates: None, activity_bands: None, encoding_label: None, expression_key: None, canonical_monomial: None, backend_features: None, deferred_materialize: None, quadratic_monomial: None, polarity_encoding: None, tier_classification: None, assumption_binding: None, effectiveness_aggregate: None, bounded_no_good: None, polynomial_lift: None, scalar_function: None, field_resolution: None, boolean_fold: None, master_cut: None, flatten_identity: None, critical_assembly: None, variable_kind_domain: None, sort_by: None, predicate_schema: None, search_budget: None, conditional_semantics: None, big_m_policy: None, number_text: None, combine_status: None, elastic_model: None, native_write: None, input_domain: None, function_usage: None, repository_call: None, unsupported_predicate: None, boolean_scope: None, update_assignment: None, flatten_cache: None, cache_key_address: None, cache_key_identity: None, model_configuration: None, predicate_field_filter: None, predicate_field_resolution: None, analysis_group_aggregate: None, master_cut_encoding: None, q_table: None, heuristic_schedule: None, symbol_core_relations: None, symbol_linearization: None, symbol_condition: None, benders_cut_request: None, benders_metrics: None, cp_checkpoint_request: None, lbb_resume_identity: None, linear_mapping_request: None, checkpoint_request: None, reward_policy: None, hyper_heuristic_config: None, criticality_request: None, multi_target_report: None, relaxation_policy: None, target_spec: None, target_report: None, perturbation_policy: None, fixed_integer_report: None, fixed_integer_config: None, relaxation_request: None, critical_analysis: None, effectiveness_ranking: None, shortlist_limit: None, candidate_funnel: None, conflict_explanation: None, remote_artifact: None, logic_based_benders_plan: None, master_solution: None, benders_quadratic_cuts: None, pipeline_steps: None, repository_query: None, assignments: None, sort_plan: None, predicate: None, benders_cuts: None, relational_query_plan: None, linear_lifecycle: None,  indexed_registration: None, column_batch: None, column_scope: None, heart_beat: None, response_envelope: None, quadratic_orchestration: None, vrp_problem: None, network_resilience_problem: None, allocation_problem: None, network_structure_problem: None, network_design_problem: Some(problem), network_generalized_flow_problem: None, network_flow_problem: None,
        sweep_cases: None, incumbent_values: None, solution_pool_options: None, gantt_activity_options: None,
        bpp3d_options: None, protocol_version: 1, solver: "scip".into(),
        model: ospf_rust_bridge::encode_model(&ospf_rust_core::model::MetaModel::<f64>::new("network-design-placeholder")
            .try_to_linear_triad_model().unwrap()).unwrap(),
        tuning_options: None, model_kind: Some(ModelKind::NetworkDesign), time_limit_ms: Some(30_000),
        cp_options: None, serial_options: None, parallel_options: None, fixed_recourse_options: None,
        column_generation_options: None, bpp1d_options: None, bpp2d_options: None, bpp_loading_options: None,
        csp2d_options: None, csp1d_options: None, iis_options: None }
}

#[test]
fn network_design_entry_rejects_option_mixing_and_unknown_backends() {
    let engine = NativeEngine::default();
    let request = request();
    let mut mixed = request.clone();
    mixed.network_flow_problem = Some(ospf_rust_bridge::NetworkFlowProblem { nodes: Vec::new(), arcs: Vec::new(), commodities: Vec::new() });
    assert_eq!(engine.submit(mixed).unwrap_err().code, "InvalidInput");
    assert_eq!(engine.submit(SolveRequest { schedule_log_probe: None, operator_capability_probe: None, hypervolume_probe: None, acceptance_recovery_probe: None, training_split_probe: None, non_stationary_probe: None, selection_policy_probe: None, solve_value_probe: None, native_lowering_probe: None, native_terminal_proof_probe: None, predicate_scope_probe: None, persistence_schema_probe: None, persistence_sort_probe: None, effectiveness_ranking_probe: None, funnel_ranking: None, sensitivity_probe: None, correction_probe: None, blocking_set: None, fingerprint_probe: None, diagnostics_probe: None, gap_probe: None, proof_probe: None, evidence_probe: None, band_probe: None, group_summary: None, critical_report: None, effectiveness_probe: None, funnel: None, solution_status: None, diversity: None, piecewise: None, archive_probe: None, migration: None, topology: None, ranking_probe: None, acceptance: None, boundary: None, population_config: None, constraint_priority: None, constraint_ranking: None, pareto_archive: None, reference_points: None, dispersion: None, dominance: None, evidence_remap: None, tier_selection: None, activation_set: None, container_surface: None, identity_wrap: None, domain_surface: None, expression_bounds: None, assignment_eval: None, constraint_canon: None, snapshot_gate: None, cp_model_build: None, no_good_oracle: None, subproblem_result: None, cp_support_merge: None, capability_matrix: None, analysis_options: None, model_view: None, polynomial_probe: None, matrix_rows: None, sparse_triad: None, master_assignment: None, stable_value: None, quadratic_elastic: None, dump_batch: None, lazy_flatten_init: None, candidate_shortlist: None, writer_registry: None, native_write_scope: None, dump_options: None, assignment_composition: None, cut_validity: None, source_identity: None, binding_assignment: None, auxiliary_domain: None, activity_report_check: None, evidence_predicates: None, activity_bands: None, encoding_label: None, expression_key: None, canonical_monomial: None, backend_features: None, deferred_materialize: None, quadratic_monomial: None, polarity_encoding: None, tier_classification: None, assumption_binding: None, effectiveness_aggregate: None, bounded_no_good: None, polynomial_lift: None, scalar_function: None, field_resolution: None, boolean_fold: None, master_cut: None, flatten_identity: None, critical_assembly: None, variable_kind_domain: None, sort_by: None, predicate_schema: None, search_budget: None, conditional_semantics: None, big_m_policy: None, number_text: None, combine_status: None, elastic_model: None, native_write: None, input_domain: None, function_usage: None, repository_call: None, unsupported_predicate: None, boolean_scope: None, update_assignment: None, flatten_cache: None, cache_key_address: None, cache_key_identity: None, model_configuration: None, predicate_field_filter: None, predicate_field_resolution: None, analysis_group_aggregate: None, master_cut_encoding: None, q_table: None, heuristic_schedule: None, symbol_core_relations: None, symbol_linearization: None, symbol_condition: None, benders_cut_request: None, benders_metrics: None, cp_checkpoint_request: None, lbb_resume_identity: None, linear_mapping_request: None, checkpoint_request: None, reward_policy: None, hyper_heuristic_config: None, criticality_request: None, multi_target_report: None, relaxation_policy: None, target_spec: None, target_report: None, perturbation_policy: None, fixed_integer_report: None, fixed_integer_config: None, relaxation_request: None, critical_analysis: None, effectiveness_ranking: None, shortlist_limit: None, candidate_funnel: None, conflict_explanation: None, remote_artifact: None, logic_based_benders_plan: None, master_solution: None, benders_quadratic_cuts: None, pipeline_steps: None, repository_query: None, assignments: None, sort_plan: None, predicate: None, benders_cuts: None, relational_query_plan: None, linear_lifecycle: None,  indexed_registration: None, column_batch: None, column_scope: None, heart_beat: None, response_envelope: None, quadratic_orchestration: None, model_kind: None, ..request.clone() }).unwrap_err().code, "InvalidInput");
    let mut missing = request.clone();
    missing.network_design_problem = None;
    assert_eq!(engine.submit(missing).unwrap_err().code, "InvalidInput");
    let mut unknown = request;
    unknown.solver = "no-such-backend".into();
    assert_eq!(engine.submit(unknown).unwrap_err().code, "UnsupportedCapability");
}

#[cfg(feature = "scip")]
#[test]
fn network_design_job_returns_construction_and_nominal_flow_together() {
    let engine = NativeEngine::default();
    let view = engine.submit(request()).unwrap();
    let finished = std::iter::repeat_with(|| { std::thread::sleep(std::time::Duration::from_millis(20)); engine.get(&view.id).unwrap() })
        .take(3000).find(|job| job.status != JobStatus::Running).expect("network design job never reached a terminal state");
    assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
    let report: ospf_rust_bridge::NetworkDesignReport = serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
    let solution = report.solution.as_ref().expect("a feasible design must carry a domain solution");
    // 必须恰好选中一档，且选中的是最便宜的可行档（8 kg 已满足 5 kg 需求）。
    // Exactly one level must be selected, and it must be the cheapest feasible one (8 kg already covers 5 kg).
    assert_eq!(solution.construction.service_levels.len(), 1, "{:?}", solution.construction.service_levels);
    assert_eq!(solution.construction.service_levels[0].arc, "a-b");
    assert_eq!(solution.construction.service_levels[0].level, "small");
    assert!((solution.construction.cost.value - 3.0).abs() < 1e-6, "construction cost {}", solution.construction.cost.value);
    // 名义流必须与需求一致，总费用 = 建设费 + 运行费。
    // The nominal flow must cover the demand and the total cost is construction plus operating cost.
    assert_eq!(solution.flow.flows.len(), 1, "{:?}", solution.flow.flows);
    assert!((solution.flow.flows[0].quantity.value - 5.0).abs() < 1e-6, "flow {}", solution.flow.flows[0].quantity.value);
    let operating = solution.flow.total_cost.value;
    assert!((solution.total_cost.value - (3.0 + operating)).abs() < 1e-6,
        "total {} vs construction 3 + operating {operating}", solution.total_cost.value);
    assert!(report.summary.values.is_empty());
    engine.release(&view.id).unwrap();
}
