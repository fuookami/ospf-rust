//! 二次组合编排的原生端契约：编排只决定串行/并行与选结果策略，不改变任何后端语义。
//! Native contract for quadratic orchestration: it only decides serial/parallel and the result-selection policy; it
//! never changes any backend's semantics.

use crate::*;
use ospf_rust_bridge::{
    NativeBackend, ParallelCombinatorialMode, QuadraticOrchestration, QuadraticOrchestrationOptions, SerialFallbackPolicy,
};
use ospf_rust_core::model::intermediate::QuadraticTetradModel;
use ospf_rust_core::model::{ConstraintRelation, MetaModel, Quadratic, QuadraticInequality, QuadraticMonomial};
use ospf_rust_core::variable::{ContinuousVariableItem, VariableRange};

/// 最小二次模型：最小化 `x`，`x^2 <= 4`，`0 <= x <= 5`，最优 `x = 0`。
/// A minimal quadratic model: minimise `x` subject to `x^2 <= 4` and `0 <= x <= 5`, with optimum `x = 0`.
fn quadratic_model() -> QuadraticTetradModel {
    let mut source = MetaModel::<f64>::new("quadratic-orchestration");
    source.add_variable(ContinuousVariableItem::auto_with_range("x", VariableRange::new(Some(0.0), Some(5.0)))).unwrap();
    let mut model = source.try_to_quadratic_tetrad_model().unwrap();
    model.add_quadratic_constraint_with_metadata(
        QuadraticInequality::new(Quadratic::new(vec![QuadraticMonomial::new_quadratic(1.0, 0, 0)], 0.0),
            ConstraintRelation::LessEqual, 4.0),
        "ball".into(), None, false, 0, None, None);
    model
}

fn request(configuration: QuadraticOrchestrationOptions) -> SolveRequest { let model = quadratic_model();
    SolveRequest { schedule_log_probe: None, operator_capability_probe: None, hypervolume_probe: None, acceptance_recovery_probe: None, training_split_probe: None, non_stationary_probe: None, selection_policy_probe: None, solve_value_probe: None, native_lowering_probe: None, native_terminal_proof_probe: None, predicate_scope_probe: None, persistence_schema_probe: None, persistence_sort_probe: None, effectiveness_ranking_probe: None, funnel_ranking: None, sensitivity_probe: None, correction_probe: None, blocking_set: None, fingerprint_probe: None, diagnostics_probe: None, gap_probe: None, proof_probe: None, evidence_probe: None, band_probe: None, group_summary: None, critical_report: None, effectiveness_probe: None, funnel: None, solution_status: None, diversity: None, piecewise: None, archive_probe: None, migration: None, topology: None, ranking_probe: None, acceptance: None, boundary: None, population_config: None, constraint_priority: None, constraint_ranking: None, pareto_archive: None, reference_points: None, dispersion: None, dominance: None, evidence_remap: None, tier_selection: None, activation_set: None, container_surface: None, identity_wrap: None, domain_surface: None, expression_bounds: None, assignment_eval: None, constraint_canon: None, snapshot_gate: None, cp_model_build: None, no_good_oracle: None, subproblem_result: None, cp_support_merge: None, capability_matrix: None, analysis_options: None, model_view: None, polynomial_probe: None, matrix_rows: None, sparse_triad: None, master_assignment: None, stable_value: None, quadratic_elastic: None, dump_batch: None, lazy_flatten_init: None, candidate_shortlist: None, writer_registry: None, native_write_scope: None, dump_options: None, assignment_composition: None, cut_validity: None, source_identity: None, binding_assignment: None, auxiliary_domain: None, activity_report_check: None, evidence_predicates: None, activity_bands: None, encoding_label: None, expression_key: None, canonical_monomial: None, backend_features: None, deferred_materialize: None, quadratic_monomial: None, polarity_encoding: None, tier_classification: None, assumption_binding: None, effectiveness_aggregate: None, bounded_no_good: None, polynomial_lift: None, scalar_function: None, field_resolution: None, boolean_fold: None, master_cut: None, flatten_identity: None, critical_assembly: None, variable_kind_domain: None, sort_by: None, predicate_schema: None, search_budget: None, conditional_semantics: None, big_m_policy: None, number_text: None, combine_status: None, elastic_model: None, native_write: None, input_domain: None, function_usage: None, repository_call: None, unsupported_predicate: None, boolean_scope: None, update_assignment: None, flatten_cache: None, cache_key_address: None, cache_key_identity: None, model_configuration: None, predicate_field_filter: None, predicate_field_resolution: None, analysis_group_aggregate: None, master_cut_encoding: None, q_table: None, heuristic_schedule: None, symbol_core_relations: None, symbol_linearization: None, symbol_condition: None, benders_cut_request: None, benders_metrics: None, cp_checkpoint_request: None, lbb_resume_identity: None, linear_mapping_request: None, checkpoint_request: None, reward_policy: None, hyper_heuristic_config: None, criticality_request: None, multi_target_report: None, relaxation_policy: None, target_spec: None, target_report: None, perturbation_policy: None, fixed_integer_report: None, fixed_integer_config: None, relaxation_request: None, critical_analysis: None, effectiveness_ranking: None, shortlist_limit: None, candidate_funnel: None, conflict_explanation: None, remote_artifact: None, logic_based_benders_plan: None, master_solution: None, benders_quadratic_cuts: None, pipeline_steps: None, repository_query: None, assignments: None, sort_plan: None, predicate: None, benders_cuts: None, relational_query_plan: None, linear_lifecycle: None,  indexed_registration: None, column_batch: None, column_scope: None, heart_beat: None, response_envelope: None, quadratic_orchestration: Some(configuration), vrp_problem: None, network_resilience_problem: None,
        allocation_problem: None, network_structure_problem: None, network_design_problem: None,
        network_generalized_flow_problem: None, network_flow_problem: None, sweep_cases: None, incumbent_values: None,
        solution_pool_options: None, gantt_activity_options: None, bpp3d_options: None, protocol_version: 1,
        solver: "scip".into(),
        model: ospf_rust_bridge::encode_quadratic_model(&model).unwrap(),
        tuning_options: None, model_kind: Some(ModelKind::QuadraticOrchestration), time_limit_ms: Some(60_000),
        cp_options: None, serial_options: None, parallel_options: None, fixed_recourse_options: None,
        column_generation_options: None, bpp1d_options: None, bpp2d_options: None, bpp_loading_options: None,
        csp2d_options: None, csp1d_options: None, iis_options: None }
}

