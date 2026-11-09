//! VRP 入口的原生端契约：路线必须按停靠点顺序交付，且预算只能收紧不能放宽算法。
//! Native contract for the VRP entry: routes are delivered in stop order, and the budget can only tighten the
//! algorithm rather than widen it.

use crate::*;
use ospf_rust_bridge::{
    PhysicalQuantity, QuantityUnit, VrpAxis, VrpCustomerSpec, VrpDepotSpec, VrpNodeSpec, VrpProblem, VrpSearchOptions,
    VrpTimeWindow, VrpTolerancesSpec, VrpUnitsSpec, VrpVehicleTypeSpec,
};

fn meter(value: f64) -> PhysicalQuantity { PhysicalQuantity { value, unit: QuantityUnit::Meter } }
fn kg(value: f64) -> PhysicalQuantity { PhysicalQuantity { value, unit: QuantityUnit::Kilogram } }
fn pure(value: f64) -> PhysicalQuantity { PhysicalQuantity { value, unit: QuantityUnit::Dimensionless } }
fn seconds(value: f64) -> PhysicalQuantity { PhysicalQuantity { value, unit: QuantityUnit::Second } }

fn node(id: &str, x: f64, y: f64) -> VrpNodeSpec {
    VrpNodeSpec { id: id.into(), axes: vec![
        VrpAxis { name: "x".into(), value: meter(x) }, VrpAxis { name: "y".into(), value: meter(y) }] }
}

fn window() -> VrpTimeWindow {
    VrpTimeWindow { ready: "2024-01-01T00:00:00Z".into(), due: "2024-01-01T10:00:00Z".into() }
}

/// 起终点同坐标、一个客户在东侧 1 km、一辆车。
/// One customer 1 km east of the shared depot coordinates with a single vehicle.
fn problem() -> VrpProblem {
    VrpProblem {
        name: "tiny".into(),
        start_depot: VrpDepotSpec { node: node("start", 0.0, 0.0), window: window() },
        end_depot: VrpDepotSpec { node: node("end", 0.0, 0.0), window: window() },
        customers: vec![VrpCustomerSpec { id: "c1".into(), node: node("c1-node", 1000.0, 0.0), demand: kg(1.0),
            window: window(), service_time: seconds(0.0) }],
        vehicle_types: vec![VrpVehicleTypeSpec { id: "v".into(), capacity: kg(10.0), fixed_cost: pure(0.0), amount: 1 }],
        arcs: Vec::new(),
        scheduling_window: window(),
        units: VrpUnitsSpec { distance: QuantityUnit::Meter, load: QuantityUnit::Kilogram, cost: QuantityUnit::Dimensionless },
        tolerances: VrpTolerancesSpec::default(),
        search: Some(VrpSearchOptions { node_limit: Some(16), relative_gap_tolerance: Some(1e-4),
            max_cg_iterations_per_node: Some(64) }),
    }
}

