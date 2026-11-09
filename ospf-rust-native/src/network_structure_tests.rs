//! 网络结构入口的原生端契约：根树、建设选择与分区归属必须一起交付。
//! Native contract for the network-structure entry: the rooted tree, construction choices and partition ownership are
//! delivered together.

use crate::*;
use ospf_rust_bridge::{
    NetworkDesignFacility, NetworkDesignLevel, NetworkDesignProblem, NetworkDesignService, NetworkFlowArc,
    NetworkFlowCommodity, NetworkFlowNode, NetworkFlowNodeQuantity, NetworkStructurePartition, NetworkStructureProblem,
    PhysicalQuantity, QuantityUnit,
};

fn mass(value: f64) -> PhysicalQuantity { PhysicalQuantity { value, unit: QuantityUnit::Kilogram } }
fn pure(value: f64) -> PhysicalQuantity { PhysicalQuantity { value, unit: QuantityUnit::Dimensionless } }

/// 根 `a`、终端 `b`，候选设施 `b`，一个必须归属的分区。
/// Root `a` with terminal `b`, a candidate facility at `b`, and one partition that must be assigned.
fn request() -> SolveRequest { let problem = NetworkStructureProblem {
        design: NetworkDesignProblem {
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
                NetworkDesignLevel { id: "small".into(), capacity: mass(8.0), cost: mass(3.0) }] }],
            facilities: vec![NetworkDesignFacility { node: "b".into(), cost: mass(2.0), required: true,
                capacity_rows: Vec::new() }],
            budget: None,
        },
        root: "a".into(),
        terminals: vec!["b".into()],
        steiner_nodes: Vec::new(),
        partitions: vec![NetworkStructurePartition { id: "p1".into(), eligible_facilities: vec!["b".into()], required: true }],
        quotas: Vec::new(),
    };
    SolveRequest { schedule_log_probe: None, operator_capability_probe: None, hypervolume_probe: None, acceptance_recovery_probe: None, training_split_probe: None, non_stationary_probe: None, selection_policy_probe: None, solve_value_probe: None, native_lowering_probe: None, native_terminal_proof_probe: None, predicate_scope_probe: None, persistence_schema_probe: None, persistence_sort_probe: None, effectiveness_ranking_probe: None, funnel_ranking: None, sensitivity_probe: None, correction_probe: None, blocking_set: None, fingerprint_probe: None, diagnostics_probe: None, gap_probe: None, proof_probe: None, evidence_probe: None, band_probe: None, group_summary: None, critical_report: None, effectiveness_probe: None, funnel: None, solution_status: None, diversity: None, piecewise: None, archive_probe: None, migration: None, topology: None, ranking_probe: None, acceptance: None, boundary: None, population_config: None, constraint_priority: None, constraint_ranking: None, pareto_archive: None, reference_points: None, dispersion: None, dominance: None, evidence_remap: None, tier_selection: None, activation_set: None, container_surface: None, identity_wrap: None, domain_surface: None, expression_bounds: None, assignment_eval: None, constraint_canon: None, snapshot_gate: None, cp_model_build: None, no_good_oracle: None, subproblem_result: None, cp_support_merge: None, capability_matrix: None, analysis_options: None, model_view: None, polynomial_probe: None, matrix_rows: None, sparse_triad: None, master_assignment: None, stable_value: None, quadratic_elastic: None, dump_batch: None, lazy_flatten_init: None, candidate_shortlist: None, writer_registry: None, native_write_scope: None, dump_options: None, assignment_composition: None, cut_validity: None, source_identity: None, binding_assignment: None, auxiliary_domain: None, activity_report_check: None, evidence_predicates: None, activity_bands: None, encoding_label: None, expression_key: None, canonical_monomial: None, backend_features: None, deferred_materialize: None, quadratic_monomial: None, polarity_encoding: None, tier_classification: None, assumption_binding: None, effectiveness_aggregate: None, bounded_no_good: None, polynomial_lift: None, scalar_function: None, field_resolution: None, boolean_fold: None, master_cut: None, flatten_identity: None, critical_assembly: None, variable_kind_domain: None, sort_by: None, predicate_schema: None, search_budget: None, conditional_semantics: None, big_m_policy: None, number_text: None, combine_status: None, elastic_model: None, native_write: None, input_domain: None, function_usage: None, repository_call: None, unsupported_predicate: None, boolean_scope: None, update_assignment: None, flatten_cache: None, cache_key_address: None, cache_key_identity: None, model_configuration: None, predicate_field_filter: None, predicate_field_resolution: None, analysis_group_aggregate: None, master_cut_encoding: None, q_table: None, heuristic_schedule: None, symbol_core_relations: None, symbol_linearization: None, symbol_condition: None, benders_cut_request: None, benders_metrics: None, cp_checkpoint_request: None, lbb_resume_identity: None, linear_mapping_request: None, checkpoint_request: None, reward_policy: None, hyper_heuristic_config: None, criticality_request: None, multi_target_report: None, relaxation_policy: None, target_spec: None, target_report: None, perturbation_policy: None, fixed_integer_report: None, fixed_integer_config: None, relaxation_request: None, critical_analysis: None, effectiveness_ranking: None, shortlist_limit: None, candidate_funnel: None, conflict_explanation: None, remote_artifact: None, logic_based_benders_plan: None, master_solution: None, benders_quadratic_cuts: None, pipeline_steps: None, repository_query: None, assignments: None, sort_plan: None, predicate: None, benders_cuts: None, relational_query_plan: None, linear_lifecycle: None,  indexed_registration: None, column_batch: None, column_scope: None, heart_beat: None, response_envelope: None, quadratic_orchestration: None, vrp_problem: None, network_resilience_problem: None, allocation_problem: None, network_structure_problem: Some(problem), network_design_problem: None,
        network_generalized_flow_problem: None, network_flow_problem: None, sweep_cases: None, incumbent_values: None,
        solution_pool_options: None, gantt_activity_options: None, bpp3d_options: None, protocol_version: 1,
        solver: "scip".into(),
        model: ospf_rust_bridge::encode_model(&ospf_rust_core::model::MetaModel::<f64>::new("network-structure-placeholder")
            .try_to_linear_triad_model().unwrap()).unwrap(),
        tuning_options: None, model_kind: Some(ModelKind::NetworkStructure), time_limit_ms: Some(30_000),
        cp_options: None, serial_options: None, parallel_options: None, fixed_recourse_options: None,
        column_generation_options: None, bpp1d_options: None, bpp2d_options: None, bpp_loading_options: None,
        csp2d_options: None, csp1d_options: None, iis_options: None }
}