fn parallel(mode: Option<ParallelCombinatorialMode>) -> QuadraticOrchestrationOptions {
    QuadraticOrchestrationOptions { backends: vec![NativeBackend::Scip], orchestration: QuadraticOrchestration::Parallel,
        mode, fallback_policy: None }
}

#[test]
fn orchestration_rejects_mixed_direction_fields_and_option_mixing() {
    let engine = NativeEngine::default();
    // 串行配置带并行模式必须被拒，反之亦然。 / A serial configuration with a parallel mode is rejected, and vice versa.
    assert_eq!(engine.submit(request(QuadraticOrchestrationOptions { backends: vec![NativeBackend::Scip],
        orchestration: QuadraticOrchestration::Serial, mode: Some(ParallelCombinatorialMode::First),
        fallback_policy: None })).unwrap_err().code, "InvalidInput");
    assert_eq!(engine.submit(request(QuadraticOrchestrationOptions { backends: vec![NativeBackend::Scip],
        orchestration: QuadraticOrchestration::Parallel, mode: None,
        fallback_policy: Some(SerialFallbackPolicy::ContinueOnTerminal) })).unwrap_err().code, "InvalidInput");
    // 重复后端与空后端都必须被拒。 / Repeated and empty backend lists are rejected.
    assert_eq!(engine.submit(request(QuadraticOrchestrationOptions { backends: vec![NativeBackend::Scip, NativeBackend::Scip],
        orchestration: QuadraticOrchestration::Parallel, mode: None, fallback_policy: None })).unwrap_err().code, "InvalidInput");
    assert_eq!(engine.submit(request(QuadraticOrchestrationOptions { backends: Vec::new(),
        orchestration: QuadraticOrchestration::Parallel, mode: None, fallback_policy: None })).unwrap_err().code, "InvalidInput");
    // 缺少 modelKind 或混入领域选项都必须被拒。 / A missing modelKind or a mixed-in domain option is rejected.
    let mut missing_kind = request(parallel(None));
    missing_kind.model_kind = None;
    assert_eq!(engine.submit(missing_kind).unwrap_err().code, "InvalidInput");
    let mut mixed = request(parallel(None));
    mixed.network_flow_problem = Some(ospf_rust_bridge::NetworkFlowProblem { nodes: Vec::new(), arcs: Vec::new(), commodities: Vec::new() });
    assert_eq!(engine.submit(mixed).unwrap_err().code, "InvalidInput");
}

#[cfg(feature = "scip")]
#[test]
fn orchestration_runs_the_quadratic_backend_and_reports_attempts() {
    let engine = NativeEngine::default();
    let view = engine.submit(request(parallel(Some(ParallelCombinatorialMode::Best)))).unwrap();
    let finished = std::iter::repeat_with(|| { std::thread::sleep(std::time::Duration::from_millis(20)); engine.get(&view.id).unwrap() })
        .take(6000).find(|job| job.status != JobStatus::Running).expect("orchestration job never reached a terminal state");
    assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
    let json = finished.report_json.as_deref().unwrap();
    let aggregate = ospf_rust_bridge::decode_quadratic_orchestration_report(json, &quadratic_model()).unwrap();
    // 单后端 → 恰好一个 attempt，且报告来自真实 SCIP。 / One backend means exactly one attempt with a real SCIP report.
    assert_eq!(aggregate.attempts.len(), 1, "{:?}", aggregate.attempts);
    // 后端名是求解器自报的，不是桥接的小写后端 ID。
    // The backend name comes from the solver itself, not from the bridge's lowercase backend ID.
    assert_eq!(aggregate.attempts[0].provenance.backend_name, "SCIP");
    assert!(aggregate.report.has_incumbent(), "{:?}", aggregate.report.problem_status);
    engine.release(&view.id).unwrap();
}

#[cfg(feature = "scip")]
#[test]
fn an_aggregate_for_a_different_model_is_refused() {
    let engine = NativeEngine::default();
    let view = engine.submit(request(parallel(None))).unwrap();
    let finished = std::iter::repeat_with(|| { std::thread::sleep(std::time::Duration::from_millis(20)); engine.get(&view.id).unwrap() })
        .take(6000).find(|job| job.status != JobStatus::Running).expect("orchestration job never reached a terminal state");
    assert_eq!(finished.status, JobStatus::Completed);
    let json = finished.report_json.as_deref().unwrap();
    // 换一个改过目标系数的模型再喂同一份聚合报告，必须因指纹不符被拒。
    // Feeding the same aggregate to a model with a changed objective coefficient must be refused on fingerprint.
    let mut other = quadratic_model();
    other.c = vec![2.0];
    assert_eq!(ospf_rust_bridge::decode_quadratic_orchestration_report(json, &other).unwrap_err().code, "FingerprintMismatch");
    engine.release(&view.id).unwrap();
}
