//! 分配入口的原生端契约：运输量按请求-服务对交付，共享配额与枢纽建设一起生效。
//! Native contract for the allocation entry: shipped amounts are delivered per request-service pair, and shared quotas
//! together with hub construction take effect.

use crate::*;
use ospf_rust_bridge::{
    AllocationFlowQuantity, AllocationLocationSpec, AllocationOptionsSpec, AllocationProblem, AllocationQuotaSpec,
    AllocationRequestSpec, AllocationServiceSpec, PhysicalQuantity, QuantityUnit,
};

fn kg(value: f64) -> PhysicalQuantity { PhysicalQuantity { value, unit: QuantityUnit::Kilogram } }
fn pure(value: f64) -> PhysicalQuantity { PhysicalQuantity { value, unit: QuantityUnit::Dimensionless } }

/// `a` 到 `b` 运 5 kg；服务容量上限 10 kg；共享配额上限 50 kg。
/// Ship 5 kg from `a` to `b`; the service capacity is capped at 10 kg and the shared quota at 50 kg.
fn request(quota_upper: f64) -> SolveRequest { let problem = AllocationProblem {
        locations: vec![AllocationLocationSpec { id: "a".into(), layer: None, attributes: Vec::new() },
            AllocationLocationSpec { id: "b".into(), layer: None, attributes: Vec::new() }],
        services: vec![AllocationServiceSpec { id: "a-b".into(), from: "a".into(), to: "b".into(),
            // 服务成本是「每容量单位」的比值：capacityUnit 与 costUnit 同量纲时无量纲。
            // A service cost is per capacity unit, so it is dimensionless when capacityUnit and costUnit share a
            // dimension.
            cost: pure(1.0), capacity_lower: kg(0.0), capacity_upper: kg(10.0), capacity_levels: Vec::new(),
            required: false, attributes: Vec::new() }],
        requests: vec![AllocationRequestSpec { id: "r".into(), origin: "a".into(), destination: "b".into(),
            quantity: kg(5.0), unit: None, integer_step: None, allowed_services: None, costs: Vec::new() }],
        options: AllocationOptionsSpec { capacity_unit: QuantityUnit::Kilogram, cost_unit: QuantityUnit::Kilogram,
            quotas: vec![AllocationQuotaSpec { id: "q".into(), lower: kg(0.0), upper: kg(quota_upper),
                // 配额用量是「每请求单位」的比值：请求单位就是 kg 时用量无量纲。
                // Quota usage is per request unit, so with a kg request unit the usage is dimensionless.
                usage: vec![AllocationFlowQuantity { request: "r".into(), service: "a-b".into(), quantity: pure(1.0) }] }],
            hubs: Vec::new(), service_usage: Vec::new(), budget: None },
    };
    SolveRequest { schedule_log_probe: None, operator_capability_probe: None, hypervolume_probe: None, acceptance_recovery_probe: None, training_split_probe: None, non_stationary_probe: None, selection_policy_probe: None, solve_value_probe: None, native_lowering_probe: None, native_terminal_proof_probe: None, predicate_scope_probe: None, persistence_schema_probe: None, persistence_sort_probe: None, effectiveness_ranking_probe: None, funnel_ranking: None, sensitivity_probe: None, correction_probe: None, blocking_set: None, fingerprint_probe: None, diagnostics_probe: None, gap_probe: None, proof_probe: None, evidence_probe: None, band_probe: None, group_summary: None, critical_report: None, effectiveness_probe: None, funnel: None, solution_status: None, diversity: None, piecewise: None, archive_probe: None, migration: None, topology: None, ranking_probe: None, acceptance: None, boundary: None, population_config: None, constraint_priority: None, constraint_ranking: None, pareto_archive: None, reference_points: None, dispersion: None, dominance: None, evidence_remap: None, tier_selection: None, activation_set: None, container_surface: None, identity_wrap: None, domain_surface: None, expression_bounds: None, assignment_eval: None, constraint_canon: None, snapshot_gate: None, cp_model_build: None, no_good_oracle: None, subproblem_result: None, cp_support_merge: None, capability_matrix: None, analysis_options: None, model_view: None, polynomial_probe: None, matrix_rows: None, sparse_triad: None, master_assignment: None, stable_value: None, quadratic_elastic: None, dump_batch: None, lazy_flatten_init: None, candidate_shortlist: None, writer_registry: None, native_write_scope: None, dump_options: None, assignment_composition: None, cut_validity: None, source_identity: None, binding_assignment: None, auxiliary_domain: None, activity_report_check: None, evidence_predicates: None, activity_bands: None, encoding_label: None, expression_key: None, canonical_monomial: None, backend_features: None, deferred_materialize: None, quadratic_monomial: None, polarity_encoding: None, tier_classification: None, assumption_binding: None, effectiveness_aggregate: None, bounded_no_good: None, polynomial_lift: None, scalar_function: None, field_resolution: None, boolean_fold: None, master_cut: None, flatten_identity: None, critical_assembly: None, variable_kind_domain: None, sort_by: None, predicate_schema: None, search_budget: None, conditional_semantics: None, big_m_policy: None, number_text: None, combine_status: None, elastic_model: None, native_write: None, input_domain: None, function_usage: None, repository_call: None, unsupported_predicate: None, boolean_scope: None, update_assignment: None, flatten_cache: None, cache_key_address: None, cache_key_identity: None, model_configuration: None, predicate_field_filter: None, predicate_field_resolution: None, analysis_group_aggregate: None, master_cut_encoding: None, q_table: None, heuristic_schedule: None, symbol_core_relations: None, symbol_linearization: None, symbol_condition: None, benders_cut_request: None, benders_metrics: None, cp_checkpoint_request: None, lbb_resume_identity: None, linear_mapping_request: None, checkpoint_request: None, reward_policy: None, hyper_heuristic_config: None, criticality_request: None, multi_target_report: None, relaxation_policy: None, target_spec: None, target_report: None, perturbation_policy: None, fixed_integer_report: None, fixed_integer_config: None, relaxation_request: None, critical_analysis: None, effectiveness_ranking: None, shortlist_limit: None, candidate_funnel: None, conflict_explanation: None, remote_artifact: None, logic_based_benders_plan: None, master_solution: None, benders_quadratic_cuts: None, pipeline_steps: None, repository_query: None, assignments: None, sort_plan: None, predicate: None, benders_cuts: None, relational_query_plan: None, linear_lifecycle: None,  indexed_registration: None, column_batch: None, column_scope: None, heart_beat: None, response_envelope: None, quadratic_orchestration: None, vrp_problem: None, network_resilience_problem: None, allocation_problem: Some(problem), network_structure_problem: None, network_design_problem: None,
        network_generalized_flow_problem: None, network_flow_problem: None, sweep_cases: None, incumbent_values: None,
        solution_pool_options: None, gantt_activity_options: None, bpp3d_options: None, protocol_version: 1,
        solver: "scip".into(),
        model: ospf_rust_bridge::encode_model(&ospf_rust_core::model::MetaModel::<f64>::new("network-allocation-placeholder")
            .try_to_linear_triad_model().unwrap()).unwrap(),
        tuning_options: None, model_kind: Some(ModelKind::NetworkAllocation), time_limit_ms: Some(30_000),
        cp_options: None, serial_options: None, parallel_options: None, fixed_recourse_options: None,
        column_generation_options: None, bpp1d_options: None, bpp2d_options: None, bpp_loading_options: None,
        csp2d_options: None, csp1d_options: None, iis_options: None }
}