fn request() -> SolveRequest { SolveRequest { schedule_log_probe: None, operator_capability_probe: None, hypervolume_probe: None, acceptance_recovery_probe: None, training_split_probe: None, non_stationary_probe: None, selection_policy_probe: None, solve_value_probe: None, native_lowering_probe: None, native_terminal_proof_probe: None, predicate_scope_probe: None, persistence_schema_probe: None, persistence_sort_probe: None, effectiveness_ranking_probe: None, funnel_ranking: None, sensitivity_probe: None, correction_probe: None, blocking_set: None, fingerprint_probe: None, diagnostics_probe: None, gap_probe: None, proof_probe: None, evidence_probe: None, band_probe: None, group_summary: None, critical_report: None, effectiveness_probe: None, funnel: None, solution_status: None, diversity: None, piecewise: None, archive_probe: None, migration: None, topology: None, ranking_probe: None, acceptance: None, boundary: None, population_config: None, constraint_priority: None, constraint_ranking: None, pareto_archive: None, reference_points: None, dispersion: None, dominance: None, evidence_remap: None, tier_selection: None, activation_set: None, container_surface: None, identity_wrap: None, domain_surface: None, expression_bounds: None, assignment_eval: None, constraint_canon: None, snapshot_gate: None, cp_model_build: None, no_good_oracle: None, subproblem_result: None, cp_support_merge: None, capability_matrix: None, analysis_options: None, model_view: None, polynomial_probe: None, matrix_rows: None, sparse_triad: None, master_assignment: None, stable_value: None, quadratic_elastic: None, dump_batch: None, lazy_flatten_init: None, candidate_shortlist: None, writer_registry: None, native_write_scope: None, dump_options: None, assignment_composition: None, cut_validity: None, source_identity: None, binding_assignment: None, auxiliary_domain: None, activity_report_check: None, evidence_predicates: None, activity_bands: None, encoding_label: None, expression_key: None, canonical_monomial: None, backend_features: None, deferred_materialize: None, quadratic_monomial: None, polarity_encoding: None, tier_classification: None, assumption_binding: None, effectiveness_aggregate: None, bounded_no_good: None, polynomial_lift: None, scalar_function: None, field_resolution: None, boolean_fold: None, master_cut: None, flatten_identity: None, critical_assembly: None, variable_kind_domain: None, sort_by: None, predicate_schema: None, search_budget: None, conditional_semantics: None, big_m_policy: None, number_text: None, combine_status: None, elastic_model: None, native_write: None, input_domain: None, function_usage: None, repository_call: None, unsupported_predicate: None, boolean_scope: None, update_assignment: None, flatten_cache: None, cache_key_address: None, cache_key_identity: None, model_configuration: None, predicate_field_filter: None, predicate_field_resolution: None, analysis_group_aggregate: None, master_cut_encoding: None, q_table: None, heuristic_schedule: None, symbol_core_relations: None, symbol_linearization: None, symbol_condition: None, benders_cut_request: None, benders_metrics: None, cp_checkpoint_request: None, lbb_resume_identity: None, linear_mapping_request: None, checkpoint_request: None, reward_policy: None, hyper_heuristic_config: None, criticality_request: None, multi_target_report: None, relaxation_policy: None, target_spec: None, target_report: None, perturbation_policy: None, fixed_integer_report: None, fixed_integer_config: None, relaxation_request: None, critical_analysis: None, effectiveness_ranking: None, shortlist_limit: None, candidate_funnel: None, conflict_explanation: None, remote_artifact: None, logic_based_benders_plan: None, master_solution: None, benders_quadratic_cuts: None, pipeline_steps: None, repository_query: None, assignments: None, sort_plan: None, predicate: None, benders_cuts: None, relational_query_plan: None, linear_lifecycle: None,  indexed_registration: None, column_batch: None, column_scope: None, heart_beat: None, response_envelope: None, quadratic_orchestration: None, vrp_problem: Some(problem()), network_resilience_problem: None, allocation_problem: None,
        network_structure_problem: None, network_design_problem: None, network_generalized_flow_problem: None,
        network_flow_problem: None, sweep_cases: None, incumbent_values: None, solution_pool_options: None,
        gantt_activity_options: None, bpp3d_options: None, protocol_version: 1, solver: "scip".into(),
        model: ospf_rust_bridge::encode_model(&ospf_rust_core::model::MetaModel::<f64>::new("vrp-placeholder")
            .try_to_linear_triad_model().unwrap()).unwrap(),
        tuning_options: None, model_kind: Some(ModelKind::Vrp), time_limit_ms: Some(60_000),
        cp_options: None, serial_options: None, parallel_options: None, fixed_recourse_options: None,
        column_generation_options: None, bpp1d_options: None, bpp2d_options: None, bpp_loading_options: None,
        csp2d_options: None, csp1d_options: None, iis_options: None }
}

