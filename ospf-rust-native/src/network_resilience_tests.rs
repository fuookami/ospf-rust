//! 韧性入口的原生端契约：每个显式场景独立交付结果，且结论不超出显式场景。
//! Native contract for the resilience entry: every explicit scenario delivers its own result, and conclusions never
//! reach beyond the explicit scenarios.

use crate::*;
use ospf_rust_bridge::{
    NetworkDesignLevel, NetworkDesignProblem, NetworkDesignService, NetworkFlowArc, NetworkFlowCommodity, NetworkFlowNode,
    NetworkFlowNodeQuantity, NetworkResilienceProblem, PhysicalQuantity, QuantityUnit, ResilienceConnectivity,
    ResilienceDisjointness, ResilienceScenario,
};

fn kg(value: f64) -> PhysicalQuantity { PhysicalQuantity { value, unit: QuantityUnit::Kilogram } }
fn pure(value: f64) -> PhysicalQuantity { PhysicalQuantity { value, unit: QuantityUnit::Dimensionless } }

/// 两条平行弧 `a-b-1`、`a-b-2`：关掉一条仍然可达，这样场景才可行。
/// Two parallel arcs `a-b-1` and `a-b-2`; taking one down still leaves a route, so the scenario stays feasible.
fn request() -> SolveRequest { let problem = NetworkResilienceProblem {
        design: NetworkDesignProblem {
            flow: ospf_rust_bridge::NetworkFlowProblem {
                nodes: vec![NetworkFlowNode { id: "a".into() }, NetworkFlowNode { id: "b".into() }],
                arcs: vec![
                    NetworkFlowArc { id: "a-b-1".into(), from: "a".into(), to: "b".into(), capacity_lower: kg(0.0),
                        capacity_upper: kg(10.0), cost: pure(1.0) },
                    NetworkFlowArc { id: "a-b-2".into(), from: "a".into(), to: "b".into(), capacity_lower: kg(0.0),
                        capacity_upper: kg(10.0), cost: pure(1.0) }],
                commodities: vec![NetworkFlowCommodity { id: "c".into(),
                    balances: vec![NetworkFlowNodeQuantity { node: "a".into(), quantity: kg(5.0) },
                        NetworkFlowNodeQuantity { node: "b".into(), quantity: kg(-5.0) }],
                    unit: None, allowed_arcs: None, costs: Vec::new(), capacity_consumption: Vec::new(), integer_step: None }],
            },
            services: Vec::new(),
            facilities: Vec::new(),
            budget: None,
        },
        scenarios: vec![ResilienceScenario { id: "arc-1-down".into(), closed_arcs: vec!["a-b-1".into()],
            failed_nodes: Vec::new(), upper_capacities: Vec::new(), balances: Vec::new(), slacks: Vec::new() }],
        connectivity: vec![ResilienceConnectivity { source: "a".into(), target: "b".into(), paths: 1,
            kind: ResilienceDisjointness::EdgeDisjoint }],
    };
    SolveRequest { schedule_log_probe: None, operator_capability_probe: None, hypervolume_probe: None, acceptance_recovery_probe: None, training_split_probe: None, non_stationary_probe: None, selection_policy_probe: None, solve_value_probe: None, native_lowering_probe: None, native_terminal_proof_probe: None, predicate_scope_probe: None, persistence_schema_probe: None, persistence_sort_probe: None, effectiveness_ranking_probe: None, funnel_ranking: None, sensitivity_probe: None, correction_probe: None, blocking_set: None, fingerprint_probe: None, diagnostics_probe: None, gap_probe: None, proof_probe: None, evidence_probe: None, band_probe: None, group_summary: None, critical_report: None, effectiveness_probe: None, funnel: None, solution_status: None, diversity: None, piecewise: None, archive_probe: None, migration: None, topology: None, ranking_probe: None, acceptance: None, boundary: None, population_config: None, constraint_priority: None, constraint_ranking: None, pareto_archive: None, reference_points: None, dispersion: None, dominance: None, evidence_remap: None, tier_selection: None, activation_set: None, container_surface: None, identity_wrap: None, domain_surface: None, expression_bounds: None, assignment_eval: None, constraint_canon: None, snapshot_gate: None, cp_model_build: None, no_good_oracle: None, subproblem_result: None, cp_support_merge: None, capability_matrix: None, analysis_options: None, model_view: None, polynomial_probe: None, matrix_rows: None, sparse_triad: None, master_assignment: None, stable_value: None, quadratic_elastic: None, dump_batch: None, lazy_flatten_init: None, candidate_shortlist: None, writer_registry: None, native_write_scope: None, dump_options: None, assignment_composition: None, cut_validity: None, source_identity: None, binding_assignment: None, auxiliary_domain: None, activity_report_check: None, evidence_predicates: None, activity_bands: None, encoding_label: None, expression_key: None, canonical_monomial: None, backend_features: None, deferred_materialize: None, quadratic_monomial: None, polarity_encoding: None, tier_classification: None, assumption_binding: None, effectiveness_aggregate: None, bounded_no_good: None, polynomial_lift: None, scalar_function: None, field_resolution: None, boolean_fold: None, master_cut: None, flatten_identity: None, critical_assembly: None, variable_kind_domain: None, sort_by: None, predicate_schema: None, search_budget: None, conditional_semantics: None, big_m_policy: None, number_text: None, combine_status: None, elastic_model: None, native_write: None, input_domain: None, function_usage: None, repository_call: None, unsupported_predicate: None, boolean_scope: None, update_assignment: None, flatten_cache: None, cache_key_address: None, cache_key_identity: None, model_configuration: None, predicate_field_filter: None, predicate_field_resolution: None, analysis_group_aggregate: None, master_cut_encoding: None, q_table: None, heuristic_schedule: None, symbol_core_relations: None, symbol_linearization: None, symbol_condition: None, benders_cut_request: None, benders_metrics: None, cp_checkpoint_request: None, lbb_resume_identity: None, linear_mapping_request: None, checkpoint_request: None, reward_policy: None, hyper_heuristic_config: None, criticality_request: None, multi_target_report: None, relaxation_policy: None, target_spec: None, target_report: None, perturbation_policy: None, fixed_integer_report: None, fixed_integer_config: None, relaxation_request: None, critical_analysis: None, effectiveness_ranking: None, shortlist_limit: None, candidate_funnel: None, conflict_explanation: None, remote_artifact: None, logic_based_benders_plan: None, master_solution: None, benders_quadratic_cuts: None, pipeline_steps: None, repository_query: None, assignments: None, sort_plan: None, predicate: None, benders_cuts: None, relational_query_plan: None, linear_lifecycle: None,  indexed_registration: None, column_batch: None, column_scope: None, heart_beat: None, response_envelope: None, quadratic_orchestration: None, vrp_problem: None, network_resilience_problem: Some(problem), allocation_problem: None, network_structure_problem: None,
        network_design_problem: None, network_generalized_flow_problem: None, network_flow_problem: None, sweep_cases: None,
        incumbent_values: None, solution_pool_options: None, gantt_activity_options: None, bpp3d_options: None,
        protocol_version: 1, solver: "scip".into(),
        model: ospf_rust_bridge::encode_model(&ospf_rust_core::model::MetaModel::<f64>::new("network-resilience-placeholder")
            .try_to_linear_triad_model().unwrap()).unwrap(),
        tuning_options: None, model_kind: Some(ModelKind::NetworkResilience), time_limit_ms: Some(30_000),
        cp_options: None, serial_options: None, parallel_options: None, fixed_recourse_options: None,
        column_generation_options: None, bpp1d_options: None, bpp2d_options: None, bpp_loading_options: None,
        csp2d_options: None, csp1d_options: None, iis_options: None }
}