#[test]
fn allocation_entry_rejects_option_mixing_and_unknown_backends() {
    let engine = NativeEngine::default();
    let request = request(50.0);
    let mut mixed = request.clone();
    mixed.network_flow_problem = Some(ospf_rust_bridge::NetworkFlowProblem { nodes: Vec::new(), arcs: Vec::new(), commodities: Vec::new() });
    assert_eq!(engine.submit(mixed).unwrap_err().code, "InvalidInput");
    assert_eq!(engine.submit(SolveRequest { schedule_log_probe: None, operator_capability_probe: None, hypervolume_probe: None, acceptance_recovery_probe: None, training_split_probe: None, non_stationary_probe: None, selection_policy_probe: None, solve_value_probe: None, native_lowering_probe: None, native_terminal_proof_probe: None, predicate_scope_probe: None, persistence_schema_probe: None, persistence_sort_probe: None, effectiveness_ranking_probe: None, funnel_ranking: None, sensitivity_probe: None, correction_probe: None, blocking_set: None, fingerprint_probe: None, diagnostics_probe: None, gap_probe: None, proof_probe: None, evidence_probe: None, band_probe: None, group_summary: None, critical_report: None, effectiveness_probe: None, funnel: None, solution_status: None, diversity: None, piecewise: None, archive_probe: None, migration: None, topology: None, ranking_probe: None, acceptance: None, boundary: None, population_config: None, constraint_priority: None, constraint_ranking: None, pareto_archive: None, reference_points: None, dispersion: None, dominance: None, evidence_remap: None, tier_selection: None, activation_set: None, container_surface: None, identity_wrap: None, domain_surface: None, expression_bounds: None, assignment_eval: None, constraint_canon: None, snapshot_gate: None, cp_model_build: None, no_good_oracle: None, subproblem_result: None, cp_support_merge: None, capability_matrix: None, analysis_options: None, model_view: None, polynomial_probe: None, matrix_rows: None, sparse_triad: None, master_assignment: None, stable_value: None, quadratic_elastic: None, dump_batch: None, lazy_flatten_init: None, candidate_shortlist: None, writer_registry: None, native_write_scope: None, dump_options: None, assignment_composition: None, cut_validity: None, source_identity: None, binding_assignment: None, auxiliary_domain: None, activity_report_check: None, evidence_predicates: None, activity_bands: None, encoding_label: None, expression_key: None, canonical_monomial: None, backend_features: None, deferred_materialize: None, quadratic_monomial: None, polarity_encoding: None, tier_classification: None, assumption_binding: None, effectiveness_aggregate: None, bounded_no_good: None, polynomial_lift: None, scalar_function: None, field_resolution: None, boolean_fold: None, master_cut: None, flatten_identity: None, critical_assembly: None, variable_kind_domain: None, sort_by: None, predicate_schema: None, search_budget: None, conditional_semantics: None, big_m_policy: None, number_text: None, combine_status: None, elastic_model: None, native_write: None, input_domain: None, function_usage: None, repository_call: None, unsupported_predicate: None, boolean_scope: None, update_assignment: None, flatten_cache: None, cache_key_address: None, cache_key_identity: None, model_configuration: None, predicate_field_filter: None, predicate_field_resolution: None, analysis_group_aggregate: None, master_cut_encoding: None, q_table: None, heuristic_schedule: None, symbol_core_relations: None, symbol_linearization: None, symbol_condition: None, benders_cut_request: None, benders_metrics: None, cp_checkpoint_request: None, lbb_resume_identity: None, linear_mapping_request: None, checkpoint_request: None, reward_policy: None, hyper_heuristic_config: None, criticality_request: None, multi_target_report: None, relaxation_policy: None, target_spec: None, target_report: None, perturbation_policy: None, fixed_integer_report: None, fixed_integer_config: None, relaxation_request: None, critical_analysis: None, effectiveness_ranking: None, shortlist_limit: None, candidate_funnel: None, conflict_explanation: None, remote_artifact: None, logic_based_benders_plan: None, master_solution: None, benders_quadratic_cuts: None, pipeline_steps: None, repository_query: None, assignments: None, sort_plan: None, predicate: None, benders_cuts: None, relational_query_plan: None, linear_lifecycle: None,  indexed_registration: None, column_batch: None, column_scope: None, heart_beat: None, response_envelope: None, quadratic_orchestration: None, model_kind: None, ..request.clone() }).unwrap_err().code, "InvalidInput");
    let mut missing = request.clone();
    missing.allocation_problem = None;
    assert_eq!(engine.submit(missing).unwrap_err().code, "InvalidInput");
    let mut unknown = request;
    unknown.solver = "no-such-backend".into();
    assert_eq!(engine.submit(unknown).unwrap_err().code, "UnsupportedCapability");
}