#[test]
fn network_structure_entry_rejects_option_mixing_and_unknown_backends() {
    let engine = NativeEngine::default();
    let request = request();
    let mut mixed = request.clone();
    mixed.network_design_problem = Some(ospf_rust_bridge::NetworkDesignProblem { flow: ospf_rust_bridge::NetworkFlowProblem {
        nodes: Vec::new(), arcs: Vec::new(), commodities: Vec::new() }, services: Vec::new(), facilities: Vec::new(), budget: None });
    assert_eq!(engine.submit(mixed).unwrap_err().code, "InvalidInput");
    assert_eq!(engine.submit(SolveRequest { schedule_log_probe: None, operator_capability_probe: None, hypervolume_probe: None, acceptance_recovery_probe: None, training_split_probe: None, non_stationary_probe: None, selection_policy_probe: None, solve_value_probe: None, native_lowering_probe: None, native_terminal_proof_probe: None, predicate_scope_probe: None, persistence_schema_probe: None, persistence_sort_probe: None, effectiveness_ranking_probe: None, funnel_ranking: None, sensitivity_probe: None, correction_probe: None, blocking_set: None, fingerprint_probe: None, diagnostics_probe: None, gap_probe: None, proof_probe: None, evidence_probe: None, band_probe: None, group_summary: None, critical_report: None, effectiveness_probe: None, funnel: None, solution_status: None, diversity: None, piecewise: None, archive_probe: None, migration: None, topology: None, ranking_probe: None, acceptance: None, boundary: None, population_config: None, constraint_priority: None, constraint_ranking: None, pareto_archive: None, reference_points: None, dispersion: None, dominance: None, evidence_remap: None, tier_selection: None, activation_set: None, container_surface: None, identity_wrap: None, domain_surface: None, expression_bounds: None, assignment_eval: None, constraint_canon: None, snapshot_gate: None, cp_model_build: None, no_good_oracle: None, subproblem_result: None, cp_support_merge: None, capability_matrix: None, analysis_options: None, model_view: None, polynomial_probe: None, matrix_rows: None, sparse_triad: None, master_assignment: None, stable_value: None, quadratic_elastic: None, dump_batch: None, lazy_flatten_init: None, candidate_shortlist: None, writer_registry: None, native_write_scope: None, dump_options: None, assignment_composition: None, cut_validity: None, source_identity: None, binding_assignment: None, auxiliary_domain: None, activity_report_check: None, evidence_predicates: None, activity_bands: None, encoding_label: None, expression_key: None, canonical_monomial: None, backend_features: None, deferred_materialize: None, quadratic_monomial: None, polarity_encoding: None, tier_classification: None, assumption_binding: None, effectiveness_aggregate: None, bounded_no_good: None, polynomial_lift: None, scalar_function: None, field_resolution: None, boolean_fold: None, master_cut: None, flatten_identity: None, critical_assembly: None, variable_kind_domain: None, sort_by: None, predicate_schema: None, search_budget: None, conditional_semantics: None, big_m_policy: None, number_text: None, combine_status: None, elastic_model: None, native_write: None, input_domain: None, function_usage: None, repository_call: None, unsupported_predicate: None, boolean_scope: None, update_assignment: None, flatten_cache: None, cache_key_address: None, cache_key_identity: None, model_configuration: None, predicate_field_filter: None, predicate_field_resolution: None, analysis_group_aggregate: None, master_cut_encoding: None, q_table: None, heuristic_schedule: None, symbol_core_relations: None, symbol_linearization: None, symbol_condition: None, benders_cut_request: None, benders_metrics: None, cp_checkpoint_request: None, lbb_resume_identity: None, linear_mapping_request: None, checkpoint_request: None, reward_policy: None, hyper_heuristic_config: None, criticality_request: None, multi_target_report: None, relaxation_policy: None, target_spec: None, target_report: None, perturbation_policy: None, fixed_integer_report: None, fixed_integer_config: None, relaxation_request: None, critical_analysis: None, effectiveness_ranking: None, shortlist_limit: None, candidate_funnel: None, conflict_explanation: None, remote_artifact: None, logic_based_benders_plan: None, master_solution: None, benders_quadratic_cuts: None, pipeline_steps: None, repository_query: None, assignments: None, sort_plan: None, predicate: None, benders_cuts: None, relational_query_plan: None, linear_lifecycle: None,  indexed_registration: None, column_batch: None, column_scope: None, heart_beat: None, response_envelope: None, quadratic_orchestration: None, model_kind: None, ..request.clone() }).unwrap_err().code, "InvalidInput");
    let mut missing = request.clone();
    missing.network_structure_problem = None;
    assert_eq!(engine.submit(missing).unwrap_err().code, "InvalidInput");
    let mut unknown = request;
    unknown.solver = "no-such-backend".into();
    assert_eq!(engine.submit(unknown).unwrap_err().code, "UnsupportedCapability");
}