#[test]
fn resilience_entry_rejects_option_mixing_and_unknown_backends() {
    let engine = NativeEngine::default();
    let request = request();
    let mut mixed = request.clone();
    mixed.allocation_problem = Some(ospf_rust_bridge::AllocationProblem { locations: Vec::new(), services: Vec::new(),
        requests: Vec::new(), options: ospf_rust_bridge::AllocationOptionsSpec { capacity_unit: QuantityUnit::Kilogram,
            cost_unit: QuantityUnit::Kilogram, quotas: Vec::new(), hubs: Vec::new(), service_usage: Vec::new(), budget: None } });
    assert_eq!(engine.submit(mixed).unwrap_err().code, "InvalidInput");
    assert_eq!(engine.submit(SolveRequest { schedule_log_probe: None, operator_capability_probe: None, hypervolume_probe: None, acceptance_recovery_probe: None, training_split_probe: None, non_stationary_probe: None, selection_policy_probe: None, solve_value_probe: None, native_lowering_probe: None, native_terminal_proof_probe: None, predicate_scope_probe: None, persistence_schema_probe: None, persistence_sort_probe: None, effectiveness_ranking_probe: None, funnel_ranking: None, sensitivity_probe: None, correction_probe: None, blocking_set: None, fingerprint_probe: None, diagnostics_probe: None, gap_probe: None, proof_probe: None, evidence_probe: None, band_probe: None, group_summary: None, critical_report: None, effectiveness_probe: None, funnel: None, solution_status: None, diversity: None, piecewise: None, archive_probe: None, migration: None, topology: None, ranking_probe: None, acceptance: None, boundary: None, population_config: None, constraint_priority: None, constraint_ranking: None, pareto_archive: None, reference_points: None, dispersion: None, dominance: None, evidence_remap: None, tier_selection: None, activation_set: None, container_surface: None, identity_wrap: None, domain_surface: None, expression_bounds: None, assignment_eval: None, constraint_canon: None, snapshot_gate: None, cp_model_build: None, no_good_oracle: None, subproblem_result: None, cp_support_merge: None, capability_matrix: None, analysis_options: None, model_view: None, polynomial_probe: None, matrix_rows: None, sparse_triad: None, master_assignment: None, stable_value: None, quadratic_elastic: None, dump_batch: None, lazy_flatten_init: None, candidate_shortlist: None, writer_registry: None, native_write_scope: None, dump_options: None, assignment_composition: None, cut_validity: None, source_identity: None, binding_assignment: None, auxiliary_domain: None, activity_report_check: None, evidence_predicates: None, activity_bands: None, encoding_label: None, expression_key: None, canonical_monomial: None, backend_features: None, deferred_materialize: None, quadratic_monomial: None, polarity_encoding: None, tier_classification: None, assumption_binding: None, effectiveness_aggregate: None, bounded_no_good: None, polynomial_lift: None, scalar_function: None, field_resolution: None, boolean_fold: None, master_cut: None, flatten_identity: None, critical_assembly: None, variable_kind_domain: None, sort_by: None, predicate_schema: None, search_budget: None, conditional_semantics: None, big_m_policy: None, number_text: None, combine_status: None, elastic_model: None, native_write: None, input_domain: None, function_usage: None, repository_call: None, unsupported_predicate: None, boolean_scope: None, update_assignment: None, flatten_cache: None, cache_key_address: None, cache_key_identity: None, model_configuration: None, predicate_field_filter: None, predicate_field_resolution: None, analysis_group_aggregate: None, master_cut_encoding: None, q_table: None, heuristic_schedule: None, symbol_core_relations: None, symbol_linearization: None, symbol_condition: None, benders_cut_request: None, benders_metrics: None, cp_checkpoint_request: None, lbb_resume_identity: None, linear_mapping_request: None, checkpoint_request: None, reward_policy: None, hyper_heuristic_config: None, criticality_request: None, multi_target_report: None, relaxation_policy: None, target_spec: None, target_report: None, perturbation_policy: None, fixed_integer_report: None, fixed_integer_config: None, relaxation_request: None, critical_analysis: None, effectiveness_ranking: None, shortlist_limit: None, candidate_funnel: None, conflict_explanation: None, remote_artifact: None, logic_based_benders_plan: None, master_solution: None, benders_quadratic_cuts: None, pipeline_steps: None, repository_query: None, assignments: None, sort_plan: None, predicate: None, benders_cuts: None, relational_query_plan: None, linear_lifecycle: None,  indexed_registration: None, column_batch: None, column_scope: None, heart_beat: None, response_envelope: None, quadratic_orchestration: None, model_kind: None, ..request.clone() }).unwrap_err().code, "InvalidInput");
    let mut missing = request.clone();
    missing.network_resilience_problem = None;
    assert_eq!(engine.submit(missing).unwrap_err().code, "InvalidInput");
    let mut unknown = request;
    unknown.solver = "no-such-backend".into();
    assert_eq!(engine.submit(unknown).unwrap_err().code, "UnsupportedCapability");
}

