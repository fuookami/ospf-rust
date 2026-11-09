//! 轴感知圆柱源经正式 native 分派执行真实单轮流程的验收。 / Acceptance of axis-aware cylinder sources through the formal native one-round dispatch.

#![cfg(any(feature = "scip", feature = "gurobi10", feature = "gurobi11", feature = "gurobi12"))]

use std::time::{Duration, Instant};
use ospf_rust_bridge::*;
use ospf_rust_native::NativeEngine;

fn source() -> ModelArtifact {
    encode_bpp3d_problem(serde_json::from_str(r#"{
        "items": [{"id": "drum", "name": "Drum", "width": {"value": 1.0, "unit": "m"}, "height": {"value": 1.0, "unit": "m"},
            "depth": {"value": 1.0, "unit": "m"}, "weight": {"value": 1000.0, "unit": "g"}, "amount": "1",
            "orientations": ["upright"], "shape": {"kind": "cylinder", "axis": "y", "radius": {"value": 0.5, "unit": "m"}}}],
        "bins": [{"id": "bin", "typeCode": "box", "width": {"value": 2.0, "unit": "m"}, "height": {"value": 1.0, "unit": "m"},
            "depth": {"value": 1.0, "unit": "m"}, "capacity": {"value": 1.0, "unit": "kg"}, "isMain": true}],
        "initialLayers": [{"id": "layer", "binId": "bin", "depth": {"value": 1.0, "unit": "m"}, "placements": [
            {"itemId": "drum", "position": {"x": {"value": 0.0, "unit": "m"}, "y": {"value": 0.0, "unit": "m"}, "z": {"value": 0.0, "unit": "m"}},
             "orientation": "upright"}]}]
    }"#).unwrap()).unwrap()
}

fn request(backend: NativeBackend, generators: &[&str]) -> SolveRequest { SolveRequest { schedule_log_probe: None, operator_capability_probe: None, hypervolume_probe: None, acceptance_recovery_probe: None, training_split_probe: None, non_stationary_probe: None, selection_policy_probe: None, solve_value_probe: None, native_lowering_probe: None, native_terminal_proof_probe: None, predicate_scope_probe: None, persistence_schema_probe: None, persistence_sort_probe: None, effectiveness_ranking_probe: None, funnel_ranking: None, sensitivity_probe: None, correction_probe: None, blocking_set: None, fingerprint_probe: None, diagnostics_probe: None, gap_probe: None, proof_probe: None, evidence_probe: None, band_probe: None, group_summary: None, critical_report: None, effectiveness_probe: None, funnel: None, solution_status: None, diversity: None, piecewise: None, archive_probe: None, migration: None, topology: None, ranking_probe: None, acceptance: None, boundary: None, population_config: None, constraint_priority: None, constraint_ranking: None, pareto_archive: None, reference_points: None, dispersion: None, dominance: None, evidence_remap: None, tier_selection: None, activation_set: None, container_surface: None, identity_wrap: None, domain_surface: None, expression_bounds: None, assignment_eval: None, constraint_canon: None, snapshot_gate: None, cp_model_build: None, no_good_oracle: None, subproblem_result: None, cp_support_merge: None, capability_matrix: None, analysis_options: None, model_view: None, polynomial_probe: None, matrix_rows: None, sparse_triad: None, master_assignment: None, stable_value: None, quadratic_elastic: None, dump_batch: None, lazy_flatten_init: None, candidate_shortlist: None, writer_registry: None, native_write_scope: None, dump_options: None, assignment_composition: None, cut_validity: None, source_identity: None, binding_assignment: None, auxiliary_domain: None, activity_report_check: None, evidence_predicates: None, activity_bands: None, encoding_label: None, expression_key: None, canonical_monomial: None, backend_features: None, deferred_materialize: None, quadratic_monomial: None, polarity_encoding: None, tier_classification: None, assumption_binding: None, effectiveness_aggregate: None, bounded_no_good: None, polynomial_lift: None, scalar_function: None, field_resolution: None, boolean_fold: None, master_cut: None, flatten_identity: None, critical_assembly: None, variable_kind_domain: None, sort_by: None, predicate_schema: None, search_budget: None, conditional_semantics: None, big_m_policy: None, number_text: None, combine_status: None, elastic_model: None, native_write: None, input_domain: None, function_usage: None, repository_call: None, unsupported_predicate: None, boolean_scope: None, update_assignment: None, flatten_cache: None, cache_key_address: None, cache_key_identity: None, model_configuration: None, predicate_field_filter: None, predicate_field_resolution: None, analysis_group_aggregate: None, master_cut_encoding: None, q_table: None, heuristic_schedule: None, symbol_core_relations: None, symbol_linearization: None, symbol_condition: None, benders_cut_request: None, benders_metrics: None, cp_checkpoint_request: None, lbb_resume_identity: None, linear_mapping_request: None, checkpoint_request: None, reward_policy: None, hyper_heuristic_config: None, criticality_request: None, multi_target_report: None, relaxation_policy: None, target_spec: None, target_report: None, perturbation_policy: None, fixed_integer_report: None, fixed_integer_config: None, relaxation_request: None, critical_analysis: None, effectiveness_ranking: None, shortlist_limit: None, candidate_funnel: None, conflict_explanation: None, remote_artifact: None, logic_based_benders_plan: None, master_solution: None, benders_quadratic_cuts: None, pipeline_steps: None, repository_query: None, assignments: None, sort_plan: None, predicate: None, benders_cuts: None, relational_query_plan: None, linear_lifecycle: None,  indexed_registration: None, column_batch: None, column_scope: None, heart_beat: None, response_envelope: None, quadratic_orchestration: None, vrp_problem: None, network_resilience_problem: None, allocation_problem: None, network_structure_problem: None, network_design_problem: None, network_generalized_flow_problem: None, network_flow_problem: None, sweep_cases: None, solution_pool_options: None, incumbent_values: None, iis_options: None, protocol_version: 1, solver: "bpp3d".into(), model: source(), model_kind: Some(ModelKind::Bpp3d),
        tuning_options: None,
        time_limit_ms: Some(60_000), cp_options: None, serial_options: None, parallel_options: None, fixed_recourse_options: None,
        column_generation_options: None, bpp1d_options: None, csp1d_options: None, bpp2d_options: None,
        bpp3d_options: Some(Bpp3dOptions { backend, generators: Some(generators.iter().map(|value| match *value { "block" => Bpp3dGeneratorKind::Block, "blLocal" => Bpp3dGeneratorKind::BlLocal,
                "blGlobal" => Bpp3dGeneratorKind::BlGlobal, "circlePacking" => Bpp3dGeneratorKind::CirclePacking, "pattern" => Bpp3dGeneratorKind::Pattern,
                "pile" => Bpp3dGeneratorKind::Pile, other => panic!("unsupported probe generator {other}") }).collect()),
            report_limit: Some(16), max_placements: Some(1000) }),
        bpp_loading_options: None, csp2d_options: None, gantt_activity_options: None }
}

fn contract(backend: NativeBackend) {
    let engine = NativeEngine::new(1);
    let source = source();
    let submitted = engine.submit(request(backend, &["block", "circlePacking", "pile"])).unwrap();
    let deadline = Instant::now() + Duration::from_secs(120);
    let view = loop {
        let view = engine.get(&submitted.id).unwrap();
        if view.status != JobStatus::Running { break view; }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(2));
    };
    assert_eq!(view.status, JobStatus::Completed, "{:?}", view.error);
    let report = decode_bpp3d_execution_report(view.report_json.as_ref().unwrap(), &source).unwrap();
    assert!(!report.stages.is_empty());
    // 圆柱源的真实足迹由原领域几何门禁校验；这里断言交付的装箱确实来自严格回放。
    // A cylinder source's real footprints are checked by the original domain geometry guard; assert the delivered packing came from strict replay.
    let packing = report.packing.as_ref().expect("feasible cylinder source must deliver a replayed packing");
    assert_eq!(packing.source_fingerprint, source.fingerprint);
    let placed = packing.bins.iter().flat_map(|bin| bin.placements.iter()).filter(|placement| placement.item_id == "drum").count();
    assert!(placed >= 1);
    engine.release(&submitted.id).unwrap();
}

#[cfg(feature = "scip")]
#[test]
fn real_scip_replays_axis_aware_cylinder_sources() { contract(NativeBackend::Scip); }

#[cfg(any(feature = "gurobi10", feature = "gurobi11", feature = "gurobi12"))]
#[test]
fn real_gurobi_replays_axis_aware_cylinder_sources() { contract(NativeBackend::Gurobi); }