#[cfg(feature = "scip")]
#[test]
fn allocation_job_ships_the_requested_amount_through_the_service() {
    let engine = NativeEngine::default();
    let view = engine.submit(request(50.0)).unwrap();
    let finished = std::iter::repeat_with(|| { std::thread::sleep(std::time::Duration::from_millis(20)); engine.get(&view.id).unwrap() })
        .take(3000).find(|job| job.status != JobStatus::Running).expect("allocation job never reached a terminal state");
    assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
    let report: ospf_rust_bridge::AllocationReport = serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
    let solution = report.solution.as_ref().expect("a feasible allocation must carry a domain solution");
    // 运输量按请求-服务对给出，且等于请求量。 / The shipped amount is per request-service pair and equals the request.
    assert_eq!(solution.shipments.len(), 1, "{:?}", solution.shipments);
    assert_eq!(solution.shipments[0].request, "r");
    assert_eq!(solution.shipments[0].service, "a-b");
    assert!((solution.shipments[0].quantity.value - 5.0).abs() < 1e-6,
        "shipped {}", solution.shipments[0].quantity.value);
    assert_eq!(solution.shipments[0].quantity.unit, "kg");
    // 底层流结果同样按领域结构交付。 / The underlying flow is delivered in domain structure too.
    assert_eq!(solution.flow.flows.len(), 1, "{:?}", solution.flow.flows);
    assert!(report.summary.values.is_empty());
    engine.release(&view.id).unwrap();
}

#[cfg(feature = "scip")]
#[test]
fn an_infeasible_quota_fails_instead_of_dropping_the_shipment() {
    let engine = NativeEngine::default();
    // 配额上限只有 1 kg，无法承载 5 kg 请求：必须失败，而不是把请求悄悄丢掉。
    // The quota allows only 1 kg against a 5 kg request: this must fail rather than silently dropping the request.
    let view = engine.submit(request(1.0)).unwrap();
    let finished = std::iter::repeat_with(|| { std::thread::sleep(std::time::Duration::from_millis(20)); engine.get(&view.id).unwrap() })
        .take(3000).find(|job| job.status != JobStatus::Running).expect("allocation job never reached a terminal state");
    assert_ne!(finished.status, JobStatus::Running);
    // 领域服务把不可行保留为报告（无 incumbent）或失败，两种都不能出现"有解但少了请求"。
    // The domain service either preserves an infeasible report (no incumbent) or fails; neither may report a solution
    // that silently misses the request.
    if finished.status == JobStatus::Completed {
        let report: ospf_rust_bridge::AllocationReport = serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        assert!(!report.summary.has_optimality_proof, "an infeasible quota must never carry an optimality proof");
        assert!(report.solution.is_none(), "an infeasible quota must not deliver a solution");
    }
    engine.release(&view.id).unwrap();
}