#[cfg(feature = "scip")]
#[test]
fn resilience_job_reports_nominal_and_explicit_scenarios_separately() {
    let engine = NativeEngine::default();
    let view = engine.submit(request()).unwrap();
    let finished = std::iter::repeat_with(|| { std::thread::sleep(std::time::Duration::from_millis(20)); engine.get(&view.id).unwrap() })
        .take(3000).find(|job| job.status != JobStatus::Running).expect("resilience job never reached a terminal state");
    assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
    let report: ospf_rust_bridge::NetworkResilienceReport = serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
    let solution = report.solution.as_ref().expect("a feasible resilience plan must carry a domain solution");
    // 场景集合必须恰好是 nominal 加声明的那个，不能外推出未声明的保障。
    // The scenario set must be exactly `nominal` plus the declared one; nothing beyond explicit scenarios is claimed.
    let ids: Vec<&str> = solution.scenarios.iter().map(|scenario| scenario.id.as_str()).collect();
    assert_eq!(ids.len(), 2, "{ids:?}");
    assert!(ids.contains(&"nominal"), "{ids:?}");
    assert!(ids.contains(&"arc-1-down"), "{ids:?}");
    // 每个场景都在剩余弧上完成 5 kg 运输。 / Every scenario ships the 5 kg over its remaining arcs.
    for scenario in &solution.scenarios {
        let shipped: f64 = scenario.flow.flows.iter().map(|flow| flow.quantity.value).sum();
        assert!((shipped - 5.0).abs() < 1e-6, "scenario {} shipped {shipped}", scenario.id);
        if scenario.id == "arc-1-down" {
            // 被关闭的弧不能出现在该场景的流量里。 / A closed arc must not appear in that scenario's flows.
            assert!(scenario.flow.flows.iter().all(|flow| flow.arc != "a-b-1"), "{:?}", scenario.flow.flows);
        }
    }
    // 最坏场景必须来自实际结果，且总费用 = 建设费 + 最坏运行费。
    // The worst scenario must come from the realized results, and the total is construction plus worst operating cost.
    assert!(ids.contains(&solution.worst_scenario.as_str()), "worst {} not in {ids:?}", solution.worst_scenario);
    let construction = solution.construction.cost.value;
    assert!((solution.total_cost.value - (construction + solution.worst_operating_cost.value)).abs() < 1e-6,
        "total {} vs construction {construction} + worst {}", solution.total_cost.value, solution.worst_operating_cost.value);
    // 一条连通要求的辅助流必须给出。 / One connectivity requirement must produce one auxiliary flow.
    assert_eq!(solution.connectivity_flows.len(), 1, "{:?}", solution.connectivity_flows);
    engine.release(&view.id).unwrap();
}