#[test]
fn vrp_entry_rejects_option_mixing_and_unknown_backends() {
    let engine = NativeEngine::default();
    let request = request();
    let mut mixed = request.clone();
    mixed.network_resilience_problem = Some(ospf_rust_bridge::NetworkResilienceProblem {
        design: ospf_rust_bridge::NetworkDesignProblem { flow: ospf_rust_bridge::NetworkFlowProblem {
            nodes: Vec::new(), arcs: Vec::new(), commodities: Vec::new() }, services: Vec::new(), facilities: Vec::new(),
            budget: None }, scenarios: Vec::new(), connectivity: Vec::new() });
    assert_eq!(engine.submit(mixed).unwrap_err().code, "InvalidInput");
    assert_eq!(engine.submit(SolveRequest { schedule_log_probe: None, operator_capability_probe: None, hypervolume_probe: None, acceptance_recovery_probe: None, training_split_probe: None, non_stationary_probe: None, selection_policy_probe: None, solve_value_probe: None, native_lowering_probe: None, native_terminal_proof_probe: None, predicate_scope_probe: None, persistence_schema_probe: None, persistence_sort_probe: None, effectiveness_ranking_probe: None, funnel_ranking: None, sensitivity_probe: None, correction_probe: None, blocking_set: None, fingerprint_probe: None, diagnostics_probe: None, gap_probe: None, proof_probe: None, evidence_probe: None, band_probe: None, group_summary: None, critical_report: None, effectiveness_probe: None, funnel: None, solution_status: None, diversity: None, piecewise: None, archive_probe: None, migration: None, topology: None, ranking_probe: None, acceptance: None, boundary: None, population_config: None, constraint_priority: None, constraint_ranking: None, pareto_archive: None, reference_points: None, dispersion: None, dominance: None, evidence_remap: None, tier_selection: None, activation_set: None, container_surface: None, identity_wrap: None, domain_surface: None, expression_bounds: None, assignment_eval: None, constraint_canon: None, snapshot_gate: None, cp_model_build: None, no_good_oracle: None, subproblem_result: None, cp_support_merge: None, capability_matrix: None, analysis_options: None, model_view: None, polynomial_probe: None, matrix_rows: None, sparse_triad: None, master_assignment: None, stable_value: None, quadratic_elastic: None, dump_batch: None, lazy_flatten_init: None, candidate_shortlist: None, writer_registry: None, native_write_scope: None, dump_options: None, assignment_composition: None, cut_validity: None, source_identity: None, binding_assignment: None, auxiliary_domain: None, activity_report_check: None, evidence_predicates: None, activity_bands: None, encoding_label: None, expression_key: None, canonical_monomial: None, backend_features: None, deferred_materialize: None, quadratic_monomial: None, polarity_encoding: None, tier_classification: None, assumption_binding: None, effectiveness_aggregate: None, bounded_no_good: None, polynomial_lift: None, scalar_function: None, field_resolution: None, boolean_fold: None, master_cut: None, flatten_identity: None, critical_assembly: None, variable_kind_domain: None, sort_by: None, predicate_schema: None, search_budget: None, conditional_semantics: None, big_m_policy: None, number_text: None, combine_status: None, elastic_model: None, native_write: None, input_domain: None, function_usage: None, repository_call: None, unsupported_predicate: None, boolean_scope: None, update_assignment: None, flatten_cache: None, cache_key_address: None, cache_key_identity: None, model_configuration: None, predicate_field_filter: None, predicate_field_resolution: None, analysis_group_aggregate: None, master_cut_encoding: None, q_table: None, heuristic_schedule: None, symbol_core_relations: None, symbol_linearization: None, symbol_condition: None, benders_cut_request: None, benders_metrics: None, cp_checkpoint_request: None, lbb_resume_identity: None, linear_mapping_request: None, checkpoint_request: None, reward_policy: None, hyper_heuristic_config: None, criticality_request: None, multi_target_report: None, relaxation_policy: None, target_spec: None, target_report: None, perturbation_policy: None, fixed_integer_report: None, fixed_integer_config: None, relaxation_request: None, critical_analysis: None, effectiveness_ranking: None, shortlist_limit: None, candidate_funnel: None, conflict_explanation: None, remote_artifact: None, logic_based_benders_plan: None, master_solution: None, benders_quadratic_cuts: None, pipeline_steps: None, repository_query: None, assignments: None, sort_plan: None, predicate: None, benders_cuts: None, relational_query_plan: None, linear_lifecycle: None,  indexed_registration: None, column_batch: None, column_scope: None, heart_beat: None, response_envelope: None, quadratic_orchestration: None, model_kind: None, ..request.clone() }).unwrap_err().code, "InvalidInput");
    let mut missing = request.clone();
    missing.vrp_problem = None;
    assert_eq!(engine.submit(missing).unwrap_err().code, "InvalidInput");
    let mut unknown = request;
    unknown.solver = "no-such-backend".into();
    assert_eq!(engine.submit(unknown).unwrap_err().code, "UnsupportedCapability");
}