#[cfg(feature = "scip")]
#[test]
fn network_structure_job_returns_tree_construction_and_ownership_together() {
    let engine = NativeEngine::default();
    let view = engine.submit(request()).unwrap();
    let finished = std::iter::repeat_with(|| { std::thread::sleep(std::time::Duration::from_millis(20)); engine.get(&view.id).unwrap() })
        .take(3000).find(|job| job.status != JobStatus::Running).expect("network structure job never reached a terminal state");
    assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
    let report: ospf_rust_bridge::NetworkStructureReport = serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
    let solution = report.solution.as_ref().expect("a feasible structure must carry a domain solution");
    // 树必须覆盖根与终端，且每个非根活动节点只有一个父弧。
    // The tree must cover the root and the terminal, and every active non-root node has exactly one parent arc.
    assert_eq!(solution.connected_nodes, vec!["a".to_owned(), "b".to_owned()]);
    assert_eq!(solution.selected_tree_arcs, vec!["a-b".to_owned()]);
    assert_eq!(solution.parent_arcs.len(), 1, "{:?}", solution.parent_arcs);
    assert_eq!(solution.parent_arcs[0].node, "b");
    assert_eq!(solution.parent_arcs[0].arc, "a-b");
    // 建设选择与分区归属同时交付。 / Construction choices and partition ownership are delivered together.
    assert_eq!(solution.construction.service_levels.len(), 1);
    assert_eq!(solution.construction.service_levels[0].level, "small");
    assert_eq!(solution.construction.open_facilities, vec!["b".to_owned()]);
    assert_eq!(solution.partition_owners.len(), 1, "{:?}", solution.partition_owners);
    assert_eq!(solution.partition_owners[0].partition, "p1");
    assert_eq!(solution.partition_owners[0].owner, "b");
    assert!(report.summary.values.is_empty());
    engine.release(&view.id).unwrap();
}