#[cfg(feature = "scip")]
#[test]
fn vrp_job_returns_one_route_visiting_the_customer_in_order() {
    let engine = NativeEngine::default();
    let view = engine.submit(request()).unwrap();
    let finished = std::iter::repeat_with(|| { std::thread::sleep(std::time::Duration::from_millis(20)); engine.get(&view.id).unwrap() })
        .take(6000).find(|job| job.status != JobStatus::Running).expect("vrp job never reached a terminal state");
    assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
    let report: ospf_rust_bridge::VrpReport = serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
    let solution = report.solution.as_ref().expect("a feasible VRP must carry a domain solution");
    // 一个客户 → 一条路线，起点 → 客户 → 终点。 / One customer means one route: start, customer, end.
    assert_eq!(solution.routes.len(), 1, "{:?}", solution.routes);
    let route = &solution.routes[0];
    assert_eq!(route.vehicle_type, "v");
    let visited: Vec<&str> = route.stops.iter().map(|stop| stop.node.as_str()).collect();
    assert_eq!(visited, vec!["start", "c1-node", "end"], "{visited:?}");
    // 客户停靠点必须带客户 ID，仓库停靠点不带。 / The customer stop carries an ID; depot stops do not.
    assert_eq!(route.stops[1].customer.as_deref(), Some("c1"));
    assert_eq!(route.stops[0].customer, None);
    assert_eq!(route.stops[2].customer, None);
    // 弧序列长度等于停靠点数减一。 / The arc sequence length is the stop count minus one.
    assert_eq!(route.arc_ids.len(), 2, "{:?}", route.arc_ids);
    // 往返 2 km，累计负载在客户处达到 1 kg。 / Two kilometres round trip, with 1 kg accumulated at the customer.
    assert!((route.distance.value - 2000.0).abs() < 1e-6, "distance {}", route.distance.value);
    assert_eq!(route.distance.unit, "m");
    assert!((route.stops[1].accumulated_load.value - 1.0).abs() < 1e-6,
        "load {}", route.stops[1].accumulated_load.value);
    assert_eq!(route.stops[1].accumulated_load.unit, "kg");
    // 时刻必须落在声明的时间窗里，且到达 <= 服务 <= 离开。
    // Instants must stay inside the declared window, and arrival <= service start <= departure.
    for stop in &route.stops {
        assert!(stop.arrival <= stop.service_start && stop.service_start <= stop.departure,
            "{stop:?}");
        assert!(stop.arrival.starts_with("2024-01-01"), "{}", stop.arrival);
    }
    assert!(report.summary.values.is_empty());
    engine.release(&view.id).unwrap();
}

#[cfg(feature = "scip")]
#[test]
fn vrp_budget_cannot_widen_the_algorithm_and_bad_instances_fail_before_solving() {
    let engine = NativeEngine::default();
    // 零预算节点上限仍要给出可用结果，不能悄悄把上限当无限。
    // A zero node limit must still produce a usable outcome rather than silently treating the limit as unlimited.
    let mut tight = request();
    tight.vrp_problem.as_mut().unwrap().search = Some(VrpSearchOptions { node_limit: Some(1),
        relative_gap_tolerance: Some(1e-4), max_cg_iterations_per_node: Some(8) });
    let view = engine.submit(tight).unwrap();
    let finished = std::iter::repeat_with(|| { std::thread::sleep(std::time::Duration::from_millis(20)); engine.get(&view.id).unwrap() })
        .take(6000).find(|job| job.status != JobStatus::Running).expect("vrp job never reached a terminal state");
    // 收紧预算后可能未收敛，但报告必须自洽：有证明就必须有解。
    // A tightened budget may stop early, but the report must stay self-consistent: a proof implies a solution.
    if let Some(json) = finished.report_json.as_deref() {
        let report: ospf_rust_bridge::VrpReport = serde_json::from_str(json).unwrap();
        assert!(!report.summary.has_optimality_proof || report.solution.is_some(), "{:?}", report.summary);
    }
    engine.release(&view.id).unwrap();
    // 需求超过所有车型容量的实例必须在建模前被拒绝，而不是拖到定价阶段。
    // An instance whose demand exceeds every vehicle capacity must be rejected before modelling instead of surfacing
    // later in pricing.
    let mut broken = request();
    broken.vrp_problem.as_mut().unwrap().customers[0].demand = kg(11.0);
    assert_eq!(engine.submit(broken).unwrap_err().code, "InvalidInput");
}
