//! 构建期/观测能力的原生端契约：这些入口不求解，只把框架层能力如实暴露。
//! Native contract for the build-time/observability capabilities: these entries do not solve; they expose framework
//! capabilities faithfully.

use crate::*;
use ospf_rust_bridge::{
    HeartBeatRequest, IndexedKey, IndexedRegistrationMode, IndexedRegistrationRequest, IndexedVariableKind,
    LinearColumnBatch, LinearColumnCompletion, LinearColumnScope, ResponseEnvelope,
};

fn base_request() -> SolveRequest {
    SolveRequest { schedule_log_probe: None, operator_capability_probe: None, hypervolume_probe: None, acceptance_recovery_probe: None, training_split_probe: None, non_stationary_probe: None, selection_policy_probe: None, solve_value_probe: None, native_lowering_probe: None, native_terminal_proof_probe: None, predicate_scope_probe: None, persistence_schema_probe: None, persistence_sort_probe: None, effectiveness_ranking_probe: None, effectiveness_ranking: None, funnel_ranking: None, sensitivity_probe: None, correction_probe: None, blocking_set: None, fingerprint_probe: None, diagnostics_probe: None, gap_probe: None, proof_probe: None, evidence_probe: None, band_probe: None, group_summary: None, critical_report: None, effectiveness_probe: None, funnel: None, solution_status: None, diversity: None, piecewise: None, archive_probe: None, migration: None, topology: None, ranking_probe: None, acceptance: None, boundary: None, population_config: None, constraint_priority: None, constraint_ranking: None, pareto_archive: None, reference_points: None, dispersion: None, dominance: None, evidence_remap: None, tier_selection: None, activation_set: None, container_surface: None, identity_wrap: None, domain_surface: None, expression_bounds: None, assignment_eval: None, constraint_canon: None, snapshot_gate: None, cp_model_build: None, no_good_oracle: None, subproblem_result: None, cp_support_merge: None, capability_matrix: None, analysis_options: None, model_view: None, polynomial_probe: None, matrix_rows: None, sparse_triad: None, master_assignment: None, stable_value: None, quadratic_elastic: None, dump_batch: None, lazy_flatten_init: None, candidate_shortlist: None, writer_registry: None, native_write_scope: None, dump_options: None, assignment_composition: None, cut_validity: None, source_identity: None, binding_assignment: None, auxiliary_domain: None, activity_report_check: None, evidence_predicates: None, activity_bands: None, encoding_label: None, expression_key: None, canonical_monomial: None, backend_features: None, deferred_materialize: None, quadratic_monomial: None, polarity_encoding: None, tier_classification: None, assumption_binding: None, effectiveness_aggregate: None, bounded_no_good: None, polynomial_lift: None, scalar_function: None, field_resolution: None, boolean_fold: None, master_cut: None, flatten_identity: None, critical_assembly: None, variable_kind_domain: None, sort_by: None, predicate_schema: None, search_budget: None, conditional_semantics: None, big_m_policy: None, number_text: None, combine_status: None, elastic_model: None, native_write: None, input_domain: None, function_usage: None, repository_call: None, unsupported_predicate: None, boolean_scope: None, update_assignment: None, flatten_cache: None, cache_key_address: None, cache_key_identity: None, model_configuration: None, predicate_field_filter: None, predicate_field_resolution: None, analysis_group_aggregate: None, master_cut_encoding: None, q_table: None, heuristic_schedule: None, symbol_core_relations: None, symbol_linearization: None, symbol_condition: None, benders_cut_request: None, benders_metrics: None, cp_checkpoint_request: None, lbb_resume_identity: None, linear_mapping_request: None, checkpoint_request: None, reward_policy: None, hyper_heuristic_config: None, criticality_request: None, multi_target_report: None, relaxation_policy: None, target_spec: None, target_report: None, perturbation_policy: None, fixed_integer_report: None, fixed_integer_config: None, relaxation_request: None, critical_analysis: None, shortlist_limit: None, candidate_funnel: None, conflict_explanation: None, remote_artifact: None, logic_based_benders_plan: None, master_solution: None, benders_quadratic_cuts: None, pipeline_steps: None, repository_query: None, assignments: None, sort_plan: None, predicate: None, benders_cuts: None, relational_query_plan: None, linear_lifecycle: None, solver: "scip".into(), protocol_version: 1, model: ospf_rust_bridge::encode_model(&ospf_rust_core::model::MetaModel::<f64>::new("helper-placeholder")
            .try_to_linear_triad_model().unwrap()).unwrap(), tuning_options: None, time_limit_ms: None, cp_options: None, serial_options: None, parallel_options: None, fixed_recourse_options: None, column_generation_options: None, bpp1d_options: None, bpp2d_options: None, bpp_loading_options: None, csp2d_options: None, csp1d_options: None, iis_options: None, model_kind: None, indexed_registration: None, column_batch: None, column_scope: None, heart_beat: None, response_envelope: None, quadratic_orchestration: None, vrp_problem: None, network_resilience_problem: None, allocation_problem: None, network_structure_problem: None, network_design_problem: None, network_generalized_flow_problem: None, network_flow_problem: None, sweep_cases: None, incumbent_values: None, solution_pool_options: None, gantt_activity_options: None, bpp3d_options: None }
}

fn settle(engine: &NativeEngine, view: &JobView) -> JobView {
    std::iter::repeat_with(|| { std::thread::sleep(std::time::Duration::from_millis(10)); engine.get(&view.id).unwrap() })
        .take(3000).find(|job| job.status != JobStatus::Running).expect("helper job never reached a terminal state")
}

#[test]
fn helper_entries_reject_mixing_and_require_their_own_kind() {
    let engine = NativeEngine::default();
    // 每个入口都必须显式声明自己的 `modelKind`。 / Each entry must declare its own `modelKind`.
    let mut registration = base_request();
    registration.indexed_registration = Some(IndexedRegistrationRequest { prefix: "x".into(),
        mode: IndexedRegistrationMode::Optional, kind: IndexedVariableKind::Continuous,
        keys: vec![IndexedKey { key: "a".into(), lower_bound: Some(0.0), upper_bound: Some(3.0) }], retired_keys: Vec::new() });
    assert_eq!(engine.submit(registration.clone()).unwrap_err().code, "InvalidInput");
    registration.model_kind = Some(ModelKind::IndexedRegistration);
    let view = engine.submit(registration).unwrap();
    let finished = settle(&engine, &view);
    assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
    engine.release(&view.id).unwrap();
    // 批次与作用域必须成对给出。 / The batch and the scope must be given together.
    let mut half = base_request();
    half.model_kind = Some(ModelKind::LinearColumnCertificate);
    half.column_batch = Some(LinearColumnBatch { column_count: 0, completion: LinearColumnCompletion::Exhausted,
        scope: None, no_improving_columns: None, children: Vec::new() });
    assert_eq!(engine.submit(half).unwrap_err().code, "InvalidInput");
    // 自相矛盾的信封在入口就被拒。 / A self-contradictory envelope is refused at the entry.
    let mut broken_envelope = base_request();
    broken_envelope.model_kind = Some(ModelKind::ResponseEnvelope);
    broken_envelope.response_envelope = Some(ResponseEnvelope { success: true, message: Some("why".into()),
        data: Some("payload".into()) });
    assert_eq!(engine.submit(broken_envelope).unwrap_err().code, "InvalidReport");
    // 心跳次数超限被拒。 / An oversized tick count is refused.
    let mut too_many = base_request();
    too_many.model_kind = Some(ModelKind::HeartBeat);
    too_many.heart_beat = Some(HeartBeatRequest { ticks: HeartBeatRequest::MAX_TICKS + 1 });
    assert_eq!(engine.submit(too_many).unwrap_err().code, "InvalidInput");
}

#[test]
fn indexed_registration_entry_returns_a_solvable_model_and_key_map() {
    let engine = NativeEngine::default();
    let mut request = base_request();
    request.model_kind = Some(ModelKind::IndexedRegistration);
    request.indexed_registration = Some(IndexedRegistrationRequest { prefix: "x".into(),
        mode: IndexedRegistrationMode::Appendable, kind: IndexedVariableKind::Continuous,
        keys: vec![IndexedKey { key: "a".into(), lower_bound: Some(0.0), upper_bound: Some(3.0) },
            IndexedKey { key: "b".into(), lower_bound: Some(0.0), upper_bound: Some(3.0) }],
        retired_keys: vec!["b".into()] });
    let view = engine.submit(request).unwrap();
    let finished = settle(&engine, &view);
    assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
    let report: ospf_rust_bridge::IndexedRegistrationReport =
        serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
    // 退役键仍占一列，但不进入映射。 / The retired key still occupies a column but is absent from the map.
    assert_eq!(report.variable_count, 2);
    assert_eq!(report.indices.len(), 1);
    assert_eq!(report.indices[0].key, "a");
    assert_eq!(report.indices[0].index, 0);
    assert_eq!(report.retired_keys, vec!["b".to_owned()]);
    // 产出的模型本身可以再提交求解。 / The produced model can itself be submitted for solving.
    let mut solve = base_request();
    solve.model_kind = Some(ModelKind::Linear);
    solve.model = report.model.clone();
    let solved = settle(&engine, &engine.submit(solve).unwrap());
    assert_eq!(solved.status, JobStatus::Completed, "{:?}", solved.error);
    engine.release(&view.id).unwrap();
}

#[test]
fn column_certificate_entry_reports_both_verdicts_separately() {
    let engine = NativeEngine::default();
    let scope = LinearColumnScope { request: "round-1".into(), groups: vec!["a".into()], model: "model-1".into(),
        dual: "dual-1".into(), branch: "root".into(), global: true, executor_slots: vec!["a:0".into()] };
    let mut request = base_request();
    request.model_kind = Some(ModelKind::LinearColumnCertificate);
    request.column_batch = Some(LinearColumnBatch { column_count: 0, completion: LinearColumnCompletion::Exhausted,
        scope: Some(scope.clone()), no_improving_columns: None, children: Vec::new() });
    request.column_scope = Some(scope);
    let view = engine.submit(request).unwrap();
    let finished = settle(&engine, &view);
    assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
    let certificate: ospf_rust_bridge::LinearColumnCertificate =
        serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
    // 完整搜索成立，但缺无改进证据：两个判定必须分开。 / Exhaustive holds, no-improvement does not: the two stay apart.
    assert!(certificate.certifies_exhaustive);
    assert!(!certificate.certifies_no_improving_columns);
    engine.release(&view.id).unwrap();
}

#[test]
fn heartbeat_and_envelope_entries_are_pure_and_faithful() {
    let engine = NativeEngine::default();
    let mut heartbeat = base_request();
    heartbeat.model_kind = Some(ModelKind::HeartBeat);
    heartbeat.heart_beat = Some(HeartBeatRequest { ticks: 5 });
    let view = engine.submit(heartbeat).unwrap();
    let finished = settle(&engine, &view);
    assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
    let snapshot: ospf_rust_bridge::HeartBeatSnapshot =
        serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
    assert_eq!(snapshot.tick_count, 5);
    engine.release(&view.id).unwrap();
    // 成功信封原样返回，不补字段。 / A successful envelope comes back as-is, with no field filled in.
    let mut envelope = base_request();
    envelope.model_kind = Some(ModelKind::ResponseEnvelope);
    envelope.response_envelope = Some(ResponseEnvelope::ok("payload").unwrap());
    let view = engine.submit(envelope).unwrap();
    let finished = settle(&engine, &view);
    assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
    let echoed: ospf_rust_bridge::ResponseEnvelope =
        serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
    assert!(echoed.success);
    assert_eq!(echoed.data.as_deref(), Some("payload"));
    assert_eq!(echoed.message, None);
    engine.release(&view.id).unwrap();
}

#[test]
fn lifecycle_entry_applies_operations_in_order() {
    let engine = NativeEngine::default();
    let mut request = base_request();
    request.model_kind = Some(ModelKind::LinearLifecycle);
    request.linear_lifecycle = Some(ospf_rust_bridge::LinearLifecycleRequest { column_count: 3,
        operations: vec![ospf_rust_bridge::LinearColumnOperation { kind: ospf_rust_bridge::LinearColumnOperationKind::Hide,
            columns: vec![1] }],
        solution: Some(vec![0.0, 1.0, 2.0]), warm_start_threshold: Some(1.0) });
    let view = engine.submit(request).unwrap();
    let finished = settle(&engine, &view);
    assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
    let report: ospf_rust_bridge::LinearLifecycleReport =
        serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
    assert_eq!(report.states[1].state, ospf_rust_bridge::LinearColumnState::Hidden);
    // 隐藏列即便超过阈值也不进入 warm start。 / A hidden column stays out of the warm start even above the threshold.
    assert_eq!(report.warm_start_columns, vec![2]);
    assert!(!report.selectable_columns.contains(&1));
    engine.release(&view.id).unwrap();
    // 越界列索引在入口就被拒。 / An out-of-range column index is refused at the entry.
    let mut broken = base_request();
    broken.model_kind = Some(ModelKind::LinearLifecycle);
    broken.linear_lifecycle = Some(ospf_rust_bridge::LinearLifecycleRequest { column_count: 2,
        operations: vec![ospf_rust_bridge::LinearColumnOperation { kind: ospf_rust_bridge::LinearColumnOperationKind::Hide,
            columns: vec![2] }], solution: None, warm_start_threshold: None });
    assert_eq!(engine.submit(broken).unwrap_err().code, "InvalidInput");
}

#[test]
fn query_plan_entry_validates_and_canonicalises() {
    use ospf_rust_bridge::{QueryColumn, QueryComparison, QueryFilter, QueryJoinCardinality, QueryJoinSpec,
        QueryJoinType, QueryScalar, QuerySourceSpec, RelationalQueryPlanSpec};
    let engine = NativeEngine::default();
    let mut plan = RelationalQueryPlanSpec {
        root: QuerySourceSpec { name: "orders".into(), alias: Some("o".into()) },
        joins: vec![QueryJoinSpec { join_type: QueryJoinType::Inner,
            source: QuerySourceSpec { name: "customers".into(), alias: Some("c".into()) },
            condition: QueryFilter::Comparison { operator: QueryComparison::Eq,
                left: QueryScalar::Column { source: "o".into(), path: "customer_id".into() },
                right: QueryScalar::Column { source: "c".into(), path: "id".into() } },
            cardinality: QueryJoinCardinality::OneToOne }],
        predicate: None,
        projections: vec![QueryColumn { source: "o".into(), path: "id".into() }],
        group_by: Vec::new(), order_by: Vec::new(), root_key: Vec::new(), page: None, limits: None };
    let mut request = base_request();
    request.model_kind = Some(ModelKind::RelationalQueryPlan);
    request.relational_query_plan = Some(plan.clone());
    let view = engine.submit(request).unwrap();
    let finished = settle(&engine, &view);
    assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
    let report: ospf_rust_bridge::QueryPlanReport =
        serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
    assert!(report.valid, "{:?}", report.violations);
    assert_eq!(report.join_count, 1);
    assert_eq!(report.depth, 2);
    assert!(report.canonical_hash.is_some());
    engine.release(&view.id).unwrap();
    // 非法计划：报告不可用且不给出规范化结果。 / An invalid plan yields a report without a canonical form.
    plan.joins[0].source = QuerySourceSpec { name: "orders".into(), alias: None };
    let mut broken = base_request();
    broken.model_kind = Some(ModelKind::RelationalQueryPlan);
    broken.relational_query_plan = Some(plan);
    let view = engine.submit(broken).unwrap();
    let finished = settle(&engine, &view);
    assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
    let report: ospf_rust_bridge::QueryPlanReport =
        serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
    assert!(!report.valid);
    assert_eq!(report.violations.len(), 1);
    assert_eq!(report.canonical, None);
    engine.release(&view.id).unwrap();
}

/// 最小线性主问题：`0 <= x <= 10` 且 `x <= 5`。没有目标函数也没关系——cut 是否生效直接看可行性变化。
/// A minimal linear master: `0 <= x <= 10` and `x <= 5`. No objective is needed — whether a cut took effect shows up
/// directly as a feasibility change.
fn benders_master_model() -> ospf_rust_core::model::intermediate::LinearTriadModel {
    use ospf_rust_core::model::{ConstraintRelation, Linear, LinearInequality, LinearMonomial, MetaConstraint, MetaModel};
    use ospf_rust_core::variable::{ContinuousVariableItem, VariableRange};
    let mut source = MetaModel::<f64>::new("benders-master");
    source.add_variable(ContinuousVariableItem::auto_with_range("x", VariableRange::new(Some(0.0), Some(10.0)))).unwrap();
    source.add_constraint(MetaConstraint::new(LinearInequality::new(
        Linear::new(vec![LinearMonomial::new(1.0, 0)], 0.0), ConstraintRelation::LessEqual, 5.0), "cap")).unwrap();
    source.try_to_linear_triad_model().unwrap()
}

#[test]
fn benders_master_entry_applies_cuts_and_reports_contradictions() {
    let engine = NativeEngine::default();
    let cut = |sense, rhs| ospf_rust_bridge::BendersCutSpec { name: format!("c-{rhs}"),
        coefficients: vec![ospf_rust_bridge::BendersCutCoefficient { column: 0, coefficient: 1.0 }],
        sense, rhs };
    let submit = |cuts| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::LinearBendersMaster);
        request.model = ospf_rust_bridge::encode_model(&benders_master_model()).unwrap();
        request.benders_cuts = Some(cuts);
        engine.submit(request).unwrap()
    };
    // 1) `x >= 7` 与 `x <= 5` 直接冲突：cut 确实被应用了。 / `x >= 7` contradicts `x <= 5`: the cut really applied.
    let view = submit(vec![cut(ospf_rust_bridge::BendersCutSense::GreaterOrEqual, 7.0)]);
    let finished = settle(&engine, &view);
    assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
    let report: ospf_rust_bridge::BendersMasterReport =
        serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
    assert_eq!(report.cut_count, 1);
    let wire: serde_json::Value = serde_json::from_str(&report.report).unwrap();
    assert_eq!(wire["problemStatus"], "INFEASIBLE", "{wire}");
    engine.release(&view.id).unwrap();
    // 2) `x <= 0.5` 仍然可行：不是"加了 cut 就不可行"。 / `x <= 0.5` stays feasible: a cut is not a blanket infeasibility.
    let view = submit(vec![cut(ospf_rust_bridge::BendersCutSense::LessOrEqual, 0.5)]);
    let finished = settle(&engine, &view);
    assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
    let report: ospf_rust_bridge::BendersMasterReport =
        serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
    let wire: serde_json::Value = serde_json::from_str(&report.report).unwrap();
    assert_eq!(wire["problemStatus"], "FEASIBLE", "{wire}");
    engine.release(&view.id).unwrap();
    // 3) 不带 cut 的裸主问题也必须能跑。 / A bare master with no cuts must run too.
    let view = submit(Vec::new());
    let finished = settle(&engine, &view);
    assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
    engine.release(&view.id).unwrap();
    // 4) 越界列索引在入口被拒。 / An out-of-range column index is refused at the entry.
    let mut broken = base_request();
    broken.model_kind = Some(ModelKind::LinearBendersMaster);
    broken.model = ospf_rust_bridge::encode_model(&benders_master_model()).unwrap();
    broken.benders_cuts = Some(vec![ospf_rust_bridge::BendersCutSpec { name: "bad".into(),
        coefficients: vec![ospf_rust_bridge::BendersCutCoefficient { column: 9, coefficient: 1.0 }],
        sense: ospf_rust_bridge::BendersCutSense::LessOrEqual, rhs: 1.0 }]);
    assert_eq!(engine.submit(broken).unwrap_err().code, "InvalidInput");
}

#[test]
fn field_filters_entry_decomposes_or_says_it_cannot() {
    use ospf_rust_bridge::{QueryComparison, QueryFilter, QueryScalar};
    let engine = NativeEngine::default();
    let submit = |predicate| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::PredicateFieldFilters);
        request.predicate = Some(predicate);
        engine.submit(request).unwrap()
    };
    // 合取式谓词按字段拆开，字段名是属性路径原文，顺序稳定。
    // A conjunctive predicate splits per field; field names are the raw property paths in a stable order.
    let view = submit(QueryFilter::And { items: vec![
        QueryFilter::Comparison { operator: QueryComparison::Eq,
            left: QueryScalar::Column { source: "orders".into(), path: "status".into() },
            right: QueryScalar::String { value: "paid".into() } },
        QueryFilter::NullCheck { source: "orders".into(), path: "note".into(), negated: true },
    ] });
    let finished = settle(&engine, &view);
    assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
    let report: ospf_rust_bridge::FieldFilterReport =
        serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
    assert!(report.decomposable);
    let fields: Vec<&str> = report.filters.iter().map(|entry| entry.field.as_str()).collect();
    assert_eq!(fields, vec!["orders.note", "orders.status"]);
    let status = report.filters.iter().find(|entry| entry.field == "orders.status").unwrap();
    assert_eq!(status.filter.eq, Some(QueryScalar::String { value: "paid".into() }));
    engine.release(&view.id).unwrap();
    // 常量谓词不可分解：桥接如实转述，不硬凑一份空过滤表当成"已分解"。
    // A constant predicate is not decomposable: the bridge relays that instead of passing an empty table off as
    // "decomposed".
    let view = submit(QueryFilter::Trivalent { value: ospf_rust_bridge::QueryTrivalent::True });
    let finished = settle(&engine, &view);
    assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
    let report: ospf_rust_bridge::FieldFilterReport =
        serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
    assert!(!report.decomposable);
    assert!(report.filters.is_empty());
    engine.release(&view.id).unwrap();
    // 缺谓词在入口被拒。 / A missing predicate is refused at the entry.
    let mut broken = base_request();
    broken.model_kind = Some(ModelKind::PredicateFieldFilters);
    assert_eq!(engine.submit(broken).unwrap_err().code, "InvalidInput");
}

#[test]
fn edit_entries_flag_ambiguity_and_unsupported_null_ordering() {
    use ospf_rust_bridge::{AssignmentKind, AssignmentReport, AssignmentSpec, AssignmentValue, NullsSupportLevel,
        QueryColumn, QueryNullsOrder, QueryOrderSpec, QuerySortDirection, SortPlanReport, SortPlanSpec};
    let engine = NativeEngine::default();
    // 1) 重复赋值字段被标出，顺序原样保留，不替调用方挑赢家。
    // A duplicated assignment field is reported, order is preserved, and no winner is picked for the caller.
    let mut request = base_request();
    request.model_kind = Some(ModelKind::UpdateAssignments);
    request.assignments = Some(vec![
        AssignmentSpec { path: "orders.status".into(), kind: AssignmentKind::Value,
            value: Some(AssignmentValue::String { value: "paid".into() }) },
        AssignmentSpec { path: "orders.note".into(), kind: AssignmentKind::Null, value: None },
        AssignmentSpec { path: "orders.status".into(), kind: AssignmentKind::Value,
            value: Some(AssignmentValue::String { value: "void".into() }) },
    ]);
    let view = engine.submit(request).unwrap();
    let finished = settle(&engine, &view);
    assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
    let report: AssignmentReport = serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
    assert_eq!(report.entries.len(), 3);
    assert_eq!(report.duplicate_paths, vec!["orders.status".to_owned()]);
    assert!(!report.is_empty);
    engine.release(&view.id).unwrap();
    // 空赋值集合是合法的。 / An empty assignment set is legal.
    let mut request = base_request();
    request.model_kind = Some(ModelKind::UpdateAssignments);
    let view = engine.submit(request).unwrap();
    let finished = settle(&engine, &view);
    assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
    let report: AssignmentReport = serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
    assert!(report.is_empty);
    engine.release(&view.id).unwrap();
    // 非法赋值形态在入口被拒。 / Malformed assignments are refused at the entry.
    let mut broken = base_request();
    broken.model_kind = Some(ModelKind::UpdateAssignments);
    broken.assignments = Some(vec![AssignmentSpec { path: " ".into(), kind: AssignmentKind::Null, value: None }]);
    assert_eq!(engine.submit(broken).unwrap_err().code, "InvalidInput");

    // 2) 后端从不支持空值排序：只有声明了的那一项被拒。
    // A backend that never supports null ordering rejects only the item that declared it.
    let mut request = base_request();
    request.model_kind = Some(ModelKind::SortPlan);
    request.sort_plan = Some(SortPlanSpec { support: NullsSupportLevel::Never, items: vec![
        QueryOrderSpec { column: QueryColumn { source: "orders".into(), path: "id".into() },
            direction: QuerySortDirection::Ascending, nulls: None },
        QueryOrderSpec { column: QueryColumn { source: "orders".into(), path: "amount".into() },
            direction: QuerySortDirection::Descending, nulls: Some(QueryNullsOrder::Last) },
    ] });
    let view = engine.submit(request).unwrap();
    let finished = settle(&engine, &view);
    assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
    let report: SortPlanReport = serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
    assert!(!report.accepted);
    assert_eq!(report.rejected_indices, vec![1]);
    // `nulls_supported` 描述"后端能不能表达"，与"有没有声明"分开。 / `nulls_supported` describes expressibility,
    // separate from whether one was declared.
    assert!(!report.items[0].declares_nulls_order);
    assert!(!report.items[0].nulls_supported);
    engine.release(&view.id).unwrap();
    // 缺 plan 在入口被拒。 / A missing plan is refused at the entry.
    let mut broken = base_request();
    broken.model_kind = Some(ModelKind::SortPlan);
    assert_eq!(engine.submit(broken).unwrap_err().code, "InvalidInput");
}

#[test]
fn repository_query_entry_merges_where_sort_and_paging() {
    use ospf_rust_bridge::{QueryColumn, QueryComparison, QueryFilter, QueryPlanReport, QueryScalar, QuerySortDirection,
        QuerySourceSpec, RelationalQueryPlanSpec, RepositoryQuerySpec};
    let engine = NativeEngine::default();
    let spec = |limit: Option<u32>, offset: Option<u32>| RepositoryQuerySpec {
        plan: RelationalQueryPlanSpec { root: QuerySourceSpec { name: "orders".into(), alias: None },
            joins: Vec::new(), predicate: None,
            projections: vec![QueryColumn { source: "orders".into(), path: "id".into() }],
            group_by: Vec::new(), order_by: Vec::new(), root_key: Vec::new(), page: None, limits: None },
        predicate: QueryFilter::Comparison { operator: QueryComparison::Eq,
            left: QueryScalar::Column { source: "orders".into(), path: "status".into() },
            right: QueryScalar::String { value: "paid".into() } },
        sort: vec![ospf_rust_bridge::QueryOrderSpec { column: QueryColumn { source: "orders".into(), path: "id".into() },
            direction: QuerySortDirection::Descending, nulls: None }],
        limit, offset };
    let submit = |spec| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::RepositoryQuery);
        request.repository_query = Some(spec);
        engine.submit(request).unwrap()
    };
    // 不带分页：合法且有规范化结果。 / No paging: valid with a canonical form.
    let view = submit(spec(None, None));
    let finished = settle(&engine, &view);
    assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
    let plain: QueryPlanReport = serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
    assert!(plain.valid, "{:?}", plain.violations);
    assert!(plain.canonical_hash.is_some());
    engine.release(&view.id).unwrap();
    // 带分页：规范化结果随之改变，说明排序/分页真的并进了计划。
    // With paging: the canonical form changes, proving sort/paging really merged into the plan.
    let view = submit(spec(Some(10), Some(20)));
    let finished = settle(&engine, &view);
    assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
    let paged: QueryPlanReport = serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
    assert!(paged.valid);
    assert_ne!(paged.canonical, plain.canonical);
    engine.release(&view.id).unwrap();
    // 只有 offset 也是合法计划（不是limit=0）。 / Offset-only is a valid plan too (not limit=0).
    let view = submit(spec(None, Some(7)));
    let finished = settle(&engine, &view);
    let offset_only: QueryPlanReport = serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
    assert!(offset_only.valid);
    engine.release(&view.id).unwrap();
    // 缺请求在入口被拒。 / A missing request is refused at the entry.
    let mut broken = base_request();
    broken.model_kind = Some(ModelKind::RepositoryQuery);
    assert_eq!(engine.submit(broken).unwrap_err().code, "InvalidInput");
}

#[test]
fn pipeline_plan_entry_reports_registration_and_id_name_conflicts() {
    use ospf_rust_bridge::{PipelineGroupSpec, PipelinePlanReport, PipelineStepSpec};
    let engine = NativeEngine::default();
    let submit = |steps| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::PipelinePlan);
        request.pipeline_steps = Some(steps);
        engine.submit(request).unwrap()
    };
    let step = |name: &str, group: Option<(u64, &str)>| PipelineStepSpec { name: name.into(),
        constraint_group: group.map(|(id, name)| PipelineGroupSpec { id, name: name.into() }) };
    // 注册序列按步骤顺序去重；同 id 同名称的后续声明被吞。
    // The registration sequence is deduplicated in step order; later declarations with the same id and name are
    // swallowed.
    let view = submit(vec![step("first", Some((1, "loads"))), step("no-group", None),
        step("second", Some((1, "loads"))), step("third", Some((2, "capacity")))]);
    let finished = settle(&engine, &view);
    assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
    let report: PipelinePlanReport = serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
    assert_eq!(report.steps.len(), 4);
    assert_eq!(report.registered_groups,
        vec![PipelineGroupSpec { id: 1, name: "loads".into() },
            PipelineGroupSpec { id: 2, name: "capacity".into() }]);
    assert!(!report.steps[2].registers_group);
    assert!(report.steps[2].already_registered);
    assert!(report.id_name_conflicts.is_empty());
    engine.release(&view.id).unwrap();
    // 同 id 不同名：领域会吞冲突并保留第一个名字，桥接必须把它报出来。
    // Same id with different names: the domain swallows the conflict and keeps the first name, and the bridge must
    // report it.
    let view = submit(vec![step("first", Some((7, "loads"))), step("second", Some((7, "weight")))]);
    let finished = settle(&engine, &view);
    let report: PipelinePlanReport = serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
    assert_eq!(report.registered_groups.len(), 1);
    assert_eq!(report.id_name_conflicts.len(), 1);
    assert_eq!(report.id_name_conflicts[0].first_name, "loads");
    assert_eq!(report.id_name_conflicts[0].later_names, vec!["weight".to_owned()]);
    engine.release(&view.id).unwrap();
    // 空步骤列表合法。 / An empty step list is legal.
    let view = submit(Vec::new());
    let finished = settle(&engine, &view);
    assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
    engine.release(&view.id).unwrap();
    // 空步骤名在入口被拒。 / A blank step name is refused at the entry.
    let mut broken = base_request();
    broken.model_kind = Some(ModelKind::PipelinePlan);
    broken.pipeline_steps = Some(vec![step(" ", None)]);
    assert_eq!(engine.submit(broken).unwrap_err().code, "InvalidInput");
}

/// 最小二次主问题：`0 <= x <= 3` 且 `x^2 <= 4`，没有目标函数——cut 是否生效看可行性变化。
/// A minimal quadratic master: `0 <= x <= 3` and `x^2 <= 4`, with no objective — a cut's effect shows as feasibility.
fn quadratic_master_model() -> ospf_rust_core::model::intermediate::QuadraticTetradModel {
    use ospf_rust_core::model::{ConstraintRelation, MetaModel, Quadratic, QuadraticInequality, QuadraticMonomial};
    use ospf_rust_core::variable::{ContinuousVariableItem, VariableRange};
    let mut source = MetaModel::<f64>::new("quadratic-benders-master");
    source.add_variable(ContinuousVariableItem::auto_with_range("x", VariableRange::new(Some(0.0), Some(3.0)))).unwrap();
    let mut model = source.try_to_quadratic_tetrad_model().unwrap();
    model.add_quadratic_constraint_with_metadata(
        QuadraticInequality::new(Quadratic::new(vec![QuadraticMonomial::new_quadratic(1.0, 0, 0)], 0.0),
            ConstraintRelation::LessEqual, 4.0), "ball".into(), None, false, 0, None, None);
    model
}

// 二次主问题需要 `scip-quadratic`；只开 `scip` 时该入口会明确返回 `UnsupportedDialect`，所以这里按特性门控。
// The quadratic master needs `scip-quadratic`; with only `scip` the entry returns `UnsupportedDialect`, hence the gate.
#[cfg(feature = "scip-quadratic")]
#[test]
fn quadratic_benders_master_applies_linear_and_quadratic_cuts() {
    use ospf_rust_bridge::{BendersCutCoefficient, BendersCutSense, BendersCutSpec, BendersMasterReport,
        BendersQuadraticCutSpec, BendersQuadraticTerm};
    let engine = NativeEngine::default();
    let submit = |linear: Vec<BendersCutSpec>, quadratic: Vec<BendersQuadraticCutSpec>| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::QuadraticBendersMaster);
        request.model = ospf_rust_bridge::encode_quadratic_model(&quadratic_master_model()).unwrap();
        request.benders_cuts = Some(linear);
        request.benders_quadratic_cuts = Some(quadratic);
        engine.submit(request).unwrap()
    };
    let linear_ge = |rhs| BendersCutSpec { name: format!("lin-{rhs}"),
        coefficients: vec![BendersCutCoefficient { column: 0, coefficient: 1.0 }],
        sense: BendersCutSense::GreaterOrEqual, rhs };
    // 1) 线性 cut `x >= 5` 与 `x <= 3` 冲突：cut 真的被应用。 / `x >= 5` contradicts `x <= 3`: the cut really applied.
    let view = submit(vec![linear_ge(5.0)], Vec::new());
    let finished = settle(&engine, &view);
    assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
    let report: BendersMasterReport = serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
    assert_eq!(report.cut_count, 1);
    let wire: serde_json::Value = serde_json::from_str(&report.report).unwrap();
    assert_eq!(wire["problemStatus"], "INFEASIBLE", "{wire}");
    engine.release(&view.id).unwrap();
    // 2) 二次 cut `x^2 >= 100` 同样不可行：二次项确实进了主问题。
    // A quadratic cut `x^2 >= 100` is also infeasible: the quadratic term really entered the master.
    let view = submit(Vec::new(), vec![BendersQuadraticCutSpec { name: "quad".into(), linear: Vec::new(),
        quadratic: vec![BendersQuadraticTerm { left: 0, right: 0, coefficient: 1.0 }],
        sense: BendersCutSense::GreaterOrEqual, rhs: 100.0 }]);
    let finished = settle(&engine, &view);
    assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
    let report: BendersMasterReport = serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
    assert_eq!(report.cut_count, 1);
    let wire: serde_json::Value = serde_json::from_str(&report.report).unwrap();
    assert_eq!(wire["problemStatus"], "INFEASIBLE", "{wire}");
    engine.release(&view.id).unwrap();
    // 3) 相容的二次 cut `x^2 <= 9`：仍然可行（不是"加了 cut 就不可行"）。
    // A compatible quadratic cut `x^2 <= 9` stays feasible (a cut is not blanket infeasibility).
    let view = submit(Vec::new(), vec![BendersQuadraticCutSpec { name: "quad-ok".into(), linear: Vec::new(),
        quadratic: vec![BendersQuadraticTerm { left: 0, right: 0, coefficient: 1.0 }],
        sense: BendersCutSense::LessOrEqual, rhs: 9.0 }]);
    let finished = settle(&engine, &view);
    let report: BendersMasterReport = serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
    let wire: serde_json::Value = serde_json::from_str(&report.report).unwrap();
    assert_eq!(wire["problemStatus"], "FEASIBLE", "{wire}");
    engine.release(&view.id).unwrap();
    // 4) 越界二次列在入口被拒。 / An out-of-range quadratic column is refused at the entry.
    let mut broken = base_request();
    broken.model_kind = Some(ModelKind::QuadraticBendersMaster);
    broken.model = ospf_rust_bridge::encode_quadratic_model(&quadratic_master_model()).unwrap();
    broken.benders_quadratic_cuts = Some(vec![BendersQuadraticCutSpec { name: "bad".into(), linear: Vec::new(),
        quadratic: vec![BendersQuadraticTerm { left: 0, right: 7, coefficient: 1.0 }],
        sense: BendersCutSense::LessOrEqual, rhs: 1.0 }]);
    assert_eq!(engine.submit(broken).unwrap_err().code, "InvalidInput");
}

/// 子问题模型：`0 <= x <= 10`（master 列）、`0 <= y <= 2`（子问题列），`x + y >= 5`。
/// 固定 x=8 时可行；固定 x=0 时不可行（y 最多 2）。
/// Subproblem model: `0 <= x <= 10` (master column), `0 <= y <= 2` (subproblem column) and `x + y >= 5`. Fixing x=8
/// stays feasible; fixing x=0 is infeasible (y caps at 2).
fn benders_sub_model() -> ospf_rust_core::model::intermediate::LinearTriadModel {
    use ospf_rust_core::model::{ConstraintRelation, Linear, LinearInequality, LinearMonomial, MetaConstraint, MetaModel};
    use ospf_rust_core::variable::{ContinuousVariableItem, VariableRange};
    let mut source = MetaModel::<f64>::new("benders-sub");
    source.add_variable(ContinuousVariableItem::auto_with_range("x", VariableRange::new(Some(0.0), Some(10.0)))).unwrap();
    source.add_variable(ContinuousVariableItem::auto_with_range("y", VariableRange::new(Some(0.0), Some(2.0)))).unwrap();
    source.add_constraint(MetaConstraint::new(LinearInequality::new(
        Linear::new(vec![LinearMonomial::new(1.0, 0), LinearMonomial::new(1.0, 1)], 0.0),
        ConstraintRelation::GreaterEqual, 5.0), "cover")).unwrap();
    source.try_to_linear_triad_model().unwrap()
}

// 子问题单步需要 `scip`；零后端特性时该入口明确返回 `UnsupportedDialect`，所以这里按特性门控。
// The subproblem step needs `scip`; with no backend feature the entry returns `UnsupportedDialect`, hence the gate.
#[cfg(feature = "scip")]
#[test]
fn benders_sub_entry_fixes_leading_columns_and_reports_duals() {
    use ospf_rust_bridge::{BendersSubOutcome, BendersSubReport};
    let engine = NativeEngine::default();
    let submit = |master_solution: Vec<f64>| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::LinearBendersSub);
        request.model = ospf_rust_bridge::encode_model(&benders_sub_model()).unwrap();
        request.master_solution = Some(master_solution);
        engine.submit(request).unwrap()
    };
    // 1) 固定 x=8：子问题可行，且只固定了 1 列（y 保持自由——这是位置固定的真实行为）。
    // Fixing x=8: the subproblem is feasible and exactly 1 column was fixed (y stays free — the measured positional
    // behaviour).
    let view = submit(vec![8.0]);
    let finished = settle(&engine, &view);
    assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
    let report: BendersSubReport = serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
    assert_eq!(report.outcome, BendersSubOutcome::Feasible);
    assert_eq!(report.fixed_columns, 1);
    // 无 cut 上下文时框架不产出 cut，桥接如实报 0 而不是伪造一条。
    // Without a cut context the framework produces no cuts, and the bridge reports 0 instead of fabricating one.
    assert_eq!(report.generated_cut_count, 0);
    // 对偶值必须被交出来（否则调用方无法构造任何 cut）。
    // The duals must be handed out (otherwise the caller cannot build any cut).
    assert!(!report.constraint_duals.is_empty(), "feasible step returned no constraint duals");
    assert!(report.constraint_duals.iter().all(|value| value.is_finite()));
    engine.release(&view.id).unwrap();
    // 2) 固定 x=0：`x + y >= 5` 与 `y <= 2` 冲突，子问题不可行。
    // Fixing x=0: `x + y >= 5` conflicts with `y <= 2`, so the subproblem is infeasible.
    let view = submit(vec![0.0]);
    let finished = settle(&engine, &view);
    assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
    let report: BendersSubReport = serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
    assert_eq!(report.outcome, BendersSubOutcome::Infeasible);
    assert_eq!(report.fixed_columns, 1);
    assert_eq!(report.generated_cut_count, 0);
    engine.release(&view.id).unwrap();
    // 3) 解向量比列数长：说明调用方把子问题列也塞进来了，必须被拒。
    // A solution longer than the column count means subproblem columns were included; it must be refused.
    let mut broken = base_request();
    broken.model_kind = Some(ModelKind::LinearBendersSub);
    broken.model = ospf_rust_bridge::encode_model(&benders_sub_model()).unwrap();
    broken.master_solution = Some(vec![1.0, 2.0, 3.0]);
    assert_eq!(engine.submit(broken).unwrap_err().code, "InvalidInput");
    // 4) 缺解向量在入口被拒。 / A missing solution is refused at the entry.
    let mut missing = base_request();
    missing.model_kind = Some(ModelKind::LinearBendersSub);
    missing.model = ospf_rust_bridge::encode_model(&benders_sub_model()).unwrap();
    assert_eq!(engine.submit(missing).unwrap_err().code, "InvalidInput");
}

#[test]
fn lbb_plan_entry_validates_binds_assumptions_and_reports_why_it_cannot_assign() {
    use ospf_rust_bridge::{CpAssumptionSpec, LogicBasedBendersPlanReport, LogicBasedBendersPlanRequest,
        LogicBasedBendersRequest, MasterVariableDomain, MasterVariableSpec};
    let engine = NativeEngine::default();
    let benders = || LogicBasedBendersRequest {
        master_variables: vec![MasterVariableSpec { id: "x".into(), domain: MasterVariableDomain::Binary,
            lower: None, upper: None }],
        cuts: Vec::new(),
        assumptions: vec![CpAssumptionSpec { variable: "subproblem/selected".into(), master_variable: "x".into(),
            value: 1 }],
        max_iterations: None,
    };
    let submit = |master_report: Option<String>| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::LogicBasedBendersPlan);
        request.logic_based_benders_plan = Some(LogicBasedBendersPlanRequest { benders: benders(), master_report });
        engine.submit(request).unwrap()
    };
    // 1) 不带报告：校验通过、assumption 绑定可用、没有 assignment（也没有"编造的错误"）。
    // Without a report: validation passes, bindings are available and there is no assignment (and no invented error).
    let view = submit(None);
    let finished = settle(&engine, &view);
    assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
    let report: LogicBasedBendersPlanReport = serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
    assert!(report.valid);
    assert_eq!(report.assumption_count, 1);
    assert_eq!(report.cut_count, 0);
    assert_eq!(report.assumption_bindings.len(), 1);
    assert_eq!(report.assumption_bindings[0].variable, "subproblem/selected");
    assert_eq!(report.assumption_bindings[0].master_variable, "x");
    assert_eq!(report.master_assignment, None);
    assert_eq!(report.master_assignment_error, None);
    engine.release(&view.id).unwrap();
    // 2) 带一份**真实求解器产出**的报告：SDK/native 的 SCIP 报告没有稳定值表，所以 assignment 提取会失败，
    //    桥接必须把原因如实交回（而不是按下标猜一个赋值）。
    // With a report produced by a **real solver**: our SCIP reports carry no stable-value table, so extraction fails and
    // the bridge must hand the reason back (instead of guessing positionally).
    #[cfg(feature = "scip")]
    {
        use ospf_rust_core::model::{ConstraintRelation, MetaModel};
        use ospf_rust_core::variable::{ContinuousVariableItem, VariableRange};
        let mut source = MetaModel::<f64>::new("lbb-plan-master");
        source.add_variable(ContinuousVariableItem::auto_with_range("x",
            VariableRange::new(Some(0.0), Some(1.0)))).unwrap();
        let artifact = ospf_rust_bridge::encode_model(&source.try_to_linear_triad_model().unwrap()).unwrap();
        let mut solve = base_request();
        solve.model = artifact;
        solve.model_kind = Some(ModelKind::Linear);
        let solved = settle(&engine, &engine.submit(solve).unwrap());
        assert_eq!(solved.status, JobStatus::Completed, "{:?}", solved.error);
        let real_report = solved.report_json.clone().unwrap();
        engine.release(&solved.id).unwrap();

        let view = submit(Some(real_report));
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: LogicBasedBendersPlanReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        assert_eq!(report.master_assignment, None);
        let reason = report.master_assignment_error.expect("a report without stable values must say so");
        assert!(reason.contains("stable variable"), "{reason}");
        engine.release(&view.id).unwrap();
    }
    // 3) 校验失败是硬失败：assumption 指向未声明的 master 变量。 / A validation failure is a hard failure.
    let mut broken = base_request();
    broken.model_kind = Some(ModelKind::LogicBasedBendersPlan);
    broken.logic_based_benders_plan = Some(LogicBasedBendersPlanRequest { benders: {
        let mut b = benders();
        b.assumptions[0].master_variable = "ghost".into();
        b
    }, master_report: None });
    assert_eq!(engine.submit(broken).unwrap_err().code, "InvalidInput");
    // 4) 缺请求在入口被拒。 / A missing request is refused at the entry.
    let mut missing = base_request();
    missing.model_kind = Some(ModelKind::LogicBasedBendersPlan);
    assert_eq!(engine.submit(missing).unwrap_err().code, "InvalidInput");
}

#[test]
fn remote_artifact_codec_entry_decodes_validates_and_echoes_identity() {
    use ospf_rust_bridge::{RemoteArtifactKind, RemoteArtifactReport, RemoteArtifactRequest};
    let engine = NativeEngine::default();
    let submit = |payload: String| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::RemoteArtifactCodec);
        request.remote_artifact = Some(RemoteArtifactRequest {
            kind: RemoteArtifactKind::CheckpointArtifact, payload,
            operation: ospf_rust_bridge::RemoteCodecOperation::Decode, expected: None });
        engine.submit(request).unwrap()
    };

    // 真实载荷：用框架构造器 + 框架编码器产出，不经任何手写 JSON。
    // Real payload: produced by the framework's constructors and encoder, with no hand-written JSON.
    use ospf_rust_core::solver::checkpoint::{SolveCheckpoint, SolveCheckpointArtifact};
    use ospf_rust_core::solver::fingerprint::sha256_fingerprint;
    use ospf_rust_core::solver::report::{AuditFingerprint, SolverProvenance,
        CURRENT_SOLVE_REPORT_SCHEMA_VERSION};
    let fingerprint = |value: &str| AuditFingerprint {
        schema_version: CURRENT_SOLVE_REPORT_SCHEMA_VERSION.to_owned(),
        algorithm: "sha256".to_owned(), value: value.to_owned() };
    let state = br#"{"iteration":3,"best":[4,5]}"#.to_vec();
    let checkpoint = SolveCheckpoint::new(
        "run-codec", "attempt-codec", None,
        fingerprint("model-c"), fingerprint("config-c"), fingerprint("solver-c"),
        SolverProvenance { solver_id: "scip".into(), backend_name: "scip".into(),
            backend_version: Some("8.0".into()), thread_count: Some(1), ..Default::default() },
        3, Some(4.0), Some(5.0), Some(0.25),
        sha256_fingerprint("ospf.solve.checkpoint.state", &state),
    ).unwrap();
    let artifact = SolveCheckpointArtifact::new(checkpoint, state).unwrap();
    let payload = String::from_utf8(
        ospf_rust_framework::solver::remote::ospf_serializer::checkpoint_artifact_to_json(&artifact).unwrap())
        .unwrap();
    let expected_digest = format!("sha256:{}", artifact.checkpoint.state_digest.value);

    let view = submit(payload.clone());
    let finished = settle(&engine, &view);
    assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
    let report: RemoteArtifactReport = serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
    assert_eq!(report.kind, RemoteArtifactKind::CheckpointArtifact);
    assert_eq!(report.run_id.as_deref(), Some("run-codec"));
    assert_eq!(report.attempt_id.as_deref(), Some("attempt-codec"));
    let identity = report.checkpoint.as_ref().expect("checkpoint identity");
    assert_eq!(identity.iteration, 3);
    assert_eq!(identity.state_digest, expected_digest);
    assert!(identity.state_bytes > 0);
    assert_eq!(report.digest, None);
    assert!(report.canonical);
    assert_eq!(report.canonical_payload, payload);
    assert_eq!(report.byte_length, payload.len() as u32);
    engine.release(&view.id).unwrap();

    // 给 SDK 侧留一份**真实**载荷 fixture（框架构造器 + 框架编码器产出），fixture 目录未配置时不写。
    // Leave a **real** payload fixture for the SDK side (produced by the framework constructors and encoder); nothing is
    // written when the fixture directory is not configured.
    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(directory.join("checkpoint.json"), payload.as_bytes()).unwrap();
    }

    // 篡改 `gap`：整封包完整性摘要失配 → 入口拒绝，而不是回显假身份。
    // Tampering with `gap` breaks the envelope integrity digest → the entry refuses instead of echoing a fake identity.
    let tampered = payload.replace("\"gap\":\"0.25\"", "\"gap\":\"0.5\"");
    assert_ne!(tampered, payload, "fixture must actually change the payload");
    let mut broken = base_request();
    broken.model_kind = Some(ModelKind::RemoteArtifactCodec);
    broken.remote_artifact = Some(RemoteArtifactRequest {
        kind: RemoteArtifactKind::CheckpointArtifact, payload: tampered,
        operation: ospf_rust_bridge::RemoteCodecOperation::Decode, expected: None });
    assert_eq!(engine.submit(broken).unwrap_err().code, "InvalidInput");


    // 恢复校验走同一入口：身份一致通过，run 或指纹不符被拒（拒绝带领域原因，不是布尔值）。
    // The resume check goes through the same entry: a matching identity passes, a wrong run or fingerprint is refused
    // (the refusal carries the domain reason and is not a boolean).
    let expectation = |run: &str, model: &str| ospf_rust_bridge::RemoteResumeExpectation {
        run_id: run.to_owned(),
        model_fingerprint: ospf_rust_bridge::RemoteFingerprintSpec {
            algorithm: "sha256".into(), value: model.to_owned() },
        configuration_fingerprint: ospf_rust_bridge::RemoteFingerprintSpec {
            algorithm: "sha256".into(), value: "config-c".into() },
        solver_fingerprint: ospf_rust_bridge::RemoteFingerprintSpec {
            algorithm: "sha256".into(), value: "solver-c".into() },
    };
    let model_value = artifact.checkpoint.model_fingerprint.value.clone();
    let resume = |run: &str, model: &str| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::RemoteArtifactCodec);
        request.remote_artifact = Some(RemoteArtifactRequest {
            kind: RemoteArtifactKind::CheckpointArtifact, payload: payload.clone(),
            operation: ospf_rust_bridge::RemoteCodecOperation::Resume,
            expected: Some(expectation(run, model)) });
        engine.submit(request)
    };
    let resumed = settle(&engine, &resume("run-codec", &model_value).unwrap());
    assert_eq!(resumed.status, JobStatus::Completed, "{:?}", resumed.error);
    let report: RemoteArtifactReport = serde_json::from_str(resumed.report_json.as_deref().unwrap()).unwrap();
    assert_eq!(report.operation, ospf_rust_bridge::RemoteCodecOperation::Resume);
    assert_eq!(report.run_id.as_deref(), Some("run-codec"));
    engine.release(&resumed.id).unwrap();
    assert_eq!(resume("run-other", &model_value).unwrap_err().code, "InvalidInput");
    assert_eq!(resume("run-codec", "another-model").unwrap_err().code, "InvalidInput");

    // resume 缺期望身份：入口拒绝，而不是降级成 decode。 / A resume without expectations is refused, not downgraded.
    let mut no_expectation = base_request();
    no_expectation.model_kind = Some(ModelKind::RemoteArtifactCodec);
    no_expectation.remote_artifact = Some(RemoteArtifactRequest {
        kind: RemoteArtifactKind::CheckpointArtifact, payload: payload.clone(),
        operation: ospf_rust_bridge::RemoteCodecOperation::Resume, expected: None });
    assert_eq!(engine.submit(no_expectation).unwrap_err().code, "InvalidInput");


    // CP 结果与 CP 检查点：线格式同样由框架自己的编码器产出（快照走核心的构造器），不手写 JSON。
    // The CP result and CP checkpoint: the wire forms are likewise produced by the framework's encoders (the snapshot via
    // the core's constructors), never hand-written JSON.
    {
        use ospf_rust_core::model::constraint_programming::{
            ConstraintDefinition, ConstraintProgrammingConstraint, ConstraintProgrammingModel, IntegerDomain,
            IntegerExpression, IntegerObjective, IntegerRelation, IntegerVariable,
        };
        use ospf_rust_core::solver::checkpoint::{SolveCheckpoint, SolveCheckpointArtifact as CoreArtifact};
        use ospf_rust_core::solver::fingerprint::sha256_fingerprint;
        use ospf_rust_core::solver::report::{
            ProblemStatus, SolveFingerprints, SolveProof, SolveReport, SolverProvenance, TerminationReason,
            CURRENT_SOLVE_REPORT_SCHEMA_VERSION,
        };
        use ospf_rust_core::ConstraintProgrammingCheckpointArtifact;
        use ospf_rust_framework::solver::remote::ospf_serializer::{
            constraint_programming_checkpoint_to_json, constraint_programming_result_to_json,
        };

        let x = IntegerVariable::new("x");
        let mut model = ConstraintProgrammingModel::new("native-remote-codec-cp");
        model.register_variable(x.clone(), IntegerDomain::range(0, 5).unwrap()).unwrap();
        model.add_constraint(ConstraintDefinition::new("capacity",
            ConstraintProgrammingConstraint::integer(
                IntegerExpression::variable(x.clone()), IntegerRelation::LessOrEqual, 5))).unwrap();
        model.set_objective(IntegerObjective::maximize(IntegerExpression::variable(x)));
        let snapshot = model.freeze().unwrap();

        let report = SolveReport::<i64>::builder(ProblemStatus::Infeasible, TerminationReason::Completed)
            .proof(SolveProof::infeasibility())
            .fingerprints(SolveFingerprints { model: Some(snapshot.fingerprint.clone()),
                ..SolveFingerprints::default() })
            .build().unwrap();
        let result_payload = String::from_utf8(constraint_programming_result_to_json(
            &snapshot, report, Some("run-cp".into()), Some("attempt-cp".into())).unwrap()).unwrap();

        let state = br#"{"iteration":4}"#.to_vec();
        let fingerprint = |value: &str| ospf_rust_core::solver::report::AuditFingerprint {
            schema_version: CURRENT_SOLVE_REPORT_SCHEMA_VERSION.to_owned(),
            algorithm: "sha256".to_owned(), value: value.to_owned() };
        let checkpoint = SolveCheckpoint::new(
            "run-cp", "attempt-cp", None, snapshot.fingerprint.clone(),
            fingerprint("config-cp"), fingerprint("solver-cp"),
            SolverProvenance { solver_id: "cp".into(), backend_name: "cp".into(),
                thread_count: Some(1), ..Default::default() },
            4, Some(2.0), Some(3.0), Some(0.5),
            sha256_fingerprint("ospf.solve.checkpoint.state", &state),
        ).unwrap();
        let cp_checkpoint = ConstraintProgrammingCheckpointArtifact::new_direct(
            checkpoint, &snapshot, Vec::new(), Default::default(), Default::default()).unwrap();
        let cp_checkpoint_payload = String::from_utf8(
            constraint_programming_checkpoint_to_json(&cp_checkpoint).unwrap()).unwrap();
        let _ = CoreArtifact::new;

        let decode = |kind, payload: String| {
            let mut request = base_request();
            request.model_kind = Some(ModelKind::RemoteArtifactCodec);
            request.remote_artifact = Some(RemoteArtifactRequest { kind, payload,
                operation: ospf_rust_bridge::RemoteCodecOperation::Decode, expected: None });
            engine.submit(request)
        };

        // CP 结果：有整封包摘要、没有检查点身份。 / The CP result: envelope digest, no checkpoint identity.
        let finished = settle(&engine, &decode(RemoteArtifactKind::ConstraintProgrammingResult,
            result_payload.clone()).unwrap());
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: RemoteArtifactReport = serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        assert_eq!(report.kind, RemoteArtifactKind::ConstraintProgrammingResult);
        assert_eq!(report.run_id.as_deref(), Some("run-cp"));
        assert_eq!(report.checkpoint, None);
        assert!(report.digest.as_deref().unwrap().starts_with("sha256:"));
        engine.release(&finished.id).unwrap();

        // CP 检查点：有检查点身份、没有整封包摘要。 / The CP checkpoint: checkpoint identity, no envelope digest.
        let finished = settle(&engine, &decode(RemoteArtifactKind::ConstraintProgrammingCheckpoint,
            cp_checkpoint_payload.clone()).unwrap());
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: RemoteArtifactReport = serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        assert_eq!(report.kind, RemoteArtifactKind::ConstraintProgrammingCheckpoint);
        let identity = report.checkpoint.as_ref().expect("checkpoint identity");
        assert_eq!(identity.iteration, 4);
        assert!(identity.state_bytes > 0);
        assert_eq!(report.digest, None);
        engine.release(&finished.id).unwrap();

        // 尾部空白破坏 CP 结果的规范形式 → 拒绝。 / A trailing space breaks the CP result's canonical form → refused.
        assert_eq!(decode(RemoteArtifactKind::ConstraintProgrammingResult,
            format!("{result_payload} ")).unwrap_err().code, "InvalidInput");
        // CP 检查点的恢复校验尚未绑定 → 明确 UnsupportedDialect，不静默当 decode。
        // The CP checkpoint's resume check is not bound → an explicit UnsupportedDialect, not a silent decode.
        let mut resume = base_request();
        resume.model_kind = Some(ModelKind::RemoteArtifactCodec);
        resume.remote_artifact = Some(RemoteArtifactRequest {
            kind: RemoteArtifactKind::ConstraintProgrammingCheckpoint, payload: cp_checkpoint_payload.clone(),
            operation: ospf_rust_bridge::RemoteCodecOperation::Resume, expected: None });
        assert_eq!(engine.submit(resume).unwrap_err().code, "UnsupportedDialect");

        // 给 SDK 侧留真实载荷 fixture。 / Leave the real payload fixtures for the SDK side.
        if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
            let directory = std::path::PathBuf::from(directory);
            std::fs::create_dir_all(&directory).unwrap();
            std::fs::write(directory.join("cp-result.json"), result_payload.as_bytes()).unwrap();
            std::fs::write(directory.join("cp-checkpoint.json"), cp_checkpoint_payload.as_bytes()).unwrap();
        }
    }

    // LBB 检查点：外层是通用检查点、内层是类型化 state，构造它必须显式补 master 指纹并让迭代号与快照自洽。
    // The LBB checkpoint: a generic envelope around a typed state; constructing it requires an explicit master fingerprint
    // and a self-consistent iteration.
    {
        use ospf_rust_core::model::ObjectiveCategory;
        use ospf_rust_core::solver::checkpoint::SolveCheckpoint;
        use ospf_rust_core::solver::fingerprint::sha256_fingerprint;
        use ospf_rust_core::solver::report::{
            AuditFingerprint, SolverProvenance, CURRENT_SOLVE_REPORT_SCHEMA_VERSION,
        };
        use ospf_rust_framework::solver::logic_based_benders::{
            LogicBasedBendersMode, MasterBinding, MasterVariableBinding,
        };
        use ospf_rust_framework::solver::logic_based_benders_checkpoint::{
            LogicBasedBendersCheckpointArtifact, LogicBasedBendersCheckpointState,
        };
        use ospf_rust_framework::solver::remote::ospf_serializer::logic_based_benders_checkpoint_to_json;

        let fingerprint = |value: &str| AuditFingerprint {
            schema_version: CURRENT_SOLVE_REPORT_SCHEMA_VERSION.to_owned(),
            algorithm: "sha256".to_owned(), value: value.to_owned() };
        let binding = MasterBinding::new([MasterVariableBinding::binary("master/use-cp")]).unwrap();
        let mut state = LogicBasedBendersCheckpointState::new(
            &binding, LogicBasedBendersMode::Heuristic, 1e-9, ObjectiveCategory::Minimum,
            Vec::new(), Vec::new(), Vec::new(), 0, false, fingerprint("model-lbb"), None, None,
        ).unwrap();
        state.master_fingerprint = Some(fingerprint("master-lbb"));
        let raw = br#"{"iteration":0}"#.to_vec();
        let checkpoint = SolveCheckpoint::new(
            "run-lbb", "attempt-lbb", None, fingerprint("model-lbb"), fingerprint("config-lbb"),
            fingerprint("solver-lbb"),
            SolverProvenance { solver_id: "scip".into(), backend_name: "scip".into(),
                thread_count: Some(1), ..Default::default() },
            0, Some(1.0), Some(2.0), Some(1.0),
            sha256_fingerprint("ospf.solve.checkpoint.state", &raw),
        ).unwrap();
        let artifact = LogicBasedBendersCheckpointArtifact::new(checkpoint, state).unwrap();
        let payload = String::from_utf8(logic_based_benders_checkpoint_to_json(&artifact).unwrap()).unwrap();

        let mut request = base_request();
        request.model_kind = Some(ModelKind::RemoteArtifactCodec);
        request.remote_artifact = Some(RemoteArtifactRequest {
            kind: RemoteArtifactKind::LogicBasedBendersCheckpoint, payload: payload.clone(),
            operation: ospf_rust_bridge::RemoteCodecOperation::Decode, expected: None });
        let finished = settle(&engine, &engine.submit(request).unwrap());
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: RemoteArtifactReport = serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        assert_eq!(report.kind, RemoteArtifactKind::LogicBasedBendersCheckpoint);
        assert_eq!(report.run_id.as_deref(), Some("run-lbb"));
        let identity = report.checkpoint.as_ref().expect("checkpoint identity");
        assert!(identity.state_bytes > 0);
        assert_eq!(report.digest, None);
        engine.release(&finished.id).unwrap();

        // 恢复校验未绑定 → 明确 UnsupportedDialect。 / Resume is not bound → explicit UnsupportedDialect.
        let mut resume = base_request();
        resume.model_kind = Some(ModelKind::RemoteArtifactCodec);
        resume.remote_artifact = Some(RemoteArtifactRequest {
            kind: RemoteArtifactKind::LogicBasedBendersCheckpoint, payload: payload.clone(),
            operation: ospf_rust_bridge::RemoteCodecOperation::Resume, expected: None });
        assert_eq!(engine.submit(resume).unwrap_err().code, "UnsupportedDialect");

        if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
            let directory = std::path::PathBuf::from(directory);
            std::fs::create_dir_all(&directory).unwrap();
            std::fs::write(directory.join("lbb-checkpoint.json"), payload.as_bytes()).unwrap();
        }
    }
    // 缺请求在入口被拒。 / A missing request is refused at the entry.
    let mut missing = base_request();
    missing.model_kind = Some(ModelKind::RemoteArtifactCodec);
    assert_eq!(engine.submit(missing).unwrap_err().code, "InvalidInput");
}

#[test]
fn conflict_explanation_verify_entry_summarizes_and_refuses_broken_reports() {
    use ospf_rust_bridge::ConflictExplanationSummary;
    use ospf_rust_core::analysis::{
        AnalysisStatus, ConflictExplanation, ConflictExtractionTier, ConflictGroupSummary,
        ConflictMinimality, ConflictValidity, DiagnosticSource, ObjectiveTarget, TargetFeasibilityReport,
    };
    use ospf_rust_core::solver::report::{ProofStatus, StableConstraintId};

    let schema = ospf_rust_core::analysis::conflict::CONFLICT_REPORT_SCHEMA_VERSION;
    let target = ObjectiveTarget::at_least("objective", 5.0).unwrap();
    let explanation = || ConflictExplanation {
        schema_version: schema.to_owned(),
        target: target.clone(),
        target_feasibility: TargetFeasibilityReport {
            schema_version: schema.to_owned(),
            target: target.clone(),
            effective_integer_bound: None,
            source: DiagnosticSource::ObjectiveTarget { target: target.clone() },
            status: AnalysisStatus::Unreachable, solution: None, exact_objective: None, objective_value: None,
            proof_status: ProofStatus::Verified, termination_reason: None, solve_time: None,
            target_fixed: true, message: None,
        },
        status: AnalysisStatus::Unreachable,
        available_evidence: Vec::new(),
        fixed_background: Vec::new(),
        members: vec![DiagnosticSource::Constraint { id: StableConstraintId("capacity".to_owned()) }],
        target_source: DiagnosticSource::ObjectiveTarget { target: target.clone() },
        validity: ConflictValidity::Verified,
        minimality: ConflictMinimality::Irreducible,
        verifications: Vec::new(),
        group_summaries: vec![ConflictGroupSummary { group: Some("capacity".to_owned()), count: 1 }],
        resolve_count: 2, memoized_checks: 1,
        extraction_tier: ConflictExtractionTier::AssumptionExtraction,
        unavailable_reason: None, background_blocking: false,
    };

    let engine = NativeEngine::default();
    let verify = |payload: String| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::ConflictExplanationVerify);
        request.conflict_explanation = Some(payload);
        engine.submit(request)
    };

    let payload = serde_json::to_string(&explanation()).unwrap();
    let view = verify(payload.clone()).unwrap();
    let finished = settle(&engine, &view);
    assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
    let summary: ConflictExplanationSummary =
        serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
    assert_eq!(summary.status, "unreachable");
    assert_eq!(summary.validity, "verified");
    assert_eq!(summary.minimality, "irreducible");
    assert_eq!(summary.member_count, 1);
    assert_eq!(summary.constraint_ids, vec!["capacity".to_owned()]);
    assert_eq!(summary.resolve_count, 2);
    assert_eq!(summary.memoized_checks, 1);
    assert_eq!(summary.extraction_tier, "AssumptionExtraction");
    assert!(!summary.background_blocking);
    engine.release(&view.id).unwrap();

    let mut duplicate = explanation();
    duplicate.members.push(DiagnosticSource::Constraint { id: StableConstraintId("capacity".to_owned()) });
    let error = verify(serde_json::to_string(&duplicate).unwrap()).unwrap_err();
    assert_eq!(error.code, "InvalidInput");
    assert!(error.message.contains("duplicate"), "{}", error.message);

    let mut missing = base_request();
    missing.model_kind = Some(ModelKind::ConflictExplanationVerify);
    assert_eq!(engine.submit(missing).unwrap_err().code, "InvalidInput");
    assert_eq!(verify("not json".to_owned()).unwrap_err().code, "InvalidInput");

    // 给 SDK 侧留真实载荷 fixture（领域 serde 序列化，不手写 JSON）。
    // Leave the real payload fixture for the SDK side (domain serde serialization, no hand-written JSON).
    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(directory.join("conflict-explanation.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn candidate_funnel_verify_entry_summarizes_and_shortlists() {
    use ospf_rust_bridge::CandidateFunnelReport;
    use ospf_rust_core::analysis::activity::ActivityStatus;
    use ospf_rust_core::analysis::pipeline::{
        CandidateFunnelRanking, CandidateTier, ConstraintCandidate, CRITICAL_ANALYSIS_REPORT_SCHEMA_VERSION,
    };
    use ospf_rust_core::solver::StableConstraintId;

    let candidate = |priority: usize, id: &str, tier, activity, dual| ConstraintCandidate {
        constraint_id: StableConstraintId(id.to_owned()), group: Some("g".to_owned()), tier, activity,
        slack: Some(0.0), dual_value: dual, local_effective: None, priority,
    };
    let ranking = || CandidateFunnelRanking {
        schema_version: CRITICAL_ANALYSIS_REPORT_SCHEMA_VERSION.to_owned(),
        dual_threshold: 0.5,
        candidates: vec![
            candidate(0, "hot", CandidateTier::TierA, ActivityStatus::Active, Some(1.0)),
            candidate(1, "warm", CandidateTier::TierB, ActivityStatus::Active, Some(0.0)),
            candidate(2, "cold", CandidateTier::TierC, ActivityStatus::Inactive, None),
        ],
        tier_a: 1, tier_b: 1, tier_c: 1, unclassified: 0,
    };

    let engine = NativeEngine::default();
    let verify = |payload: String, limit: Option<u32>| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::CandidateFunnelVerify);
        request.candidate_funnel = Some(payload);
        request.shortlist_limit = limit;
        engine.submit(request)
    };

    let payload = serde_json::to_string(&ranking()).unwrap();
    let view = verify(payload.clone(), Some(2)).unwrap();
    let finished = settle(&engine, &view);
    assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
    let report: CandidateFunnelReport =
        serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
    assert_eq!(report.candidate_count, 3);
    assert_eq!((report.tier_a, report.tier_b, report.tier_c, report.unclassified), (1, 1, 1, 0));
    assert_eq!(report.shortlist_limit, Some(2));
    assert_eq!(report.shortlist.iter().map(|entry| entry.constraint_id.as_str()).collect::<Vec<_>>(),
        vec!["hot", "warm"]);
    assert_eq!(report.shortlist[0].tier, "A");
    engine.release(&view.id).unwrap();

    // 不给上限：没有短名单，但摘要照旧。 / No limit: no shortlist but the summary is unchanged.
    let view = verify(payload.clone(), None).unwrap();
    let finished = settle(&engine, &view);
    let report: CandidateFunnelReport =
        serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
    assert_eq!(report.shortlist_limit, None);
    assert!(report.shortlist.is_empty());
    engine.release(&view.id).unwrap();

    // 名额足够时 Tier C 才被补进短名单，顺序在 A/B 之后。 / Tier C only joins the shortlist with room, after A/B.
    let view = verify(payload.clone(), Some(5)).unwrap();
    let finished = settle(&engine, &view);
    let report: CandidateFunnelReport =
        serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
    assert_eq!(report.shortlist.iter().map(|entry| entry.constraint_id.as_str()).collect::<Vec<_>>(),
        vec!["hot", "warm", "cold"]);
    engine.release(&view.id).unwrap();

    // 稀疏优先级：领域原因原样带出。 / Sparse priorities: the domain reason is relayed verbatim.
    let mut broken = ranking();
    broken.candidates[1].priority = 7;
    let error = verify(serde_json::to_string(&broken).unwrap(), None).unwrap_err();
    assert_eq!(error.code, "InvalidInput");
    assert!(error.message.contains("dense"), "{}", error.message);

    // 缺请求 / 非 JSON：入口拒绝。 / A missing request / non-JSON: refused at the entry.
    let mut missing = base_request();
    missing.model_kind = Some(ModelKind::CandidateFunnelVerify);
    assert_eq!(engine.submit(missing).unwrap_err().code, "InvalidInput");
    assert_eq!(verify("not json".to_owned(), None).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(directory.join("candidate-funnel.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn effectiveness_verify_entry_projects_both_views() {
    use ospf_rust_bridge::EffectivenessReport;
    use ospf_rust_core::analysis::activity::ActivityStatus;
    use ospf_rust_core::analysis::pipeline::{
        CandidateTier, EffectivenessEntry, EffectivenessRanking, CRITICAL_ANALYSIS_REPORT_SCHEMA_VERSION,
    };
    use ospf_rust_core::AnalysisStatus;
    use ospf_rust_core::solver::StableConstraintId as ConstraintId;

    let entry = |id: &str, tier| EffectivenessEntry {
        constraint_id: ConstraintId(id.to_owned()), group: Some("g".to_owned()), tier,
        activity: ActivityStatus::Active, dual_value: Some(1.0), local_effective: None,
        removal_effective: None, first_effective_delta: None, max_improvement: None,
        marginal_effect: None, status: AnalysisStatus::Unknown, unavailable_reason: None,
    };
    let ranking = || {
        let mut hot = entry("hot", CandidateTier::TierA);
        hot.local_effective = Some(true); hot.max_improvement = Some(0.5);
        hot.first_effective_delta = Some(0.1); hot.removal_effective = Some(true);
        hot.status = AnalysisStatus::Reachable;
        let mut warm = entry("warm", CandidateTier::TierB);
        warm.max_improvement = Some(0.0); warm.removal_effective = Some(true);
        warm.status = AnalysisStatus::Reachable;
        let mut cold = entry("cold", CandidateTier::TierC);
        cold.dual_value = None; cold.status = AnalysisStatus::Unsupported;
        cold.unavailable_reason = Some("no dual".to_owned());
        EffectivenessRanking {
            schema_version: CRITICAL_ANALYSIS_REPORT_SCHEMA_VERSION.to_owned(),
            baseline_objective: 10.0, entries: vec![hot, warm, cold],
        }
    };

    let engine = NativeEngine::default();
    let verify = |payload: String| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::EffectivenessVerify);
        request.effectiveness_ranking = Some(payload);
        engine.submit(request)
    };

    let payload = serde_json::to_string(&ranking()).unwrap();
    let view = verify(payload.clone()).unwrap();
    let finished = settle(&engine, &view);
    assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
    let report: EffectivenessReport =
        serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
    assert_eq!(report.entry_count, 3);
    assert_eq!(report.globally_effective_ids, vec!["hot".to_owned()]);
    assert_eq!(report.locally_effective_ids, vec!["hot".to_owned()]);
    assert_eq!(report.rows[1].max_improvement, Some(0.0));
    assert!(!report.rows[1].globally_effective);
    assert_eq!(report.rows[2].status, "unsupported");
    engine.release(&view.id).unwrap();

    // 重复约束：领域原因原样带出。 / A duplicate constraint: the domain reason is relayed verbatim.
    let mut broken = ranking();
    broken.entries[1].constraint_id = ConstraintId("hot".to_owned());
    let error = verify(serde_json::to_string(&broken).unwrap()).unwrap_err();
    assert_eq!(error.code, "InvalidInput");
    assert!(error.message.contains("duplicate"), "{}", error.message);

    let mut missing = base_request();
    missing.model_kind = Some(ModelKind::EffectivenessVerify);
    assert_eq!(engine.submit(missing).unwrap_err().code, "InvalidInput");
    assert_eq!(verify("not json".to_owned()).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(directory.join("effectiveness-ranking.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn critical_analysis_verify_entry_summarizes_stage_presence_and_blocking() {
    use ospf_rust_bridge::CriticalAnalysisReport;
    use ospf_rust_core::analysis::pipeline::{
        AnalysisBaselineSummary, CriticalConstraintAnalysisReport, CRITICAL_ANALYSIS_REPORT_SCHEMA_VERSION,
    };
    use ospf_rust_core::analysis::{
        AnalysisStatus, ConflictExplanation, ConflictExtractionTier, ConflictGroupSummary,
        ConflictMinimality, ConflictValidity, DiagnosticSource, ObjectiveTarget, TargetFeasibilityReport,
    };
    use ospf_rust_core::solver::report::{ProofStatus, StableConstraintId};

    let target = ObjectiveTarget::at_least("objective", 5.0).unwrap();
    let conflict = || ConflictExplanation {
        schema_version: ospf_rust_core::analysis::conflict::CONFLICT_REPORT_SCHEMA_VERSION.to_owned(),
        target: target.clone(),
        target_feasibility: TargetFeasibilityReport {
            schema_version: CRITICAL_ANALYSIS_REPORT_SCHEMA_VERSION.to_owned(),
            target: target.clone(), effective_integer_bound: None,
            source: DiagnosticSource::ObjectiveTarget { target: target.clone() },
            status: AnalysisStatus::Unreachable, solution: None, exact_objective: None,
            objective_value: None, proof_status: ProofStatus::Verified, termination_reason: None,
            solve_time: None, target_fixed: true, message: None,
        },
        status: AnalysisStatus::Unreachable,
        available_evidence: Vec::new(), fixed_background: Vec::new(),
        members: vec![DiagnosticSource::Constraint { id: StableConstraintId("capacity".to_owned()) }],
        target_source: DiagnosticSource::ObjectiveTarget { target: target.clone() },
        validity: ConflictValidity::Verified, minimality: ConflictMinimality::Irreducible,
        verifications: Vec::new(),
        group_summaries: vec![ConflictGroupSummary { group: Some("capacity".to_owned()), count: 1 }],
        resolve_count: 2, memoized_checks: 0,
        extraction_tier: ConflictExtractionTier::AssumptionExtraction,
        unavailable_reason: None, background_blocking: false,
    };
    let report = || CriticalConstraintAnalysisReport {
        schema_version: CRITICAL_ANALYSIS_REPORT_SCHEMA_VERSION.to_owned(),
        baseline: AnalysisBaselineSummary {
            objective_id: "objective".to_owned(), sense: "MINIMUM".to_owned(),
            optimal_value: None, status: AnalysisStatus::Unreachable, proven_optimal: false,
        },
        activity: None, local_sensitivity: None, funnel: None, effectiveness: None,
        target: None, conflict: Some(conflict()),
        group_summaries: Vec::new(), status: AnalysisStatus::Unreachable,
        unavailable_reasons: vec!["activity stage was not run".to_owned()],
    };

    let engine = NativeEngine::default();
    let verify = |payload: String| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::CriticalAnalysisVerify);
        request.critical_analysis = Some(payload);
        engine.submit(request)
    };

    let payload = serde_json::to_string(&report()).unwrap();
    let view = verify(payload.clone()).unwrap();
    let finished = settle(&engine, &view);
    assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
    let summary: CriticalAnalysisReport =
        serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
    assert_eq!(summary.status, "unreachable");
    assert!(summary.stages.conflict);
    assert!(!summary.stages.activity, "a missing stage stays visibly missing");
    assert!(summary.has_minimal_blocking_set);
    assert!(summary.blocking_summary.contains("最小阻塞集合"), "{}", summary.blocking_summary);
    assert!(summary.blocking_summary.contains("capacity: 1"), "{}", summary.blocking_summary);
    assert_eq!(summary.unavailable_reasons, vec!["activity stage was not run".to_owned()]);
    engine.release(&view.id).unwrap();

    // 不带冲突阶段：摘要必须说"未执行"，而不是"没有阻塞"。
    // Without a conflict stage the summary must say "was not run", never "no blocking".
    let mut bare = report();
    bare.conflict = None;
    bare.status = AnalysisStatus::Unknown;
    let view = verify(serde_json::to_string(&bare).unwrap()).unwrap();
    let finished = settle(&engine, &view);
    let summary: CriticalAnalysisReport =
        serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
    assert!(!summary.has_minimal_blocking_set);
    assert!(summary.blocking_summary.contains("未执行"), "{}", summary.blocking_summary);
    engine.release(&view.id).unwrap();

    // solver 生成物痕迹：领域红线，直接拒绝。 / Solver-generated traces: a domain red line, refused outright.
    let mut leaked = report();
    leaked.unavailable_reasons.push("leaked solver_row handle".to_owned());
    let error = verify(serde_json::to_string(&leaked).unwrap()).unwrap_err();
    assert_eq!(error.code, "InvalidInput");
    assert!(error.message.contains("solver-generated"), "{}", error.message);

    let mut missing = base_request();
    missing.model_kind = Some(ModelKind::CriticalAnalysisVerify);
    assert_eq!(engine.submit(missing).unwrap_err().code, "InvalidInput");
    assert_eq!(verify("not json".to_owned()).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(directory.join("critical-analysis-report.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn relaxation_recommend_entry_ranks_real_conflicts() {
    use ospf_rust_bridge::{RelaxationCostInput, RelaxationPolicyInput, RelaxationReport, RelaxationRequest};
    use ospf_rust_core::analysis::{
        AnalysisStatus, ConflictExplanation, ConflictExtractionTier, ConflictGroupSummary,
        ConflictMinimality, ConflictValidity, DiagnosticSource, ObjectiveTarget, TargetFeasibilityReport,
    };
    use ospf_rust_core::solver::report::{ProofStatus, StableConstraintId};
    use std::collections::BTreeMap;

    let target = ObjectiveTarget::at_least("objective", 5.0).unwrap();
    let conflict = |members: &[&str], minimality| ConflictExplanation {
        schema_version: ospf_rust_core::analysis::conflict::CONFLICT_REPORT_SCHEMA_VERSION.to_owned(),
        target: target.clone(),
        target_feasibility: TargetFeasibilityReport {
            schema_version: ospf_rust_core::analysis::conflict::CONFLICT_REPORT_SCHEMA_VERSION.to_owned(),
            target: target.clone(), effective_integer_bound: None,
            source: DiagnosticSource::ObjectiveTarget { target: target.clone() },
            status: AnalysisStatus::Unreachable, solution: None, exact_objective: None, objective_value: None,
            proof_status: ProofStatus::Verified, termination_reason: None, solve_time: None,
            target_fixed: true, message: None,
        },
        status: AnalysisStatus::Unreachable,
        available_evidence: Vec::new(), fixed_background: Vec::new(),
        members: members.iter().map(|id| DiagnosticSource::Constraint {
            id: StableConstraintId((*id).to_owned()) }).collect(),
        target_source: DiagnosticSource::ObjectiveTarget { target: target.clone() },
        validity: ConflictValidity::Verified, minimality,
        verifications: Vec::new(),
        group_summaries: vec![ConflictGroupSummary { group: Some("g".to_owned()), count: members.len() }],
        resolve_count: 2, memoized_checks: 0,
        extraction_tier: ConflictExtractionTier::AssumptionExtraction,
        unavailable_reason: None, background_blocking: false,
    };

    let engine = NativeEngine::default();
    let recommend = |request: RelaxationRequest| {
        let mut solve = base_request();
        solve.model_kind = Some(ModelKind::RelaxationRecommend);
        solve.relaxation_request = Some(request);
        engine.submit(solve)
    };

    let proven = serde_json::to_string(&conflict(&["capacity", "shift"], ConflictMinimality::Irreducible)).unwrap();
    let mut costs = BTreeMap::new();
    costs.insert("constraint:capacity".to_owned(), RelaxationCostInput { weight: 5.0, relaxation: 2.0 });
    let request = RelaxationRequest { conflict: proven.clone(), costs, policy: None };
    let view = recommend(request).unwrap();
    let finished = settle(&engine, &view);
    assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
    let report: RelaxationReport = serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
    assert_eq!(report.plan_count, 2);
    assert!(report.conflict_proven);
    // 便宜的那条排在前面：`shift` 用默认成本 1.0，`capacity` 是 5.0 × 2.0 = 10.0。
    // The cheaper one comes first: `shift` uses the default cost 1.0 while `capacity` is 5.0 × 2.0 = 10.0.
    assert_eq!(report.plans[0].members[0].source, "constraint:shift");
    assert_eq!(report.plans[0].total_cost, 1.0);
    assert_eq!(report.plans[1].total_cost, 10.0);
    assert!(report.plans[1].minimal);
    assert!(!report.plans[1].is_complete_mcs);
    assert!(report.plans[1].requires_revalidation);
    assert_eq!(report.plans[1].target_objective_id, "objective");
    assert_eq!(report.plans[1].target_relation, "AtLeast");
    assert_eq!(report.plans[1].target_value, 5.0);
    engine.release(&view.id).unwrap();

    // 未通过证明门槛：0 条建议，任务仍算完成（不是错误）。
    // Failing the proof gates: zero recommendations while the job still completes (not an error).
    let unproven = serde_json::to_string(&conflict(&["capacity"], ConflictMinimality::NotChecked)).unwrap();
    let view = recommend(RelaxationRequest {
        conflict: unproven, costs: BTreeMap::new(), policy: None }).unwrap();
    let finished = settle(&engine, &view);
    assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
    let report: RelaxationReport = serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
    assert_eq!(report.plan_count, 0);
    assert!(!report.conflict_proven);
    engine.release(&view.id).unwrap();

    // 策略上限为 0：领域拒绝。 / A policy limit of zero: refused by the domain.
    let error = recommend(RelaxationRequest {
        conflict: proven.clone(), costs: BTreeMap::new(),
        policy: Some(RelaxationPolicyInput { max_candidates: 0, max_plans: 8, require_positive_weight: true }),
    }).unwrap_err();
    assert_eq!(error.code, "InvalidInput");
    assert!(error.message.contains("must be positive"), "{}", error.message);

    // 缺请求 / 冲突非法：入口拒绝。 / Missing request / invalid conflict: refused at the entry.
    let mut missing = base_request();
    missing.model_kind = Some(ModelKind::RelaxationRecommend);
    assert_eq!(engine.submit(missing).unwrap_err().code, "InvalidInput");
    assert_eq!(recommend(RelaxationRequest {
        conflict: "not json".to_owned(), costs: BTreeMap::new(), policy: None }).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(directory.join("conflict-irreducible-two-members.json"), proven.as_bytes()).unwrap();
    }
}

#[test]
fn fixed_integer_lp_verify_entry_validates_config_and_projects_the_report() {
    use ospf_rust_bridge::{FixedIntegerConfigInput, FixedIntegerLpReport};
    use ospf_rust_core::analysis::fixed_integer::{
        FixedIntegerLpSensitivityConfig, LocalConstraintSensitivity, LocalConstraintSensitivityReport,
        SensitivityScope,
    };
    use ospf_rust_core::analysis::AnalysisStatus;
    use ospf_rust_core::solver::StableConstraintId as ConstraintId;

    let row = |id: &str, dual: Option<f64>, local: Option<bool>| LocalConstraintSensitivity {
        constraint_id: ConstraintId(id.to_owned()), dual_value: dual, local_effective: local,
        status: AnalysisStatus::Reachable, scope: SensitivityScope::FixedIntegerIncumbent, activity: None,
    };
    let report = || LocalConstraintSensitivityReport {
        schema_version: "1.0".to_owned(),
        scope: SensitivityScope::FixedIntegerIncumbent,
        baseline_objective: 10.0, lp_objective: Some(12.5), fixed_integer_count: 3,
        constraints: vec![row("capacity", Some(1.5), Some(true)), row("shift", Some(0.0), Some(false)),
            row("budget", None, None)],
        status: AnalysisStatus::Reachable, termination_reason: None, unavailable_reason: None,
    };

    let engine = NativeEngine::default();
    let verify = |config: Option<FixedIntegerConfigInput>, payload: Option<String>| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::FixedIntegerLpVerify);
        request.fixed_integer_config = config;
        request.fixed_integer_report = payload;
        engine.submit(request)
    };

    // 1) 只给配置：用领域默认值或显式值校验并回显，不投影报告。
    // Config only: validate and echo either the domain defaults or explicit values, projecting no report.
    let view = verify(None, None).unwrap();
    let finished = settle(&engine, &view);
    assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
    let summary: FixedIntegerLpReport = serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
    let default = FixedIntegerLpSensitivityConfig::default();
    assert!(!summary.has_report);
    assert_eq!(summary.integrality_tolerance, default.integrality_tolerance);
    assert_eq!(summary.dual_tolerance, default.dual_tolerance);
    engine.release(&view.id).unwrap();

    // 2) 显式容差 + 真实报告：投影全部字段。
    // Explicit tolerances plus a real report: every field is projected.
    let payload = serde_json::to_string(&report()).unwrap();
    let view = verify(Some(FixedIntegerConfigInput { integrality_tolerance: 1e-4, dual_tolerance: 1e-6 }),
        Some(payload.clone())).unwrap();
    let finished = settle(&engine, &view);
    assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
    let summary: FixedIntegerLpReport = serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
    assert_eq!(summary.integrality_tolerance, 1e-4);
    assert!(summary.has_report);
    assert_eq!(summary.rows.len(), 3);
    assert!(summary.has_duals);
    assert_eq!(summary.rows[1].dual_value, Some(0.0));
    assert_eq!(summary.rows[2].dual_value, None);
    assert!(summary.duplicate_constraint_ids.is_empty());
    engine.release(&view.id).unwrap();

    // 3) 重复 ID：领域放行，桥接显式列出。 / Duplicate IDs: the domain allows them, the bridge lists them.
    let mut duplicated = report();
    duplicated.constraints.push(row("capacity", Some(9.0), Some(false)));
    assert!(duplicated.validate().is_ok(), "the domain really does accept duplicate identities");
    let view = verify(None, Some(serde_json::to_string(&duplicated).unwrap())).unwrap();
    let finished = settle(&engine, &view);
    let summary: FixedIntegerLpReport = serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
    assert_eq!(summary.row_count, 4);
    assert_eq!(summary.duplicate_constraint_ids, vec!["capacity".to_owned()]);
    engine.release(&view.id).unwrap();

    // 4) 负容差 / 空白 schema：领域原因原样带出。
    // A negative tolerance and a blank schema: the domain reasons are relayed verbatim.
    let error = verify(Some(FixedIntegerConfigInput { integrality_tolerance: -1.0, dual_tolerance: 1e-9 }), None)
        .unwrap_err();
    assert_eq!(error.code, "InvalidInput");
    assert!(error.message.contains("finite and non-negative"), "{}", error.message);
    let mut blank = report();
    blank.schema_version = "   ".to_owned();
    assert_eq!(verify(None, Some(serde_json::to_string(&blank).unwrap())).unwrap_err().code, "InvalidInput");
    assert_eq!(verify(None, Some("not json".to_owned())).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(directory.join("fixed-integer-sensitivity.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn perturbation_policy_verify_entry_validates_and_projects() {
    use ospf_rust_bridge::{
        AdaptivePerturbationInput, PerturbationPolicyRequest, PerturbationVerification,
    };
    use ospf_rust_core::analysis::perturbation::{
        AdaptivePerturbationOutcome, ConstraintPerturbationObservation, ConstraintPerturbationReport,
        RemovalTestObservation,
    };
    use ospf_rust_core::analysis::AnalysisStatus;
    use ospf_rust_core::solver::report::TerminationReason;
    use ospf_rust_core::solver::StableConstraintId as ConstraintId;
    use std::time::Duration;

    let observation = |delta: f64, improved: Option<bool>| ConstraintPerturbationObservation {
        delta, perturbed_rhs: 10.0 + delta, status: AnalysisStatus::Reachable,
        termination_reason: TerminationReason::Completed, objective: Some(9.0),
        objective_change: Some(-1.0), improvement: Some(1.0), improved,
        integer_pattern_changed: Some(false), solve_time: Duration::from_millis(2), objective_proven: true,
    };
    let report = ConstraintPerturbationReport {
        schema_version: "1.0".to_owned(),
        constraint_id: ConstraintId("capacity".to_owned()),
        baseline_objective: 10.0,
        observations: vec![observation(1.0, Some(false)), observation(2.0, Some(true))],
        removal: Some(RemovalTestObservation {
            status: AnalysisStatus::Reachable, termination_reason: TerminationReason::Completed,
            objective: Some(8.0), objective_change: Some(-2.0), improvement: Some(2.0),
            effective: Some(false), integer_pattern_changed: Some(false),
            solve_time: Duration::from_millis(2), objective_proven: true,
        }),
        adaptive_outcome: Some(AdaptivePerturbationOutcome::NoObservedEffect),
        status: AnalysisStatus::Reachable, solve_count: 3, unavailable_reason: None,
    };

    let engine = NativeEngine::default();
    let verify = |policy: PerturbationPolicyRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::PerturbationPolicyVerify);
        request.perturbation_policy = Some(policy);
        engine.submit(request)
    };

    // 1) 显式序列 + 构建器：回显归一化后的策略。 / Explicit sequence with builders: echo the normalized policy.
    let view = verify(PerturbationPolicyRequest {
        deltas: vec![1.0, 2.0, 4.0], adaptive: None, removal_test: true,
        max_solves: Some(8), time_budget_ms: Some(500), objective_tolerance: Some(1e-3), report: None,
    }).unwrap();
    let finished = settle(&engine, &view);
    assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
    let verification: PerturbationVerification =
        serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
    assert_eq!(verification.mode, "explicit");
    assert_eq!(verification.delta_count, 3);
    assert_eq!(verification.deltas, vec![1.0, 2.0, 4.0]);
    assert_eq!(verification.max_solves, 8);
    assert_eq!(verification.time_budget_ms, Some(500));
    assert!(verification.removal_test);
    assert_eq!(verification.objective_tolerance, 1e-3);
    assert!(verification.report.is_none());
    engine.release(&view.id).unwrap();

    // 2) 自适应 + 真实报告：结论来自领域的 serde 标签与 `has_global_improvement`。
    // Adaptive plus a real report: the outcome comes from the domain's serde tag and `has_global_improvement`.
    let payload = serde_json::to_string(&report).unwrap();
    let view = verify(PerturbationPolicyRequest {
        deltas: Vec::new(), adaptive: Some(AdaptivePerturbationInput {
            initial_delta: 1.0, growth_factor: 2.0, max_delta: 8.0,
            refine_threshold: true, refinement_tolerance: 1e-7 }),
        removal_test: false, max_solves: None, time_budget_ms: None, objective_tolerance: None,
        report: Some(payload.clone()),
    }).unwrap();
    let finished = settle(&engine, &view);
    assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
    let verification: PerturbationVerification =
        serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
    assert_eq!(verification.mode, "adaptive");
    assert_eq!(verification.delta_count, 0);
    let summary = verification.report.expect("report summary");
    assert_eq!(summary.constraint_id, "capacity");
    assert_eq!(summary.solve_count, 3);
    assert!(summary.has_removal);
    assert!(summary.has_global_improvement);
    assert_eq!(summary.last_observation_index, Some(1));
    assert_eq!(summary.adaptive_outcome.as_deref(), Some("noObservedEffect"));
    engine.release(&view.id).unwrap();

    // 3) 互斥冲突：两边都给必须在桥接先被拒绝，否则 deltas 会被领域构造器静默丢弃。
    // The exclusivity conflict: supplying both must be refused in the bridge first, otherwise the domain constructor
    // silently drops the deltas.
    let error = verify(PerturbationPolicyRequest {
        deltas: vec![1.0], adaptive: Some(AdaptivePerturbationInput {
            initial_delta: 1.0, growth_factor: 2.0, max_delta: 8.0,
            refine_threshold: true, refinement_tolerance: 1e-7 }),
        removal_test: false, max_solves: None, time_budget_ms: None, objective_tolerance: None, report: None,
    }).unwrap_err();
    assert_eq!(error.code, "InvalidInput");
    assert!(error.message.contains("either explicit deltas or an adaptive strategy"), "{}", error.message);

    // 4) 两边都不给 / 非法 delta / 非法预算：领域原因原样带出。
    // Neither side, an illegal delta and an illegal budget: the domain reasons are relayed verbatim.
    let error = verify(PerturbationPolicyRequest {
        deltas: Vec::new(), adaptive: None, removal_test: false, max_solves: None,
        time_budget_ms: None, objective_tolerance: None, report: None }).unwrap_err();
    assert!(error.message.contains("either explicit deltas or an adaptive strategy"), "{}", error.message);
    let error = verify(PerturbationPolicyRequest {
        deltas: vec![0.0], adaptive: None, removal_test: false, max_solves: None,
        time_budget_ms: None, objective_tolerance: None, report: None }).unwrap_err();
    assert!(error.message.contains("strictly positive"), "{}", error.message);
    let error = verify(PerturbationPolicyRequest {
        deltas: vec![1.0], adaptive: None, removal_test: false, max_solves: Some(0),
        time_budget_ms: None, objective_tolerance: None, report: None }).unwrap_err();
    assert!(error.message.contains("max solves must be positive"), "{}", error.message);

    // 5) 报告 `solve_count` 漂移：领域不变量。 / A drifted report `solve_count`: a domain invariant.
    let mut drifted = report;
    drifted.solve_count = 9;
    let error = verify(PerturbationPolicyRequest {
        deltas: vec![1.0], adaptive: None, removal_test: false, max_solves: None, time_budget_ms: None,
        objective_tolerance: None, report: Some(serde_json::to_string(&drifted).unwrap()) }).unwrap_err();
    assert!(error.message.contains("solve count"), "{}", error.message);

    let mut missing = base_request();
    missing.model_kind = Some(ModelKind::PerturbationPolicyVerify);
    assert_eq!(engine.submit(missing).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(directory.join("perturbation-report.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn target_verify_entry_derives_exact_integer_bounds_and_checks_reports() {
    use ospf_rust_bridge::{TargetSpec, TargetVerification};
    use ospf_rust_core::analysis::target::{exact_integer_bound, TargetFeasibilityReport};
    use ospf_rust_core::analysis::{AnalysisStatus, DiagnosticSource, ObjectiveTarget};
    use ospf_rust_core::solver::report::ProofStatus;

    let engine = NativeEngine::default();
    let verify = |spec: TargetSpec, report: Option<String>| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::TargetVerify);
        request.target_spec = Some(spec);
        request.target_report = report;
        engine.submit(request)
    };
    let absolute = |relation: &str, value: f64| TargetSpec::Absolute {
        objective_id: "objective".to_owned(), relation: relation.to_owned(), value };

    // 1) 绝对上下界：ceil / floor 规则。 / Absolute bounds: the ceil / floor rules.
    let view = verify(absolute("atLeast", 4.2), None).unwrap();
    let finished = settle(&engine, &view);
    assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
    let verification: TargetVerification =
        serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
    assert_eq!(verification.target.value, 4.2);
    assert_eq!(verification.target.exact_integer_bound, Some(5));
    assert_eq!(verification.target.relation, "AtLeast");
    assert!(verification.target.constraint_id.contains(&verification.target.stable_id));
    assert!(verification.report.is_none());
    engine.release(&view.id).unwrap();

    let view = verify(absolute("atMost", 4.8), None).unwrap();
    let finished = settle(&engine, &view);
    let verification: TargetVerification =
        serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
    assert_eq!(verification.target.exact_integer_bound, Some(4));
    engine.release(&view.id).unwrap();

    // 2) 相对 / 业务单位：折算成绝对值。 / Relative and business-unit modes fold into an absolute value.
    let view = verify(TargetSpec::Relative {
        objective_id: "objective".to_owned(), relation: "atLeast".to_owned(),
        baseline: 100.0, fraction: 0.05 }, None).unwrap();
    let finished = settle(&engine, &view);
    let verification: TargetVerification =
        serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
    assert_eq!(verification.target.value, 105.0);
    assert_eq!(verification.target.exact_integer_bound, Some(105));
    engine.release(&view.id).unwrap();

    // 3) 真实报告：与请求 target 一致时投影；不一致时点名两边。
    // A real report: projected when it agrees with the request, otherwise refused naming both sides.
    let target = ObjectiveTarget::at_least("objective", 5.0).unwrap();
    let report = TargetFeasibilityReport {
        schema_version: "1.0".to_owned(),
        target: target.clone(),
        effective_integer_bound: exact_integer_bound(&target),
        source: DiagnosticSource::ObjectiveTarget { target: target.clone() },
        status: AnalysisStatus::Unreachable, solution: None, exact_objective: None,
        objective_value: None, proof_status: ProofStatus::Verified, termination_reason: None,
        solve_time: None, target_fixed: true, message: Some("target unreachable".to_owned()),
    };
    let payload = serde_json::to_string(&report).unwrap();
    let view = verify(absolute("atLeast", 5.0), Some(payload.clone())).unwrap();
    let finished = settle(&engine, &view);
    assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
    let verification: TargetVerification =
        serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
    let summary = verification.report.expect("report summary");
    assert_eq!(summary.status, "unreachable");
    assert_eq!(summary.reachable, Some(false));
    assert_eq!(summary.effective_integer_bound, Some(5));
    assert_eq!(summary.proof_status, "verified");
    assert!(summary.target_fixed);
    assert_eq!(summary.message.as_deref(), Some("target unreachable"));
    engine.release(&view.id).unwrap();

    let error = verify(absolute("atLeast", 7.0), Some(payload.clone())).unwrap_err();
    assert_eq!(error.code, "InvalidInput");
    assert!(error.message.contains("but the request asked for"), "{}", error.message);

    // 4) 非法输入：空白身份 / 未知方向 / 非 JSON / 缺请求。
    // Illegal inputs: a blank identity, an unknown relation, non-JSON and a missing request.
    let error = verify(TargetSpec::Absolute {
        objective_id: " ".to_owned(), relation: "atLeast".to_owned(), value: 1.0 }, None).unwrap_err();
    assert!(error.message.contains("must not be blank"), "{}", error.message);
    let error = verify(absolute("greater", 1.0), None).unwrap_err();
    assert!(error.message.contains("unsupported relation"), "{}", error.message);
    assert_eq!(verify(absolute("atLeast", 1.0), Some("not json".to_owned())).unwrap_err().code, "InvalidInput");
    let mut missing = base_request();
    missing.model_kind = Some(ModelKind::TargetVerify);
    assert_eq!(engine.submit(missing).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(directory.join("target-feasibility-report.json"), payload.as_bytes()).unwrap();
    }
}
#[test]
fn multi_target_verify_entry_recomputes_cross_target_bottlenecks() {
    use ospf_rust_bridge::{MultiTargetReportSummary, RelaxationPolicyInput};
    use ospf_rust_core::analysis::correction::{
        alternative_improvement_plans_from_conflict, weighted_correction_sets_from_conflict,
        CorrectionSet, RelaxabilityPolicy,
    };
    use ospf_rust_core::analysis::criticality::{build_criticality_profile, CriticalityKind, CriticalityObservation};
    use ospf_rust_core::analysis::multi_target::{
        MultiTargetAnalysisReport, MultiTargetAnalysisResult, MULTI_TARGET_ANALYSIS_REPORT_SCHEMA_VERSION,
    };
    use ospf_rust_core::analysis::{
        AnalysisStatus, ConflictExplanation, ConflictExtractionTier, ConflictGroupSummary,
        ConflictMinimality, ConflictValidity, DiagnosticSource, ObjectiveTarget, TargetFeasibilityReport,
    };
    use ospf_rust_core::solver::report::{ProofStatus, StableConstraintId};
    use std::collections::BTreeSet;

    let feasibility = |target: &ObjectiveTarget, status| TargetFeasibilityReport {
        schema_version: "1.0".to_owned(), target: target.clone(), effective_integer_bound: None,
        source: DiagnosticSource::ObjectiveTarget { target: target.clone() }, status,
        solution: None, exact_objective: None, objective_value: None, proof_status: ProofStatus::Verified,
        termination_reason: None, solve_time: None, target_fixed: true, message: None,
    };
    let conflict = |target: &ObjectiveTarget, members: &[&str]| ConflictExplanation {
        schema_version: ospf_rust_core::analysis::conflict::CONFLICT_REPORT_SCHEMA_VERSION.to_owned(),
        target: target.clone(), target_feasibility: feasibility(target, AnalysisStatus::Unreachable),
        status: AnalysisStatus::Unreachable, available_evidence: Vec::new(), fixed_background: Vec::new(),
        members: members.iter().map(|id| DiagnosticSource::Constraint {
            id: StableConstraintId((*id).to_owned()) }).collect(),
        target_source: DiagnosticSource::ObjectiveTarget { target: target.clone() },
        validity: ConflictValidity::Verified, minimality: ConflictMinimality::Irreducible,
        verifications: Vec::new(),
        group_summaries: vec![ConflictGroupSummary { group: None, count: members.len() }],
        resolve_count: 2, memoized_checks: 0,
        extraction_tier: ConflictExtractionTier::AssumptionExtraction,
        unavailable_reason: None, background_blocking: false,
    };
    let proven = |target: ObjectiveTarget, members: &[&str]| {
        let conflict = conflict(&target, members);
        let policy = RelaxabilityPolicy::default();
        let costs = std::collections::BTreeMap::new();
        MultiTargetAnalysisResult {
            feasibility: conflict.target_feasibility.clone(),
            target,
            correction_sets: weighted_correction_sets_from_conflict(&conflict, &costs, &policy).unwrap(),
            improvement_plans: alternative_improvement_plans_from_conflict(&conflict, &costs, &policy).unwrap(),
            conflict,
        }
    };
    let unknown = |target: ObjectiveTarget| {
        let feasibility = feasibility(&target, AnalysisStatus::Unknown);
        MultiTargetAnalysisResult {
            target: target.clone(),
            feasibility: feasibility.clone(),
            conflict: ConflictExplanation {
                schema_version: ospf_rust_core::analysis::conflict::CONFLICT_REPORT_SCHEMA_VERSION.to_owned(),
                target: target.clone(), target_feasibility: feasibility, status: AnalysisStatus::Unknown,
                available_evidence: Vec::new(), fixed_background: Vec::new(), members: Vec::new(),
                target_source: DiagnosticSource::ObjectiveTarget { target: target.clone() },
                validity: ConflictValidity::Unknown, minimality: ConflictMinimality::NotChecked,
                verifications: Vec::new(), group_summaries: Vec::new(), resolve_count: 0,
                memoized_checks: 0, extraction_tier: ConflictExtractionTier::Unavailable,
                unavailable_reason: Some("no conflict analysis".to_owned()), background_blocking: false,
            },
            correction_sets: Vec::<CorrectionSet>::new(),
            improvement_plans: Vec::new(),
        }
    };
    let report = || {
        let results = vec![
            proven(ObjectiveTarget::at_least("throughput", 10.0).unwrap(), &["capacity", "shift"]),
            proven(ObjectiveTarget::at_least("cost", 5.0).unwrap(), &["capacity"]),
            unknown(ObjectiveTarget::at_least("latency", 3.0).unwrap()),
        ];
        let criticality_profile = build_criticality_profile(results.iter().map(|result| {
            CriticalityObservation {
                target: result.target.clone(),
                status: result.feasibility.status,
                blocking_constraint_ids: if result.has_verified_conflict() {
                    result.conflict.constraint_ids()
                } else {
                    BTreeSet::new()
                },
            }
        }).collect()).unwrap();
        MultiTargetAnalysisReport {
            schema_version: MULTI_TARGET_ANALYSIS_REPORT_SCHEMA_VERSION.to_owned(),
            results, criticality_profile,
        }
    };

    let engine = NativeEngine::default();
    let verify = |payload: String, policy: Option<RelaxationPolicyInput>| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::MultiTargetVerify);
        request.multi_target_report = Some(payload);
        request.relaxation_policy = policy;
        engine.submit(request)
    };

    let payload = serde_json::to_string(&report()).unwrap();
    let view = verify(payload.clone(), None).unwrap();
    let finished = settle(&engine, &view);
    assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
    let summary: MultiTargetReportSummary =
        serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
    assert_eq!(summary.result_count, 3);
    assert_eq!(summary.verified_conflict_count, 2);
    assert_eq!(summary.plan_count, 3);
    assert_eq!(summary.proven_target_count, 2);
    assert_eq!(summary.classified_constraint_count, 2);
    assert_eq!(summary.results[0].blocking_constraint_ids, vec!["capacity", "shift"]);
    assert_eq!(summary.results[2].reachable, None);
    // 跨目标分类：`capacity` 在两个不可达目标里都出现 → 持久 + 结构瓶颈；`shift` 只在一个里出现 → 局部瓶颈。
    // Cross-target classification: `capacity` is in both unreachable targets (persistent + structural), `shift` in one
    // (local).
    assert_eq!(summary.bottlenecks[0].constraint_ids, vec!["shift"]);
    assert_eq!(summary.bottlenecks[1].constraint_ids, vec!["capacity"]);
    assert_eq!(summary.bottlenecks[2].constraint_ids, vec!["capacity"]);
    engine.release(&view.id).unwrap();

    // 篡改分类：先被 profile 自身的重算校验挡住。 / Tampered classification: caught by the profile's own recomputation.
    let mut broken = report();
    broken.criticality_profile.classifications
        .get_mut(&StableConstraintId("shift".to_owned())).unwrap()
        .insert(CriticalityKind::StructuralBottleneck);
    let error = verify(serde_json::to_string(&broken).unwrap(), None).unwrap_err();
    assert_eq!(error.code, "InvalidInput");
    assert!(error.message.contains("classifications do not match observations"), "{}", error.message);

    // 策略收紧：方案序号超出策略。 / A tightened policy: the plan rank exceeds it.
    let tight = RelaxationPolicyInput { max_candidates: 32, max_plans: 1, require_positive_weight: true };
    let error = verify(payload.clone(), Some(tight)).unwrap_err();
    assert!(error.message.contains("exceeds policy"), "{}", error.message);

    // 缺请求 / 非 JSON。 / A missing request and non-JSON.
    let mut missing = base_request();
    missing.model_kind = Some(ModelKind::MultiTargetVerify);
    assert_eq!(engine.submit(missing).unwrap_err().code, "InvalidInput");
    assert_eq!(verify("not json".to_owned(), None).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(directory.join("multi-target-report.json"), payload.as_bytes()).unwrap();
    }
}
#[test]
fn criticality_profile_verify_entry_classifies_and_cross_checks() {
    use ospf_rust_bridge::{CriticalityObservationInput, CriticalityProfileReport, CriticalityRequest};
    use ospf_rust_core::analysis::criticality::{build_criticality_profile, CriticalityObservation};
    use ospf_rust_core::analysis::ObjectiveTarget;
    use ospf_rust_core::solver::report::StableConstraintId;
    use ospf_rust_core::analysis::AnalysisStatus;

    let observation = |id: &str, status: &str, ids: &[&str]| CriticalityObservationInput {
        objective_id: id.to_owned(), relation: "atLeast".to_owned(), value: 10.0,
        status: status.to_owned(),
        blocking_constraint_ids: ids.iter().map(|value| (*value).to_owned()).collect(),
    };

    let engine = NativeEngine::default();
    let verify = |request: CriticalityRequest| {
        let mut solve = base_request();
        solve.model_kind = Some(ModelKind::CriticalityProfileVerify);
        solve.criticality_request = Some(request);
        engine.submit(solve)
    };

    let observations = vec![
        observation("throughput", "unreachable", &["capacity", "shift"]),
        observation("cost", "unreachable", &["capacity"]),
        observation("latency", "reachable", &["ghost"]),
    ];
    let view = verify(CriticalityRequest { observations: observations.clone(), profile: None }).unwrap();
    let finished = settle(&engine, &view);
    assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
    let report: CriticalityProfileReport =
        serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
    assert_eq!(report.observation_count, 3);
    // 只有两条不可达观察算已证明，`ghost` 不参与分类。 / Only two unreachable observations count, `ghost` does not.
    assert_eq!(report.proven_target_count, 2);
    assert_eq!(report.classified_constraint_count, 2);
    assert!(!report.observations[2].contributes_to_classification);
    assert_eq!(report.bottlenecks[0].constraint_ids, vec!["shift".to_owned()]);
    assert_eq!(report.bottlenecks[1].constraint_ids, vec!["capacity".to_owned()]);
    assert_eq!(report.bottlenecks[2].constraint_ids, vec!["capacity".to_owned()]);
    engine.release(&view.id).unwrap();

    // 单个不可达目标：只有局部瓶颈，没有结构瓶颈。
    // A single unreachable target: only a local bottleneck, no structural one.
    let single = vec![observation("throughput", "unreachable", &["capacity"])];
    let view = verify(CriticalityRequest { observations: single, profile: None }).unwrap();
    let finished = settle(&engine, &view);
    let report: CriticalityProfileReport =
        serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
    assert_eq!(report.bottlenecks[1].constraint_ids, Vec::<String>::new());
    assert_eq!(report.bottlenecks[2].constraint_ids, Vec::<String>::new());
    engine.release(&view.id).unwrap();

    // 声称的 profile 与重算一致时通过；不一致时点名差异。
    // A claimed profile matching the recomputation passes; a mismatch names the difference.
    let real = {
        let built = build_criticality_profile(observations.iter().map(|input| CriticalityObservation {
            target: ObjectiveTarget::at_least(input.objective_id.as_str(), input.value).unwrap(),
            status: if input.status == "unreachable" { AnalysisStatus::Unreachable } else { AnalysisStatus::Reachable },
            blocking_constraint_ids: input.blocking_constraint_ids.iter()
                .map(|id| StableConstraintId(id.clone())).collect(),
        }).collect()).unwrap();
        serde_json::to_string(&built).unwrap()
    };
    let view = verify(CriticalityRequest { observations: observations.clone(), profile: Some(real.clone()) }).unwrap();
    let finished = settle(&engine, &view);
    assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
    engine.release(&view.id).unwrap();
    // 这份 `real` 是**领域** profile 的序列化（状态是 SCREAMING_SNAKE_CASE），桥接报告的 `status` 是小写，
    // 两者不能混用——声称的 profile 必须是领域格式。
    // `real` is the serialized **domain** profile (SCREAMING_SNAKE_CASE statuses), while the bridge report uses lowercase
    // statuses; a claimed profile must be in the domain format, so the two are not interchangeable.

    let mut mismatched = CriticalityRequest { observations: observations.clone(), profile: Some(real.clone()) };
    mismatched.observations.truncate(1);
    let error = verify(mismatched).unwrap_err();
    assert_eq!(error.code, "InvalidInput");
    assert!(error.message.contains("disagree with the recomputation"), "{}", error.message);

    // 重复 target / 未知方向 / 未知状态 / 缺请求。
    // A duplicate target, an unknown relation, an unknown status and a missing request.
    let error = verify(CriticalityRequest {
        observations: vec![observation("throughput", "unreachable", &[]),
            observation("throughput", "unreachable", &[])], profile: None }).unwrap_err();
    assert!(error.message.contains("duplicate targets"), "{}", error.message);
    let mut unknown_relation = observation("throughput", "unreachable", &[]);
    unknown_relation.relation = "greater".to_owned();
    let error = verify(CriticalityRequest { observations: vec![unknown_relation], profile: None }).unwrap_err();
    assert!(error.message.contains("unsupported relation"), "{}", error.message);
    let mut unknown_status = observation("throughput", "unreachable", &[]);
    unknown_status.status = "solved".to_owned();
    let error = verify(CriticalityRequest { observations: vec![unknown_status], profile: None }).unwrap_err();
    assert!(error.message.contains("unknown status"), "{}", error.message);
    let mut missing = base_request();
    missing.model_kind = Some(ModelKind::CriticalityProfileVerify);
    assert_eq!(engine.submit(missing).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        let real_profile = real.clone();
        let payload = serde_json::to_string(&CriticalityRequest {
            observations, profile: None }).unwrap();
        std::fs::write(directory.join("criticality-request.json"), payload.as_bytes()).unwrap();
        // 真实领域 profile 也留一份，供 SDK 侧作为"声称的 profile"用。
        // Also leave a real domain profile for the SDK side to use as the claimed profile.
        std::fs::write(directory.join("criticality-profile.json"), real_profile.as_bytes()).unwrap();
    }
}
#[test]
fn hyper_heuristic_config_verify_entry_round_trips_all_three_formats() {
    use ospf_rust_bridge::{HyperHeuristicConfigReport, HyperHeuristicConfigRequest};

    // 三份文本描述同一个配置模型（手写文本是被测对象，不是载荷）。
    // Three texts describe the same model (the hand-written text is the subject under test, not a payload).
    let json = r#"{"problem":{"type":"bpp","version":1},
        "algorithms":[{"id":"a1","algorithm":{"type":"simulated-annealing","version":1}}],
        "budget":{"evaluations":1000}}"#;
    let yaml = "problem:\n  type: bpp\n  version: 1\nalgorithms:\n  - id: a1\n    algorithm:\n      type: simulated-annealing\nbudget:\n  evaluations: 1000\n";
    let toml = "problem = { type = \"bpp\", version = 1 }\n[[algorithms]]\nid = \"a1\"\nalgorithm = { type = \"simulated-annealing\" }\n[budget]\nevaluations = 1000\n";

    let engine = NativeEngine::default();
    let verify = |text: &str, format: &str| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::HyperHeuristicConfigVerify);
        request.hyper_heuristic_config = Some(HyperHeuristicConfigRequest {
            text: text.to_owned(), format: format.to_owned() });
        engine.submit(request)
    };

    let mut canonical = None;
    for (format, text) in [("json", json), ("yaml", yaml), ("toml", toml)] {
        let view = verify(text, format).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: HyperHeuristicConfigReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        assert_eq!(report.version, 1);
        assert_eq!(report.problem, "bpp");
        assert_eq!(report.algorithm_count, 1);
        assert_eq!(report.algorithms[0].id, "a1");
        assert_eq!(report.algorithms[0].size, 20);
        assert_eq!(report.algorithms[0].initialization, "Model");
        assert_eq!(report.budget.global_limit_count, 1);
        assert_eq!(report.execution.mode, "Synchronous");
        // 三种格式解码到同一模型 → 规范化 JSON 必须逐字相同。
        // All three formats decode into one model, so the normalized JSON must be byte-identical.
        match &canonical {
            None => canonical = Some(report.canonical_json.clone()),
            Some(expected) => assert_eq!(&report.canonical_json, expected,
                "format `{format}` must normalize identically"),
        }
        engine.release(&view.id).unwrap();
    }

    // 规范化 JSON 再解码：幂等。 / Re-decoding the normalized JSON is idempotent.
    let view = verify(canonical.as_deref().unwrap(), "json").unwrap();
    let finished = settle(&engine, &view);
    let report: HyperHeuristicConfigReport =
        serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
    assert_eq!(Some(report.canonical_json), canonical, "normalization must be idempotent");
    engine.release(&view.id).unwrap();

    // 结构规则：没有全局上限 / 并行度为 0 / 自动检查点缺持久化 / 未知字段。
    // Structural rules: no global limit, zero parallelism, automatic checkpoints without persistence, an unknown field.
    let no_limit = r#"{"problem":{"type":"bpp"},"algorithms":[{"id":"a1","algorithm":{"type":"sa"}}],
        "budget":{"sliceEvaluations":256,"sliceSteps":1}}"#;
    let error = verify(no_limit, "json").unwrap_err();
    assert_eq!(error.code, "InvalidInput");
    assert!(error.message.contains("global stopping limit"), "{}", error.message);
    let zero_parallel = r#"{"problem":{"type":"bpp"},"algorithms":[{"id":"a1","algorithm":{"type":"sa"}}],
        "budget":{"evaluations":10},"execution":{"parallelism":0}}"#;
    let error = verify(zero_parallel, "json").unwrap_err();
    assert!(error.message.contains("capacities and parallelism must be positive"), "{}", error.message);
    let no_persistence = r#"{"problem":{"type":"bpp"},"algorithms":[{"id":"a1","algorithm":{"type":"sa"}}],
        "budget":{"evaluations":10},"persistence":{"everyDecisions":5}}"#;
    let error = verify(no_persistence, "json").unwrap_err();
    assert!(error.message.contains("automatic checkpoints require persistence"), "{}", error.message);
    let unknown = r#"{"problem":{"type":"bpp"},"algorithms":[{"id":"a1","algorithm":{"type":"sa"}}],
        "budget":{"evaluations":10},"nonsense":1}"#;
    let error = verify(unknown, "json").unwrap_err();
    assert!(error.message.contains("unknown field"), "{}", error.message);
    assert_eq!(verify(json, "xml").unwrap_err().code, "InvalidInput");

    // 缺请求。 / A missing request.
    let mut missing = base_request();
    missing.model_kind = Some(ModelKind::HyperHeuristicConfigVerify);
    assert_eq!(engine.submit(missing).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(directory.join("hyper-heuristic-config.json"), json.as_bytes()).unwrap();
        std::fs::write(directory.join("hyper-heuristic-config.yaml"), yaml.as_bytes()).unwrap();
        std::fs::write(directory.join("hyper-heuristic-config.toml"), toml.as_bytes()).unwrap();
    }
}
#[test]
fn reward_policy_verify_entry_validates_all_three_policies() {
    use ospf_rust_bridge::{ExtremeValueInput, HypervolumeConfigInput, RewardPolicyReport, RewardPolicyRequest,
        TimeNormalizedInput};

    let engine = NativeEngine::default();
    let verify = |policy: RewardPolicyRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::RewardPolicyVerify);
        request.reward_policy = Some(policy);
        engine.submit(request)
    };

    // 三个策略一起校验：回显参数 + 真实可恢复状态。 / All three validated: parameter echo plus real restorable state.
    let policy = RewardPolicyRequest {
        time_normalized: Some(TimeNormalizedInput { epsilon_seconds: 0.001 }),
        extreme_value: Some(ExtremeValueInput { size: 32 }),
        hypervolume: Some(HypervolumeConfigInput {
            reference: vec![10.0, 10.0], mode: "sampled".to_owned(), max_work: 1_000_000,
            samples: 16384, seed: 7, calibration_observations: 32,
        }),
    };
    let view = verify(policy.clone()).unwrap();
    let finished = settle(&engine, &view);
    assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
    let report: RewardPolicyReport =
        serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
    assert_eq!(report.validated_count, 3);
    assert_eq!(report.time_normalized_epsilon_seconds, Some(0.001));
    assert_eq!(report.extreme_value_size, Some(32));
    let hypervolume = report.hypervolume.as_ref().unwrap();
    assert_eq!(hypervolume.mode, "Sampled");
    assert!(!hypervolume.has_measurement);
    // 状态文本是字符串化的 JSON：参考点、空区间、零观测、无测量。`definition` 指纹可用作策略身份。
    // The state text is stringified JSON: the reference, an empty envelope, zero observations and no measurement; the
    // `definition` fingerprint doubles as the policy identity.
    let state: serde_json::Value = serde_json::from_str(&hypervolume.state_json).unwrap();
    assert_eq!(state["version"], 1);
    assert_eq!(state["reference"], serde_json::json!([10.0, 10.0]));
    assert_eq!(state["observations"], 0);
    assert!(state["measurement"].is_null());
    assert!(state["definition"].as_str().unwrap().contains("10.0, 10.0"));
    engine.release(&view.id).unwrap();

    // 空参考点：状态里区间为空。 / An empty reference: the state envelope is empty.
    let view = verify(RewardPolicyRequest {
        time_normalized: None, extreme_value: None,
        hypervolume: Some(HypervolumeConfigInput { reference: Vec::new(), mode: "exact".to_owned(),
            max_work: 10, samples: 2, seed: 1, calibration_observations: 4 }),
    }).unwrap();
    let finished = settle(&engine, &view);
    let report: RewardPolicyReport =
        serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
    let state: serde_json::Value =
        serde_json::from_str(&report.hypervolume.as_ref().unwrap().state_json).unwrap();
    assert_eq!(state["reference"], serde_json::json!([]));
    assert_eq!(state["minimum"], serde_json::json!([]));
    engine.release(&view.id).unwrap();

    // 各自的非法参数：错误信息必须点名是哪个策略。
    // Each policy's illegal parameters: the message must name which policy failed.
    for epsilon in [0.0, -1.0] {
        let error = verify(RewardPolicyRequest {
            time_normalized: Some(TimeNormalizedInput { epsilon_seconds: epsilon }),
            extreme_value: None, hypervolume: None }).unwrap_err();
        assert_eq!(error.code, "InvalidInput");
        assert!(error.message.contains("time-normalized reward failed validation"), "{}", error.message);
        assert!(error.message.contains("minimum reward duration"), "{}", error.message);
    }
    for size in [0u64, i32::MAX as u64 + 1] {
        let error = verify(RewardPolicyRequest { time_normalized: None,
            extreme_value: Some(ExtremeValueInput { size }), hypervolume: None }).unwrap_err();
        assert!(error.message.contains("extreme-value window"), "{}", error.message);
    }
    // 边界值合法。 / The boundary value is legal.
    let view = verify(RewardPolicyRequest { time_normalized: None,
        extreme_value: Some(ExtremeValueInput { size: i32::MAX as u64 }), hypervolume: None }).unwrap();
    let finished = settle(&engine, &view);
    let report: RewardPolicyReport =
        serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
    assert_eq!(report.extreme_value_size, Some(i32::MAX as u64));
    engine.release(&view.id).unwrap();

    let legal = HypervolumeConfigInput {
        reference: vec![1.0], mode: "exact".to_owned(), max_work: 100,
        samples: 1, seed: 1, calibration_observations: 1,
    };
    for input in [
        HypervolumeConfigInput { reference: vec![f64::INFINITY], ..legal.clone() },
        HypervolumeConfigInput { max_work: 0, ..legal.clone() },
        HypervolumeConfigInput { samples: 0, ..legal.clone() },
        HypervolumeConfigInput { calibration_observations: 0, ..legal.clone() },
    ] {
        let error = verify(RewardPolicyRequest { time_normalized: None, extreme_value: None,
            hypervolume: Some(input) }).unwrap_err();
        assert!(error.message.contains("invalid hypervolume parameters"), "{}", error.message);
    }
    let error = verify(RewardPolicyRequest { time_normalized: None, extreme_value: None,
        hypervolume: Some(HypervolumeConfigInput { mode: "approximate".to_owned(), ..legal }) }).unwrap_err();
    assert!(error.message.contains("unsupported mode"), "{}", error.message);
    let error = verify(RewardPolicyRequest { time_normalized: None, extreme_value: None,
        hypervolume: None }).unwrap_err();
    assert!(error.message.contains("at least one reward policy"), "{}", error.message);

    // 缺请求。 / A missing request.
    let mut missing = base_request();
    missing.model_kind = Some(ModelKind::RewardPolicyVerify);
    assert_eq!(engine.submit(missing).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        let payload = serde_json::to_string(&policy).unwrap();
        std::fs::write(directory.join("reward-policy-request.json"), payload.as_bytes()).unwrap();
    }
}
#[test]
fn checkpoint_verify_entry_decodes_strictly_and_audits_kotlin_v1() {
    use ospf_rust_bridge::{CheckpointReport, CheckpointRequest};

    // 两份手写文档（这里是被测对象，不是载荷）：一份策略档案、一份完整检查点。
    // Two hand-written documents (the subject under test, not payloads): one policy archive and one run checkpoint.
    let policy = serde_json::json!({
        "version": 1,
        "definition": "algorithms:fingerprint",
        "featureVersion": "progress:1",
        "root": { "version": 1, "type": "tabular", "actions": ["a1", "a2"],
            "counts": [3, 4], "sums": [1.5, 2.5] },
        "operators": { "a1": { "swap": { "version": 1, "type": "tabular", "actions": ["swap"] } } },
        "metadata": { "episodes": "12" },
    }).to_string();
    let checkpoint = serde_json::json!({
        "version": 1,
        "configuration": "config:fingerprint",
        "problem": "problem:fingerprint",
        "policy": serde_json::from_str::<serde_json::Value>(&policy).unwrap(),
        "workers": { "w1": { "session": { "initialized": true, "iteration": 7, "stagnation": 2,
            "state": { "iteration": 7 }, "archive": [] }, "random": 42, "initialized": true,
            "disabled": false } },
        "archive": [], "working": [],
        "evaluations": 128, "dispatched": 16, "completed": 15, "steps": 64, "stagnation": 3,
        "activeNanos": 1000, "random": 7, "diagnostics": ["note"],
        "objectiveCalls": 96, "transitions": [], "successes": 5, "lastCost": 2,
        "configurationData": { "algorithms": [{ "id": "a1", "algorithm": { "type": "sa" } }] },
        "components": { "reward": { "version": 1 } },
    }).to_string();

    let engine = NativeEngine::default();
    let verify = |kind: &str, text: &str, kotlin_v1: bool| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::CheckpointVerify);
        request.checkpoint_request = Some(CheckpointRequest {
            kind: kind.to_owned(), text: text.to_owned(), kotlin_v1 });
        engine.submit(request)
    };

    // 策略档案：解出统计、规范文本可再解（往返稳定）。 / Policy archive: counters, and canonical text decodes again.
    let view = verify("policy", &policy, false).unwrap();
    let finished = settle(&engine, &view);
    assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
    let report: CheckpointReport =
        serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
    assert_eq!(report.kind, "policy");
    assert_eq!(report.root_action_count, 2);
    assert_eq!(report.operator_scope_count, 1);
    assert_eq!(report.operator_policy_count, 1);
    assert_eq!(report.metadata_count, 1);
    assert_eq!(report.kotlin_v1_compatible, None);
    let canonical = report.canonical_json.clone();
    engine.release(&view.id).unwrap();

    let view = verify("policy", &canonical, false).unwrap();
    let finished = settle(&engine, &view);
    let report: CheckpointReport =
        serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
    assert_eq!(report.canonical_json, canonical, "policy round trip must be stable");
    engine.release(&view.id).unwrap();

    // 完整检查点 + 审计：计数器全部投影，且可导出。 / A full checkpoint with audit: counters projected, exportable.
    let view = verify("checkpoint", &checkpoint, true).unwrap();
    let finished = settle(&engine, &view);
    assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
    let report: CheckpointReport =
        serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
    assert_eq!(report.kind, "checkpoint");
    assert_eq!(report.worker_count, 1);
    assert_eq!(report.evaluations, 128);
    assert_eq!(report.dispatched, 16);
    assert_eq!(report.completed, 15);
    assert_eq!(report.steps, 64);
    assert_eq!(report.objective_calls, 96);
    assert_eq!(report.successes, 5);
    assert_eq!(report.stagnation, 3);
    assert_eq!(report.diagnostic_count, 1);
    assert_eq!(report.component_count, 1);
    assert_eq!(report.kotlin_v1_compatible, Some(true));
    assert_eq!(report.kotlin_v1_reason, None);
    engine.release(&view.id).unwrap();

    // 审计不兼容是**数据**：请求成功，报告给出原因。
    // An incompatible audit is **data**: the request succeeds and the report carries the reason.
    let mut scheduled = serde_json::from_str::<serde_json::Value>(&checkpoint).unwrap();
    scheduled["schedule"] = serde_json::json!({ "cursor": 0, "log": {
        "configuration": "config:fingerprint", "problem": "problem:fingerprint",
        "events": [], "checksum": "" } });
    let view = verify("checkpoint", &scheduled.to_string(), true).unwrap();
    let finished = settle(&engine, &view);
    assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
    let report: CheckpointReport =
        serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
    assert!(report.has_schedule);
    assert_eq!(report.kotlin_v1_compatible, Some(false));
    assert!(report.kotlin_v1_reason.as_deref().unwrap().contains("schedule replay state"));
    engine.release(&view.id).unwrap();

    // 非平稳选择器（实测类型名是 `frr-mab` 一族）+ 自适应奖励，都点名。
    // A non-stationary selector (measured type names are the `frr-mab` family) and adaptive rewards both name themselves.
    let mut flaky = serde_json::from_str::<serde_json::Value>(&checkpoint).unwrap();
    flaky["policy"]["root"]["type"] = serde_json::json!("frr-mab");
    let view = verify("checkpoint", &flaky.to_string(), true).unwrap();
    let finished = settle(&engine, &view);
    let report: CheckpointReport =
        serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
    assert!(report.kotlin_v1_reason.as_deref().unwrap().contains("non-stationary selector"));
    engine.release(&view.id).unwrap();

    let mut adaptive = serde_json::from_str::<serde_json::Value>(&checkpoint).unwrap();
    adaptive["configurationData"]["learning"] = serde_json::json!({ "reward": { "type": "hypervolume-delta" } });
    let view = verify("checkpoint", &adaptive.to_string(), true).unwrap();
    let finished = settle(&engine, &view);
    let report: CheckpointReport =
        serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
    assert!(report.kotlin_v1_reason.as_deref().unwrap().contains("adaptive rewards"));
    engine.release(&view.id).unwrap();

    // 非法文档：未知字段 / 版本非 1 / 非 JSON / 未知类别 / 对策略档案请求审计 / 缺请求。
    // Illegal documents: an unknown field, a non-1 version, non-JSON, an unknown kind, an audit on a policy archive.
    let mut unknown = serde_json::from_str::<serde_json::Value>(&policy).unwrap();
    unknown["nonsense"] = serde_json::json!(1);
    let error = verify("policy", &unknown.to_string(), false).unwrap_err();
    assert_eq!(error.code, "InvalidInput");
    assert!(error.message.contains("unknown field"), "{}", error.message);
    let mut wrong = serde_json::from_str::<serde_json::Value>(&policy).unwrap();
    wrong["version"] = serde_json::json!(2);
    assert!(verify("policy", &wrong.to_string(), false).unwrap_err().message.contains("format version 1"));
    assert_eq!(verify("policy", "not json", false).unwrap_err().code, "InvalidInput");
    assert_eq!(verify("run", &policy, false).unwrap_err().code, "InvalidInput");
    assert!(verify("policy", &policy, true).unwrap_err().message.contains("applies to checkpoints"));
    let mut missing = base_request();
    missing.model_kind = Some(ModelKind::CheckpointVerify);
    assert_eq!(engine.submit(missing).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(directory.join("checkpoint-policy.json"), policy.as_bytes()).unwrap();
        std::fs::write(directory.join("checkpoint-run.json"), checkpoint.as_bytes()).unwrap();
        std::fs::write(directory.join("checkpoint-scheduled.json"), scheduled.to_string().as_bytes()).unwrap();
        std::fs::write(directory.join("checkpoint-nonstationary.json"), flaky.to_string().as_bytes()).unwrap();
    }
}
#[test]
fn linear_mapping_verify_entry_exposes_stable_ids_and_refuses_foreign_claims() {
    use ospf_rust_bridge::{LinearMappingReport, LinearMappingRequest};
    use ospf_rust_core::model::{ConstraintRelation, MetaModel};
    use ospf_rust_core::variable::{ContinuousVariableItem, VariableRange};

    // 两个变量一约束，走真实的 `MetaModel` 构造链。 / Two variables and one constraint via the real `MetaModel` chain.
    let build = |name: &str| {
        let mut source = MetaModel::<f64>::new(name);
        let left = source.add_variable(ContinuousVariableItem::auto_with_range(
            "x", VariableRange::bounded(0.0, 1.0))).unwrap();
        let right = source.add_variable(ContinuousVariableItem::auto_with_range(
            "y", VariableRange::bounded(0.0, 1.0))).unwrap();
        source.add_linear_constraint(&[(left, 1.0), (right, 1.0)],
            ConstraintRelation::LessEqual, 1.0, "budget").unwrap();
        ospf_rust_bridge::encode_model(&source.try_to_linear_triad_model().unwrap()).unwrap()
    };

    let engine = NativeEngine::default();
    let verify = |artifact: ospf_rust_bridge::ModelArtifact, claimed: Option<String>| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::LinearMappingVerify);
        request.model = artifact;
        request.linear_mapping_request = claimed.map(|claimed| LinearMappingRequest { claimed_mapping: Some(claimed) });
        engine.submit(request)
    };

    let view = verify(build("probe"), None).unwrap();
    let finished = settle(&engine, &view);
    assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
    let report: LinearMappingReport =
        serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
    assert_eq!(report.variable_count, 2);
    assert_eq!(report.constraint_count, 1);
    assert_eq!(report.quadratic_constraint_count, 0);
    // 实测的稳定 ID 形状：长度前缀分帧 `linear-variable:5#probe:1#0:1#x`。
    // The measured stable ID shape: length-prefixed framing `linear-variable:5#probe:1#0:1#x`.
    assert!(report.variables_by_column[0].starts_with("linear-variable:"),
        "{}", report.variables_by_column[0]);
    assert!(report.variables_by_column[0].contains("#probe"), "{}", report.variables_by_column[0]);
    assert!(report.variables_by_column[0].ends_with("#x"), "{}", report.variables_by_column[0]);
    assert!(report.variables_by_column[1].ends_with("#y"), "{}", report.variables_by_column[1]);
    assert!(report.constraints_by_row[0].starts_with("linear-constraint:"),
        "{}", report.constraints_by_row[0]);
    assert!(report.objective_id.starts_with("linear-objective:"), "{}", report.objective_id);
    let canonical = report.canonical_json.clone();
    engine.release(&view.id).unwrap();

    // 规范文本再提交：一致（往返稳定）。 / Re-submitting the canonical text agrees (stable round trip).
    let view = verify(build("probe"), Some(canonical.clone())).unwrap();
    let finished = settle(&engine, &view);
    assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
    let report: LinearMappingReport =
        serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
    assert_eq!(report.canonical_json, canonical);
    engine.release(&view.id).unwrap();

    // 换模型名：声称的旧映射不再成立。 / A different model name: the claimed old mapping no longer holds.
    let error = verify(build("other"), Some(canonical.clone())).unwrap_err();
    assert_eq!(error.code, "InvalidInput");
    assert!(error.message.contains("objective disagrees with the recomputation"), "{}", error.message);

    // 指纹不符 / 模型文本损坏 / 声称的映射非法。
    // A fingerprint mismatch, corrupted model text and an illegal claimed mapping.
    let mut mismatch = build("probe");
    mismatch.fingerprint = "foreign".to_owned();
    assert!(verify(mismatch, None).unwrap_err().message.contains("could not be decoded"));
    let mut garbage = build("probe");
    garbage.model_json = "not json".to_owned();
    assert!(verify(garbage, None).unwrap_err().message.contains("could not be decoded"));
    assert_eq!(verify(build("probe"), Some("not json".to_owned())).unwrap_err().code, "InvalidInput");
    assert_eq!(verify(build("probe"), Some("{}".to_owned())).unwrap_err().code, "InvalidInput");

    // 交换两个变量：领域自己先报"非双射"。
    // Swapping two variables: the domain reports the non-bijection itself.
    let mut swapped: serde_json::Value = serde_json::from_str(&canonical).unwrap();
    let first = swapped["variablesByColumn"][0].clone();
    swapped["variablesByColumn"][0] = swapped["variablesByColumn"][1].clone();
    swapped["variablesByColumn"][1] = first;
    assert!(verify(build("probe"), Some(swapped.to_string())).unwrap_err().message.contains("not bijective"));

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(directory.join("linear-mapping.json"), canonical.as_bytes()).unwrap();
    }
}
#[test]
fn lbb_resume_identity_verify_entry_preflights_without_a_checkpoint() {
    use ospf_rust_bridge::{CancellationInput, FingerprintInput, LbbResumeIdentityReport,
        LbbResumeIdentityRequest, ProvenanceInput};

    let print = |value: &str| FingerprintInput {
        schema_version: "1.0".to_owned(), algorithm: "sha256".to_owned(), value: value.to_owned() };
    let identity = |run_id: &str| LbbResumeIdentityRequest {
        run_id: run_id.to_owned(),
        source_attempt_id: "attempt-1".to_owned(),
        source_parent_attempt_id: None,
        model: print("model-a"), configuration: print("config-a"), solver: print("solver-a"),
        cp_snapshot: print("cp-a"), master: print("master-a"), subproblem_factory: print("factory-a"),
        provenance: ProvenanceInput { solver_id: "scip".to_owned(), backend_name: "scip".to_owned(),
            backend_version: Some("8.0".to_owned()), plugin_version: None },
        cancellation_chain: vec![
            CancellationInput { origin: "user".to_owned(), requested_at_epoch_ms: 1, reason: Some("stop".into()) },
            // 词表之外的私有取值：无损回显。 / A private value outside the vocabulary: lossless echo.
            CancellationInput { origin: "tenantShutdown".to_owned(), requested_at_epoch_ms: 2, reason: None },
        ],
    };

    let engine = NativeEngine::default();
    let verify = |request_identity: LbbResumeIdentityRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::LbbResumeIdentityVerify);
        request.lbb_resume_identity = Some(request_identity);
        engine.submit(request)
    };

    let view = verify(identity("run-1")).unwrap();
    let finished = settle(&engine, &view);
    assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
    let report: LbbResumeIdentityReport =
        serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
    assert_eq!(report.run_id, "run-1");
    assert_eq!(report.source_attempt_id, "attempt-1");
    assert_eq!(report.source_parent_attempt_id, None);
    assert_eq!(report.fingerprint_count, 6);
    // 取消链只回显、不参与校验。 / The chain is echoed and not validated.
    assert_eq!(report.cancellation_count, 2);
    assert_eq!(report.cancellation_origins, vec!["user".to_owned(), "tenantShutdown".to_owned()]);
    assert_eq!(report.provenance_solver_id, "scip");
    assert!(report.has_backend_version);
    assert!(!report.has_plugin_version);
    let canonical = report.canonical_json.clone();
    engine.release(&view.id).unwrap();

    // 规范文本再提交：逐字一致。 / Re-submitting the canonical text is byte-identical.
    let again: LbbResumeIdentityRequest = serde_json::from_str(&canonical).unwrap();
    let view = verify(again.clone()).unwrap();
    let finished = settle(&engine, &view);
    let report: LbbResumeIdentityReport =
        serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
    assert_eq!(report.canonical_json, canonical);
    engine.release(&view.id).unwrap();

    // 各自点名：空白运行 ID / 父 attempt 等于源 / 空白 provenance / 指纹空白段。
    // Each names itself: a blank run ID, a parent equal to the source, blank provenance, a blank fingerprint part.
    let error = verify(identity("  ")).unwrap_err();
    assert_eq!(error.code, "InvalidInput");
    assert!(error.message.contains("run and source attempt identities cannot be blank"), "{}", error.message);
    let mut parent_same = identity("run-1");
    parent_same.source_parent_attempt_id = Some("attempt-1".to_owned());
    assert!(verify(parent_same).unwrap_err().message.contains("source parent cannot equal the source attempt"));
    let mut blank_provenance = identity("run-1");
    blank_provenance.provenance.backend_name = String::new();
    assert!(verify(blank_provenance).unwrap_err().message.contains("provenance identity cannot be blank"));
    for part in ["schema", "algorithm", "value"] {
        let mut broken = identity("run-1");
        match part {
            "schema" => broken.model.schema_version = " ".to_owned(),
            "algorithm" => broken.master.algorithm = String::new(),
            _ => broken.subproblem_factory.value = "  ".to_owned(),
        }
        let error = verify(broken).unwrap_err();
        assert!(error.message.contains("cannot contain blank fields"), "{part}: {}", error.message);
    }

    // 缺请求。 / A missing request.
    let mut missing = base_request();
    missing.model_kind = Some(ModelKind::LbbResumeIdentityVerify);
    assert_eq!(engine.submit(missing).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(directory.join("lbb-resume-identity.json"), canonical.as_bytes()).unwrap();
    }
}
#[test]
fn cp_checkpoint_verify_entry_projects_the_typed_state() {
    use ospf_rust_bridge::{CpCheckpointReport, CpCheckpointRequest};
    use ospf_rust_core::model::constraint_programming::{
        ConstraintDefinition, ConstraintProgrammingConstraint, ConstraintProgrammingModel,
        ConstraintProgrammingSnapshot, IntegerDomain, IntegerExpression, IntegerObjective, IntegerRelation,
        IntegerVariable,
    };
    use ospf_rust_core::solver::checkpoint::SolveCheckpoint;
    use ospf_rust_core::solver::fingerprint::sha256_fingerprint;
    use ospf_rust_core::solver::report::{AuditFingerprint, SolverProvenance,
        CURRENT_SOLVE_REPORT_SCHEMA_VERSION};
    use ospf_rust_core::ConstraintProgrammingCheckpointArtifact;

    let fingerprint = |value: &str| AuditFingerprint {
        schema_version: CURRENT_SOLVE_REPORT_SCHEMA_VERSION.to_owned(),
        algorithm: "sha256".to_owned(), value: value.to_owned() };

    // 用核心自己的构造器造快照与检查点（真实产物，不是手写内部结构）。
    // Build the snapshot and checkpoint with the core's own constructors (real output, not hand-written internals).
    let snapshot = {
        let x = IntegerVariable::new("x");
        let mut model = ConstraintProgrammingModel::new("cp-checkpoint-native");
        model.register_variable(x.clone(), IntegerDomain::range(0, 5).unwrap()).unwrap();
        model.add_constraint(ConstraintDefinition::new("capacity",
            ConstraintProgrammingConstraint::integer(
                IntegerExpression::variable(x.clone()), IntegerRelation::LessOrEqual, 5))).unwrap();
        model.set_objective(IntegerObjective::maximize(IntegerExpression::variable(x)));
        let _: ConstraintProgrammingSnapshot = model.freeze().unwrap();
        model.freeze().unwrap()
    };
    let state = br#"{"iteration":4}"#.to_vec();
    let checkpoint = SolveCheckpoint::new(
        "run-cp", "attempt-cp", None,
        snapshot.fingerprint.clone(), fingerprint("config-cp"), fingerprint("solver-cp"),
        SolverProvenance { solver_id: "cp".to_owned(), backend_name: "cp".to_owned(),
            backend_version: None, thread_count: Some(1), ..Default::default() },
        4, Some(2.0), Some(3.0), Some(0.5),
        sha256_fingerprint("ospf.solve.checkpoint.state", &state)).unwrap();
    let artifact = ConstraintProgrammingCheckpointArtifact::new_direct(
        checkpoint, &snapshot, Vec::new(), Default::default(),
        [("backend".to_owned(), "fake".to_owned())].into_iter().collect()).unwrap();
    let payload = String::from_utf8(artifact.to_json().unwrap()).unwrap();

    let engine = NativeEngine::default();
    let verify = |text: String| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::CpCheckpointVerify);
        request.cp_checkpoint_request = Some(CpCheckpointRequest { payload: text });
        engine.submit(request)
    };

    let view = verify(payload.clone()).unwrap();
    let finished = settle(&engine, &view);
    assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
    let report: CpCheckpointReport =
        serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
    assert_eq!(report.schema_version, "1.0");
    assert_eq!(report.run_id, "run-cp");
    assert_eq!(report.attempt_id, "attempt-cp");
    assert_eq!(report.state_schema_version, "1.0");
    assert_eq!(report.restore_mode, "RebuildFromSnapshot");
    assert_eq!(report.engine, "direct-cp");
    assert_eq!(report.iteration, 4);
    assert_eq!(report.assumption_count, 0);
    assert_eq!(report.solution_hint_count, 0);
    // primitive state 的键投影出来，值不外泄。 / Primitive-state keys are projected; values are not.
    assert_eq!(report.primitive_state_keys, vec!["backend".to_owned()]);
    assert_eq!(report.snapshot_variable_count, 1);
    assert_eq!(report.snapshot_constraint_count, 1);
    assert_eq!(report.snapshot_fingerprint.len(), 64);
    assert!(report.canonical_input);
    let canonical = report.canonical_payload.clone();
    engine.release(&view.id).unwrap();

    // 规范载荷再提交：逐字一致。 / Re-submitting the canonical payload is byte-identical.
    let view = verify(canonical.clone()).unwrap();
    let finished = settle(&engine, &view);
    let report: CpCheckpointReport =
        serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
    assert_eq!(report.canonical_payload, canonical);
    engine.release(&view.id).unwrap();

    // 非规范 JSON：领域要求规范字节，拒绝。 / Non-canonical JSON: the domain requires canonical bytes.
    let pretty = canonical.replacen("{", "{ ", 1);
    let error = verify(pretty).unwrap_err();
    assert_eq!(error.code, "InvalidInput");
    assert!(error.message.contains("failed validation"), "{}", error.message);
    assert!(verify("not json".to_owned()).unwrap_err().message.contains("failed validation"));

    // 篡改类型化 state：schema 版本、engine 空白、primitive-state 键空白，全部拒绝。
    // Tampering with the typed state: schema version, blank engine, blank primitive-state key — all refused.
    let mut wrong_schema: serde_json::Value = serde_json::from_str(&canonical).unwrap();
    wrong_schema["state"]["schemaVersion"] = serde_json::json!("9.9");
    assert!(verify(wrong_schema.to_string()).is_err());
    let mut blank_engine: serde_json::Value = serde_json::from_str(&canonical).unwrap();
    blank_engine["state"]["engine"] = serde_json::json!("  ");
    assert!(verify(blank_engine.to_string()).is_err());
    let mut blank_key: serde_json::Value = serde_json::from_str(&canonical).unwrap();
    blank_key["state"]["primitiveState"] = serde_json::json!({ "  ": "value" });
    assert!(verify(blank_key.to_string()).is_err());

    // 缺请求。 / A missing request.
    let mut missing = base_request();
    missing.model_kind = Some(ModelKind::CpCheckpointVerify);
    assert_eq!(engine.submit(missing).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(directory.join("cp-portable-checkpoint.json"), canonical.as_bytes()).unwrap();
    }
}
#[test]
fn benders_metrics_verify_entry_validates_and_projects() {
    use ospf_rust_bridge::{BendersMetricsReport, BendersMetricsRequest, BendersSnapshotInput};

    let snapshot = |iteration: u64, cuts_added: u64, total_cuts: u64| BendersSnapshotInput {
        iteration, master_obj: -1.5, master_gap: Some(0.25), master_bound: Some(-2.0),
        cuts_added, total_cuts, no_cut_iterations: 0, no_obj_improvement_iterations: 0,
        master_attempt_id: format!("master-{iteration}"),
        subproblem_attempt_id: Some(format!("sub-{iteration}")),
        bound_valid: true, proof_reference: None, stop_reason: None,
    };
    let metrics = |stop: Option<&str>| BendersMetricsRequest {
        executed_iterations: 3, best_solution_iteration: Some(2), total_cuts: 5,
        no_cut_iterations: 0, no_obj_improvement_iterations: 1,
        stop_reason: stop.map(str::to_owned),
        iteration_snapshots: vec![snapshot(1, 2, 2), snapshot(2, 3, 5), snapshot(3, 0, 5)],
    };

    let engine = NativeEngine::default();
    let verify = |document: BendersMetricsRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::BendersMetricsVerify);
        request.benders_metrics = Some(document);
        engine.submit(request)
    };

    let view = verify(metrics(Some("iterationLimit"))).unwrap();
    let finished = settle(&engine, &view);
    assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
    let report: BendersMetricsReport =
        serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
    assert_eq!(report.executed_iterations, 3);
    assert_eq!(report.total_cuts, 5);
    assert_eq!(report.best_solution_iteration, Some(2));
    assert_eq!(report.stop_reason.as_deref(), Some("iterationLimit"));
    assert_eq!(report.stop_category, "executed");
    assert_eq!(report.snapshot_count, 3);
    assert_eq!(report.snapshots[1].cuts_added, 3);
    assert_eq!(report.snapshots[1].master_attempt_id, "master-2");
    // 词表里既有纯桥接代码，也有嵌套终止原因的完整交叉。
    // The vocabulary holds the bridge-only codes and both nested families.
    assert!(report.stop_reason_vocabulary.contains(&"cutStall".to_owned()));
    assert!(report.stop_reason_vocabulary.contains(&"masterTermination:COMPLETED".to_owned()));
    assert!(report.stop_reason_vocabulary.contains(&"subproblemTermination:UNKNOWN".to_owned()));
    engine.release(&view.id).unwrap();

    // 嵌套终止原因：两类都能命名。 / Both nested families name their reason.
    let view = verify(metrics(Some("masterTermination:TIME_LIMIT"))).unwrap();
    let finished = settle(&engine, &view);
    let report: BendersMetricsReport =
        serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
    assert_eq!(report.stop_category, "masterTermination");
    assert_eq!(report.nested_termination_reason.as_deref(), Some("TIME_LIMIT"));
    engine.release(&view.id).unwrap();
    let view = verify(metrics(Some("subproblemTermination:CANCELLED"))).unwrap();
    let finished = settle(&engine, &view);
    let report: BendersMetricsReport =
        serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
    assert_eq!(report.stop_category, "subproblemTermination");
    assert_eq!(report.nested_termination_reason.as_deref(), Some("CANCELLED"));
    engine.release(&view.id).unwrap();

    // 无停止原因。 / No stop reason.
    let view = verify(metrics(None)).unwrap();
    let finished = settle(&engine, &view);
    let report: BendersMetricsReport =
        serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
    assert_eq!(report.stop_category, "none");
    engine.release(&view.id).unwrap();

    // 不一致全部拒绝。 / Every inconsistency is refused.
    let mut out_of_range = metrics(None);
    out_of_range.best_solution_iteration = Some(9);
    assert!(verify(out_of_range).unwrap_err().message.contains("place the best solution outside"));
    let mut stall = metrics(None);
    stall.no_obj_improvement_iterations = 9;
    assert!(verify(stall).unwrap_err().message.contains("more stall iterations"));
    let mut out_of_order = metrics(None);
    out_of_order.iteration_snapshots.swap(0, 1);
    assert!(verify(out_of_order).unwrap_err().message.contains("strictly increasing"));
    let mut wrong_total = metrics(None);
    wrong_total.total_cuts = 6;
    assert!(verify(wrong_total).unwrap_err().message.contains("disagree with the last snapshot"));
    let mut wrong_sum = metrics(None);
    wrong_sum.iteration_snapshots[1].cuts_added = 4;
    assert!(verify(wrong_sum).unwrap_err().message.contains("do not sum to the total"));
    let mut no_snapshots = metrics(None);
    no_snapshots.iteration_snapshots.clear();
    assert!(verify(no_snapshots).unwrap_err().message.contains("without any iteration snapshot"));
    let mut non_finite = metrics(None);
    non_finite.iteration_snapshots[0].master_obj = f64::INFINITY;
    assert!(verify(non_finite).unwrap_err().message.contains("non-finite master objective"));
    let mut unknown_stop = metrics(Some("timeoutStall"));
    unknown_stop.iteration_snapshots.clear();
    unknown_stop.total_cuts = 0;
    assert!(verify(unknown_stop).unwrap_err().message.contains("unknown stop reason"));
    let mut unknown_nested = metrics(Some("masterTermination:NOT_A_REASON"));
    unknown_nested.iteration_snapshots.clear();
    unknown_nested.total_cuts = 0;
    assert!(verify(unknown_nested).unwrap_err().message.contains("unknown termination reason"));

    // 缺请求。 / A missing request.
    let mut missing = base_request();
    missing.model_kind = Some(ModelKind::BendersMetricsVerify);
    assert_eq!(engine.submit(missing).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        let payload = serde_json::to_string(&metrics(Some("masterTermination:TIME_LIMIT"))).unwrap();
        std::fs::write(directory.join("benders-metrics.json"), payload.as_bytes()).unwrap();
    }
}
#[test]
fn benders_cut_verify_entry_round_trips_through_the_domain_converter() {
    use ospf_rust_bridge::{BendersCutCoefficient, BendersCutReport, BendersCutRequest, BendersCutSense,
        BendersCutSpec, BendersQuadraticCutSpec, BendersQuadraticTerm};

    let coefficient = |column: u32, value: f64| BendersCutCoefficient { column, coefficient: value };
    let document = || BendersCutRequest {
        column_count: 4,
        linear: vec![
            BendersCutSpec { name: "budget".to_owned(),
                coefficients: vec![coefficient(0, 1.0), coefficient(2, -2.5)],
                sense: BendersCutSense::LessOrEqual, rhs: 10.0 },
            BendersCutSpec { name: "floor".to_owned(), coefficients: vec![coefficient(3, 1.0)],
                sense: BendersCutSense::GreaterOrEqual, rhs: 0.0 },
        ],
        quadratic: vec![BendersQuadraticCutSpec { name: "pair".to_owned(),
            linear: vec![coefficient(1, 0.5)],
            quadratic: vec![BendersQuadraticTerm { left: 0, right: 1, coefficient: 1.0 }],
            sense: BendersCutSense::Equal, rhs: 2.0 }],
    };

    let engine = NativeEngine::default();
    let verify = |document: BendersCutRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::BendersCutVerify);
        request.benders_cut_request = Some(document);
        engine.submit(request)
    };

    let view = verify(document()).unwrap();
    let finished = settle(&engine, &view);
    assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
    let report: BendersCutReport =
        serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
    assert_eq!(report.linear_cut_count, 2);
    assert_eq!(report.quadratic_cut_count, 1);
    assert_eq!(report.linear_term_count, 3);
    assert_eq!(report.quadratic_term_count, 1);
    // 列覆盖：0,1,2,3 全被引用。 / Column coverage: 0, 1, 2 and 3 are all referenced.
    assert_eq!(report.distinct_column_count, 4);
    assert_eq!(report.max_column, Some(3));
    assert_eq!(report.linear[0].name, "budget");
    assert_eq!(report.linear[0].sense, "lessOrEqual");
    assert_eq!(report.linear[0].first_column, Some(0));
    assert_eq!(report.linear[0].last_column, Some(2));
    assert_eq!(report.quadratic[0].quadratic_terms, 1);
    assert_eq!(report.quadratic[0].last_column, Some(1));
    // 重建的线格式与输入逐字一致（没有静默丢弃或重排）。
    // The rebuilt wire format equals the input byte for byte (nothing dropped or reordered).
    let rebuilt: serde_json::Value = serde_json::from_str(&report.canonical_json).unwrap();
    assert_eq!(rebuilt["linear"], serde_json::to_value(&document().linear).unwrap());
    assert_eq!(rebuilt["quadratic"], serde_json::to_value(&document().quadratic).unwrap());
    engine.release(&view.id).unwrap();

    // 领域规则逐条。 / The domain rules one by one.
    let mut blank = document();
    blank.linear[0].name = "  ".to_owned();
    assert!(verify(blank).unwrap_err().message.contains("names must not be blank"));
    let mut repeated = document();
    repeated.linear[1].name = "budget".to_owned();
    assert!(verify(repeated).unwrap_err().message.contains("repeat a name"));
    let mut outside = document();
    outside.linear[1].coefficients = vec![coefficient(4, 1.0)];
    assert!(verify(outside).unwrap_err().message.contains("outside the model"));
    let mut empty = document();
    empty.linear[1].coefficients.clear();
    assert!(verify(empty).unwrap_err().message.contains("at least one coefficient"));
    let mut summed = document();
    summed.linear[1].coefficients = vec![coefficient(3, 1.0), coefficient(3, 2.0)];
    assert!(verify(summed).unwrap_err().message.contains("repeats a column"));
    let mut infinite_rhs = document();
    infinite_rhs.linear[1].rhs = f64::INFINITY;
    assert!(verify(infinite_rhs).unwrap_err().message.contains("right-hand sides must be finite"));
    // 空文档合法。 / An empty document is legal.
    let view = verify(BendersCutRequest { column_count: 0, linear: Vec::new(), quadratic: Vec::new() }).unwrap();
    let finished = settle(&engine, &view);
    let report: BendersCutReport =
        serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
    assert_eq!(report.linear_cut_count, 0);
    assert_eq!(report.max_column, None);
    engine.release(&view.id).unwrap();

    // 缺请求。 / A missing request.
    let mut missing = base_request();
    missing.model_kind = Some(ModelKind::BendersCutVerify);
    assert_eq!(engine.submit(missing).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        let payload = serde_json::to_string(&document()).unwrap();
        std::fs::write(directory.join("benders-cuts.json"), payload.as_bytes()).unwrap();
    }
}
#[test]
fn symbol_condition_verify_entry_classifies_without_evaluating() {
    use ospf_rust_bridge::{SymbolConditionBounds, SymbolConditionReport, SymbolConditionRequest};

    let request_of = |relation: &str, boundary: f64, lower: f64, upper: f64,
        differences: Vec<f64>| SymbolConditionRequest {
        relation: relation.to_owned(), strict_boundary: boundary,
        bounds: SymbolConditionBounds { lower, upper }, differences,
    };

    let engine = NativeEngine::default();
    let verify = |document: SymbolConditionRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::SymbolConditionVerify);
        request.symbol_condition = Some(document);
        engine.submit(request)
    };

    // 范围钉死在真分支：只做判定，不建变量也不求值。 / A pinned range: decided, with no variable and no evaluation.
    let view = verify(request_of("greater", 2.0, 2.0, 10.0, Vec::new())).unwrap();
    let finished = settle(&engine, &view);
    assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
    let report: SymbolConditionReport =
        serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
    assert_eq!(report.relation, "greater");
    assert_eq!(report.coverage, "true");
    assert!(report.decidable);
    assert_eq!(report.relation_vocabulary,
        vec!["greater".to_owned(), "greaterEqual".to_owned(), "less".to_owned(), "lessEqual".to_owned()]);
    assert_eq!(report.truth_vocabulary,
        vec!["true".to_owned(), "false".to_owned(), "undefined".to_owned()]);
    engine.release(&view.id).unwrap();

    // 三值分类：`greater` 的边界规则逐条。 / Three-valued classification: the `greater` boundary rules.
    let view = verify(request_of("greater", 2.0, -1.0, 5.0, vec![2.0, 3.0, 0.0, -1.0, 1.0])).unwrap();
    let finished = settle(&engine, &view);
    let report: SymbolConditionReport =
        serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
    let truths: Vec<&str> = report.samples.iter().map(|sample| sample.truth.as_str()).collect();
    assert_eq!(truths, vec!["true", "true", "false", "false", "undefined"]);
    assert_eq!((report.true_count, report.false_count, report.undefined_count), (2, 2, 1));
    assert_eq!(report.coverage, "undefined");
    assert!(!report.decidable);
    engine.release(&view.id).unwrap();

    // 四种关系的三值规则交叉。 / The three-valued rules across all four relations.
    for (relation, boundary, lower, upper, differences, expected) in [
        ("greaterEqual", 2.0, -5.0, 5.0, vec![0.0, -3.0, -1.0], vec!["true", "false", "undefined"]),
        ("less", 2.0, -5.0, 5.0, vec![-2.0, 0.0, -1.0], vec!["true", "false", "undefined"]),
        ("lessEqual", 2.0, -5.0, 5.0, vec![0.0, 2.0, 1.0], vec!["true", "false", "undefined"]),
    ] {
        let view = verify(request_of(relation, boundary, lower, upper, differences)).unwrap();
        let finished = settle(&engine, &view);
        let report: SymbolConditionReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        let truths: Vec<&str> = report.samples.iter().map(|sample| sample.truth.as_str()).collect();
        assert_eq!(truths, expected, "{relation}");
        engine.release(&view.id).unwrap();
    }

    // 非法参数逐条拒绝。 / Every illegal parameter is refused.
    assert!(verify(request_of("greater", 0.0, 0.0, 5.0, Vec::new())).unwrap_err()
        .message.contains("positive"));
    assert!(verify(request_of("greater", -1.0, 0.0, 5.0, Vec::new())).unwrap_err()
        .message.contains("positive"));
    assert!(verify(request_of("greater", 1.0, 5.0, 0.0, Vec::new())).unwrap_err()
        .message.contains("must not exceed"));
    // 整个范围落在 `Undefined` 间隔内：拒绝。 / A range wholly inside the `Undefined` interval is refused.
    assert!(verify(request_of("greater", 3.0, 0.5, 2.5, Vec::new())).unwrap_err()
        .message.contains("Undefined interval"));
    assert!(verify(request_of("equal", 1.0, 0.0, 5.0, Vec::new())).unwrap_err()
        .message.contains("unknown relation"));
    assert!(verify(request_of("greater", 1.0, -5.0, 5.0, vec![f64::NAN])).unwrap_err()
        .message.contains("must be finite"));

    // 缺请求。 / A missing request.
    let mut missing = base_request();
    missing.model_kind = Some(ModelKind::SymbolConditionVerify);
    assert_eq!(engine.submit(missing).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        let payload = serde_json::to_string(&request_of("greater", 2.0, -1.0, 5.0,
            vec![2.0, 3.0, 0.0, -1.0, 1.0])).unwrap();
        std::fs::write(directory.join("symbol-condition.json"), payload.as_bytes()).unwrap();
    }
}
#[test]
fn symbol_linearization_verify_entry_reports_the_calibration_knob() {
    use ospf_rust_bridge::{SymbolLinearizationReport, SymbolLinearizationRequest};

    let request_of = |big_m: Option<f64>, methods: &[&str]| SymbolLinearizationRequest { big_m,
        methods: methods.iter().map(|method| (*method).to_owned()).collect() };

    let engine = NativeEngine::default();
    let verify = |document: SymbolLinearizationRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::SymbolLinearizationVerify);
        request.symbol_linearization = Some(document);
        engine.submit(request)
    };

    // 缺省：默认 Big-M，未触发下限，四种方法都在。 / Omitted: the default big-M, no clamp, all four methods.
    let view = verify(request_of(None, &[])).unwrap();
    let finished = settle(&engine, &view);
    assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
    let report: SymbolLinearizationReport =
        serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
    assert_eq!(report.default_big_m, 1_000_000.0);
    assert_eq!(report.min_big_m, 1.0);
    assert_eq!(report.effective_big_m, 1_000_000.0);
    assert!(!report.clamped_to_floor);
    assert_eq!(report.methods.len(), 4);
    // `indicator` 与 `sos1` 在机制层都被归一成 `bigM`。 / `indicator` and `sos1` both normalise to `bigM`.
    for code in ["indicator", "sos1"] {
        let row = report.methods.iter().find(|row| row.method == code).unwrap();
        assert_eq!(row.mechanism_equivalent, "bigM", "{code}");
        assert!(row.normalised, "{code}");
    }
    let threshold = report.methods.iter().find(|row| row.method == "threshold").unwrap();
    assert!(!threshold.normalised);
    assert_eq!(threshold.when_true_rhs, 0.0);
    assert!(threshold.when_false_rhs < 0.0);
    engine.release(&view.id).unwrap();

    // 低于下限但为正：被抬到下限并标出。 / Positive but below the floor: raised and flagged.
    let view = verify(request_of(Some(0.25), &["bigM"])).unwrap();
    let finished = settle(&engine, &view);
    let report: SymbolLinearizationReport =
        serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
    assert_eq!(report.requested_big_m, 0.25);
    assert_eq!(report.effective_big_m, 1.0);
    assert!(report.clamped_to_floor);
    assert_eq!(report.methods.len(), 1);
    assert_eq!(report.method_vocabulary.len(), 4);
    engine.release(&view.id).unwrap();

    // 合法的大值原样放行。 / A legal large value passes through.
    let view = verify(request_of(Some(500.0), &["threshold"])).unwrap();
    let finished = settle(&engine, &view);
    let report: SymbolLinearizationReport =
        serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
    assert_eq!(report.effective_big_m, 500.0);
    assert!(!report.clamped_to_floor);
    engine.release(&view.id).unwrap();

    // 非有限、零、负数报错。 / Non-finite, zero and negative values fail.
    for value in [0.0, -1.0, f64::INFINITY] {
        assert!(verify(request_of(Some(value), &[])).unwrap_err()
            .message.contains("finite and positive"), "{value}");
    }
    // 未知方法。 / An unknown method.
    assert!(verify(request_of(None, &["simplex"])).unwrap_err()
        .message.contains("unknown binaryzation method"));

    // 缺请求。 / A missing request.
    let mut missing = base_request();
    missing.model_kind = Some(ModelKind::SymbolLinearizationVerify);
    assert_eq!(engine.submit(missing).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        let payload = serde_json::to_string(&SymbolLinearizationRequest {
            big_m: Some(250.0), methods: Vec::new() }).unwrap();
        std::fs::write(directory.join("symbol-linearization.json"), payload.as_bytes()).unwrap();
    }
}
#[test]
fn symbol_core_relations_verify_entry_reads_the_pure_tables() {
    use ospf_rust_bridge::{SymbolCoreRelationsReport, SymbolCoreRelationsRequest};

    let engine = NativeEngine::default();
    let mut request = base_request();
    request.model_kind = Some(ModelKind::SymbolCoreRelationsVerify);
    request.symbol_core_relations = Some(SymbolCoreRelationsRequest {});
    let view = engine.submit(request).unwrap();
    let finished = settle(&engine, &view);
    assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
    let report: SymbolCoreRelationsReport =
        serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();

    assert_eq!(report.kind_vocabulary.len(), 6);
    assert_eq!(report.indicator_tolerance, 1.0e-10);
    assert_eq!(report.step_epsilon, 1.0e-8);
    assert_eq!(report.strict_boundary, report.step_epsilon * 2.0);
    // 六种里支持四种：`equal` / `notEqual` 的取假侧是析取。
    // Four of six are supported: `equal` and `notEqual` have a disjunctive false side.
    assert_eq!(report.supported_kind_count, 4);
    assert_eq!(report.inequalities.len(), 6);
    let unsupported: Vec<&str> = report.inequalities.iter()
        .filter(|row| !row.supported).map(|row| row.kind.as_str()).collect();
    assert_eq!(unsupported, vec!["equal", "notEqual"]);
    for row in report.inequalities.iter().filter(|row| !row.supported) {
        assert!(row.when_true.is_none() && row.when_false.is_none());
        assert!(row.relaxed_lower_rhs.is_none() && row.relaxed_upper_rhs.is_none());
    }
    // 非严格 `lessEqual` 外扩、严格 `less` 内缩。 / Non-strict expands outward, strict shrinks inward.
    let less_equal = report.inequalities.iter().find(|row| row.kind == "lessEqual").unwrap();
    assert_eq!(less_equal.when_false.as_ref().unwrap().rhs, report.indicator_tolerance);
    let strict_less = report.inequalities.iter().find(|row| row.kind == "less").unwrap();
    assert_eq!(strict_less.when_true.as_ref().unwrap().rhs, -report.indicator_tolerance);
    // 蕴含耦合：三条，前提两条。 / Implication coupling: three rows, two on the premise.
    assert_eq!(report.imply.len(), 3);
    assert_eq!(report.imply.iter().filter(|row| row.keyed_on_premise).count(), 2);
    // band：取真一对、两个方向对称、严格边界是两条松弛的右端。
    // The band: a symmetric pair when true, with the strict boundary on both relaxations.
    assert_eq!(report.in_values.when_indicator_true.len(), 2);
    assert_eq!(report.in_values.band_tolerance, report.step_epsilon);
    assert_eq!(report.in_values.relaxed_lower_rhs, report.strict_boundary);
    assert_eq!(report.in_values.relaxed_upper_rhs, report.strict_boundary);
    engine.release(&view.id).unwrap();

    // 这个 kind **不读任何输入**：连字段都不给也能跑。空结构体只是保持"每个 kind 有自己字段"的对称性，
    // 所以强制要求它存在就是纯仪式。 / This kind **reads no input at all**: it runs even without the field. The empty
    // struct only keeps the per-kind field symmetry, so insisting on its presence would be pure ceremony.
    let mut bare = base_request();
    bare.model_kind = Some(ModelKind::SymbolCoreRelationsVerify);
    let view = engine.submit(bare).unwrap();
    let finished = settle(&engine, &view);
    assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
    engine.release(&view.id).unwrap();
}
#[test]
fn heuristic_schedule_sample_entry_curves_the_parameters() {
    use ospf_rust_bridge::{HeuristicScheduleReport, HeuristicScheduleRequest};

    let request_of = |kind: &str, samples: u32| HeuristicScheduleRequest { kind: kind.to_owned(),
        value: None, from: None, to: None, limit: None, initial: None, ratio: None, rate: None,
        amplitude: None, seed: None, samples };

    let engine = NativeEngine::default();
    let sample = |document: HeuristicScheduleRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::HeuristicScheduleSample);
        request.heuristic_schedule = Some(document);
        engine.submit(request)
    };

    // 线性调度：起点 → 终点，逐点线性。 / A linear schedule: start to end, linear per point.
    let mut linear = request_of("linear", 5);
    linear.from = Some(0.0);
    linear.to = Some(1.0);
    linear.limit = Some(4);
    let view = sample(linear).unwrap();
    let finished = settle(&engine, &view);
    assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
    let report: HeuristicScheduleReport =
        serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
    let values: Vec<f64> = report.samples.iter().map(|sample| sample.value).collect();
    assert_eq!(values, vec![0.0, 0.25, 0.5, 0.75, 1.0]);
    assert_eq!(report.min_value, 0.0);
    assert_eq!(report.max_value, 1.0);
    assert!(report.all_finite);
    assert_eq!(report.kind_vocabulary.len(), 5);
    assert_eq!(report.seed, 20_240_521);
    engine.release(&view.id).unwrap();

    // 几何衰减：`initial * ratio^i`。 / Geometric decay.
    let mut geometric = request_of("geometric", 3);
    geometric.initial = Some(100.0);
    geometric.ratio = Some(0.5);
    let view = sample(geometric).unwrap();
    let finished = settle(&engine, &view);
    let report: HeuristicScheduleReport =
        serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
    let values: Vec<f64> = report.samples.iter().map(|sample| sample.value).collect();
    assert_eq!(values, vec![100.0, 50.0, 25.0]);
    engine.release(&view.id).unwrap();

    // 抖动：同种子同曲线，且落在 ±amplitude 内。 / Jitter: one seed one curve, inside the amplitude.
    let mut jittered = request_of("jittered", 8);
    jittered.initial = Some(10.0);
    jittered.ratio = Some(1.0);
    jittered.amplitude = Some(0.5);
    jittered.seed = Some(7);
    let view = sample(jittered.clone()).unwrap();
    let finished = settle(&engine, &view);
    let first: HeuristicScheduleReport =
        serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
    assert_eq!(first.seed, 7);
    assert!(first.samples.iter().any(|sample| sample.value != 10.0));
    assert!(first.samples.iter().all(|sample| (sample.value - 10.0).abs() <= 0.5 + 1e-12));
    engine.release(&view.id).unwrap();
    let view = sample(jittered).unwrap();
    let finished = settle(&engine, &view);
    let again: HeuristicScheduleReport =
        serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
    assert_eq!(first.samples, again.samples);
    engine.release(&view.id).unwrap();

    // 非法参数逐条。 / Every illegal parameter.
    assert!(sample(request_of("constant", 0)).unwrap_err().message.contains("between 1 and 4096"));
    assert!(sample(request_of("constant", 4097)).unwrap_err().message.contains("between 1 and 4096"));
    assert!(sample(request_of("constant", 2)).unwrap_err().message.contains("requires `value`"));
    assert!(sample(request_of("simulated", 2)).unwrap_err().message.contains("unknown shape"));
    let mut infinite = request_of("geometric", 2);
    infinite.initial = Some(f64::INFINITY);
    infinite.ratio = Some(0.5);
    assert!(sample(infinite).unwrap_err().message.contains("must be finite"));
    // 曲线越出有限范围：拒绝而不是报假极值。 / A curve leaving the finite range is refused.
    let mut overflow = request_of("geometric", 4096);
    overflow.initial = Some(1e300);
    overflow.ratio = Some(1e10);
    assert!(sample(overflow).unwrap_err().message.contains("leaves the finite range"));

    // 缺请求。 / A missing request.
    let mut missing = base_request();
    missing.model_kind = Some(ModelKind::HeuristicScheduleSample);
    assert_eq!(engine.submit(missing).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        let payload = serde_json::to_string(&HeuristicScheduleRequest { kind: "geometric".to_owned(),
            value: None, from: None, to: None, limit: None, initial: Some(100.0), ratio: Some(0.5),
            rate: None, amplitude: None, seed: None, samples: 3 }).unwrap();
        std::fs::write(directory.join("heuristic-schedule.json"), payload.as_bytes()).unwrap();
    }
}
#[test]
fn q_table_simulate_entry_replays_the_domain_q_table() {
    use ospf_rust_bridge::{QSelector, QTableReport, QTableRequest, QTransition};

    let transition = |state: u32, action: u32, reward: f64, next_state: u32, terminal: bool| QTransition {
        state, action, reward, next_state, alpha: 1.0, gamma: 1.0, terminal,
        available_actions: None };

    let engine = NativeEngine::default();
    let replay = |document: QTableRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::QTableSimulate);
        request.q_table = Some(document);
        engine.submit(request)
    };

    // 编解码往返 + 并列取最小下标。 / Codec round trip plus ties taking the lowest index.
    let mut document = QTableRequest { state_amount: 2, action_amount: 3,
        initial_values: vec![0.0, 3.0, 3.0, -1.0, 0.5, 0.5], transitions: Vec::new(),
        selector: None, selection_states: Vec::new() };
    let view = replay(document.clone()).unwrap();
    let finished = settle(&engine, &view);
    assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
    let report: QTableReport =
        serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
    assert!(report.initial_supplied);
    assert_eq!(report.final_values, document.initial_values);
    assert_eq!(report.best_actions, vec![1, 1]);
    assert_eq!(report.selector_kind, "greedy");
    engine.release(&view.id).unwrap();

    // 更新用**下一状态**的最大值；终止转移把它归零；掩码参与取最大。
    // The update uses the **next** state's maximum, terminal zeroes it and the mask participates.
    document.transitions = vec![transition(0, 0, 0.0, 1, false)];
    document.initial_values = vec![9.0, 9.0, 9.0, -1.0, 0.5, 0.5];
    let view = replay(document.clone()).unwrap();
    let finished = settle(&engine, &view);
    let report: QTableReport =
        serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
    assert_eq!(report.transition_count, 1);
    assert_eq!(report.transitions[0].updated_value, 0.5);
    engine.release(&view.id).unwrap();
    let mut masked = document.clone();
    let mut sample = transition(0, 0, 0.0, 1, false);
    sample.available_actions = Some(vec![0]);
    masked.transitions = vec![sample];
    let view = replay(masked).unwrap();
    let finished = settle(&engine, &view);
    let report: QTableReport =
        serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
    assert_eq!(report.transitions[0].updated_value, -1.0);
    engine.release(&view.id).unwrap();

    // 选择器：缺省贪心、ε=0 等价贪心、ε=1 同种子可复现。 / Selectors: default greedy, ε=0 greedy, ε=1 reproducible.
    document.initial_values = vec![0.1, 0.2, 0.3, 0.0, 0.0, 0.0];
    document.transitions = Vec::new();
    document.selection_states = vec![0];
    let view = replay(document.clone()).unwrap();
    let finished = settle(&engine, &view);
    let report: QTableReport =
        serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
    assert_eq!(report.selections[0].action, 2);
    engine.release(&view.id).unwrap();
    document.selection_states = vec![0; 8];
    document.selector = Some(QSelector { kind: "epsilonGreedy".to_owned(), epsilon: Some(1.0),
        seed: Some(9) });
    let view = replay(document.clone()).unwrap();
    let finished = settle(&engine, &view);
    let first: QTableReport =
        serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
    assert_eq!(first.seed, 9);
    assert!(first.selections.iter().all(|selection| selection.action < 3));
    engine.release(&view.id).unwrap();
    let view = replay(document).unwrap();
    let finished = settle(&engine, &view);
    let again: QTableReport =
        serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
    assert_eq!(first.selections, again.selections);
    engine.release(&view.id).unwrap();

    // 非法输入逐条。 / Every illegal input.
    let mut wrong_length = QTableRequest { state_amount: 2, action_amount: 2,
        initial_values: vec![0.0, 1.0, 2.0], transitions: Vec::new(), selector: None,
        selection_states: Vec::new() };
    assert!(replay(wrong_length.clone()).unwrap_err().message.contains("exactly 4 values"));
    wrong_length.initial_values = vec![0.0, 1.0, 2.0, f64::INFINITY];
    assert!(replay(wrong_length).unwrap_err().message.contains("shape or values"));
    let mut bad_alpha = QTableRequest { state_amount: 2, action_amount: 2, initial_values: Vec::new(),
        transitions: Vec::new(), selector: None, selection_states: Vec::new() };
    let mut sample = transition(0, 0, 0.0, 1, false);
    sample.alpha = 1.5;
    bad_alpha.transitions = vec![sample];
    assert!(replay(bad_alpha).unwrap_err().message.contains("alpha"));
    let mut outside = QTableRequest { state_amount: 2, action_amount: 2, initial_values: Vec::new(),
        transitions: Vec::new(), selector: None, selection_states: vec![5] };
    assert!(replay(outside.clone()).unwrap_err().message.contains("outside the table"));
    outside.selection_states = Vec::new();
    outside.selector = Some(QSelector { kind: "softmax".to_owned(), epsilon: None, seed: None });
    assert!(replay(outside).unwrap_err().message.contains("unknown selector"));
    let zero = QTableRequest { state_amount: 0, action_amount: 1, initial_values: Vec::new(),
        transitions: Vec::new(), selector: None, selection_states: Vec::new() };
    assert!(replay(zero).unwrap_err().message.contains("at least one state"));

    // 缺请求。 / A missing request.
    let mut missing = base_request();
    missing.model_kind = Some(ModelKind::QTableSimulate);
    assert_eq!(engine.submit(missing).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        let payload = serde_json::to_string(&QTableRequest { state_amount: 2, action_amount: 3,
            initial_values: vec![0.0, 3.0, 3.0, -1.0, 0.5, 0.5],
            transitions: vec![transition(0, 1, 1.0, 1, false)],
            selector: Some(QSelector { kind: "epsilonGreedy".to_owned(), epsilon: Some(0.0),
                seed: Some(7) }),
            selection_states: vec![0, 1] }).unwrap();
        std::fs::write(directory.join("q-table.json"), payload.as_bytes()).unwrap();
    }
}
#[test]
fn master_cut_encoding_verify_entry_binds_the_encodings_and_the_conflict() {
    use ospf_rust_bridge::{MasterAssignmentEntry, MasterCutEncodingReport, MasterCutEncodingRequest,
        MasterNoGoodRowSpec};

    let entry = |id: &str, value: i64| MasterAssignmentEntry { id: id.to_owned(), value };
    let request_of = |kind: &str| MasterCutEncodingRequest { kind: kind.to_owned(),
        assignment: Vec::new(), family_id: None, row: None, conflict_members: Vec::new(),
        conflict_proof_reference: None };

    let engine = NativeEngine::default();
    let verify = |document: MasterCutEncodingRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::MasterCutEncodingVerify);
        request.master_cut_encoding = Some(document);
        engine.submit(request)
    };

    // 线性：领域标签原样交出。 / Linear: the domain tag is handed back verbatim.
    let view = verify(request_of("linear")).unwrap();
    let finished = settle(&engine, &view);
    assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
    let report: MasterCutEncodingReport =
        serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
    assert_eq!(report.kind_vocabulary.len(), 3);
    assert_eq!(report.row_vocabulary.len(), 4);
    assert!(!report.multi_row);
    assert_eq!(report.canonical_json, "\"Linear\"");
    engine.release(&view.id).unwrap();

    // 多行 no-good：族与行分开，行变量在赋值里。 / Multi-row: family and row split; the row ID is assigned.
    let mut multi = request_of("boundedIntegerNoGood");
    multi.assignment = vec![entry("side:x", 1), entry("other", 0)];
    multi.family_id = Some("family-1".to_owned());
    multi.row = Some(MasterNoGoodRowSpec { kind: "atMostOne".to_owned(), id: Some("side:x".to_owned()) });
    let view = verify(multi.clone()).unwrap();
    let finished = settle(&engine, &view);
    let report: MasterCutEncodingReport =
        serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
    assert!(report.multi_row);
    assert_eq!(report.family_id.as_deref(), Some("family-1"));
    assert_eq!(report.row_kind.as_deref(), Some("atMostOne"));
    assert_eq!(report.row_id.as_deref(), Some("side:x"));
    assert_eq!(report.row_variable_is_assigned, Some(true));
    // 投影按稳定 ID 升序。 / The projection is sorted by stable ID.
    assert_eq!(report.assignment[0].id, "other");
    assert_eq!(report.assignment_count, 2);
    assert!(report.canonical_json.starts_with("{\"BoundedIntegerNoGood\""));
    engine.release(&view.id).unwrap();

    // 验证冲突：领域构造器给出拒绝。 / The verified conflict: the domain constructor supplies the refusal.
    let mut conflicted = multi.clone();
    conflicted.conflict_members = vec!["a".to_owned(), "b".to_owned()];
    conflicted.conflict_proof_reference = Some("proof-9".to_owned());
    let view = verify(conflicted.clone()).unwrap();
    let finished = settle(&engine, &view);
    let report: MasterCutEncodingReport =
        serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
    assert_eq!(report.conflict_member_count, Some(2));
    assert_eq!(report.conflict_proof_reference.as_deref(), Some("proof-9"));
    engine.release(&view.id).unwrap();
    let mut empty_members = conflicted.clone();
    empty_members.conflict_members = Vec::new();
    assert!(verify(empty_members).unwrap_err().message.contains("requires members"));
    let mut blank_proof = conflicted;
    blank_proof.conflict_proof_reference = Some("  ".to_owned());
    assert!(verify(blank_proof).is_err());

    // 两种编码混用。 / Mixed-up encodings.
    let mut linear = request_of("linear");
    linear.assignment = vec![entry("a", 1)];
    assert!(verify(linear).unwrap_err().message.contains("carries no excluded"));
    let mut binary = request_of("binaryNoGood");
    binary.assignment = vec![entry("a", 1), entry("a", 0)];
    assert!(verify(binary).unwrap_err().message.contains("repeats a stable ID"));
    let mut unknown = request_of("quadratic");
    unknown.assignment = vec![entry("a", 1)];
    assert!(verify(unknown).unwrap_err().message.contains("unknown encoding kind"));

    // 缺请求。 / A missing request.
    let mut missing = base_request();
    missing.model_kind = Some(ModelKind::MasterCutEncodingVerify);
    assert_eq!(engine.submit(missing).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        let payload = serde_json::to_string(&multi).unwrap();
        std::fs::write(directory.join("master-cut-encoding.json"), payload.as_bytes()).unwrap();
    }
}
#[test]
fn analysis_group_aggregate_entry_summarizes_across_stages() {
    use ospf_rust_bridge::{AnalysisGroupAggregateReport, AnalysisGroupAggregateRequest};
    use ospf_rust_core::analysis::{
        ActivityEvidence, ActivityGroupSummary, ActivityStatus, ActivitySummary, AnalysisStatus,
        CandidateTier, ConstraintActivity, ConstraintActivityReport, EffectivenessEntry,
        EffectivenessRanking,
    };
    use std::collections::BTreeMap;

    let record = |id: &str, group: &str, status: ActivityStatus| ConstraintActivity {
        constraint_id: id.into(), group: Some(group.to_owned()), status,
        lhs: Some(1.0), rhs: Some(1.0), slack: Some(0.0), normalized_slack: Some(0.0),
        tolerance: Some(1e-7), satisfied: Some(true),
        evidence: ActivityEvidence::LinearConstraint { lhs: 1.0, rhs: 1.0 } };
    let activity = |records: Vec<ConstraintActivity>| {
        let mut summary = ActivitySummary::default();
        let mut grouped = BTreeMap::<Option<String>, ActivitySummary>::new();
        for item in &records {
            summary.add(item.status);
            grouped.entry(item.group.clone()).or_default().add(item.status);
        }
        let groups = grouped.into_iter()
            .map(|(group, summary)| ActivityGroupSummary { group, summary }).collect();
        ConstraintActivityReport { schema_version: "1.0".to_owned(), constraints: records,
            variable_bounds: Vec::new(), summary, variable_bound_summary: ActivitySummary::default(),
            groups, tolerance: 1e-7, nearly_active_tolerance: 1e-5 }
    };
    let entry = |id: &str, group: &str, improvement: Option<f64>| EffectivenessEntry {
        constraint_id: id.into(), group: Some(group.to_owned()), tier: CandidateTier::TierA,
        activity: ActivityStatus::Active, dual_value: None, local_effective: None,
        removal_effective: None, first_effective_delta: None, max_improvement: improvement,
        marginal_effect: None, status: AnalysisStatus::Reachable, unavailable_reason: None };

    let engine = NativeEngine::default();
    let aggregate = |document: AnalysisGroupAggregateRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::AnalysisGroupAggregate);
        request.analysis_group_aggregate = Some(document);
        engine.submit(request)
    };
    let encode = |value: &str| serde_json::to_string(value).unwrap();

    // 活动性 + 有效性两份（冲突分支由桥接单测穷尽覆盖）。 / Activity plus effectiveness (bridge tests cover conflict).
    let activity_document = activity(vec![record("c1", "g1", ActivityStatus::Active),
        record("c2", "g2", ActivityStatus::Inactive)]);
    let ranking_document = EffectivenessRanking { schema_version: encode("1.0").trim_matches('"').to_owned(),
        baseline_objective: 10.0, entries: vec![entry("c1", "g1", Some(0.25)),
            entry("c2", "g2", Some(0.5))] };
    let view = aggregate(AnalysisGroupAggregateRequest {
        activity_json: Some(serde_json::to_string(&activity_document).unwrap()),
        effectiveness_json: Some(serde_json::to_string(&ranking_document).unwrap()),
        conflict_json: None }).unwrap();
    let finished = settle(&engine, &view);
    assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
    let report: AnalysisGroupAggregateReport =
        serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
    assert_eq!(report.sources, vec!["activity".to_owned(), "effectiveness".to_owned()]);
    assert_eq!(report.group_count, 2);
    // 两组的活跃数分别是 1 和 0 → `g1` 在前。 / The two groups differ on active count, so `g1` comes first.
    assert_eq!(report.groups[0].group.as_deref(), Some("g1"));
    assert_eq!(report.groups[0].active_instances, 1);
    assert_eq!(report.groups[0].max_improvement, Some(0.25));
    assert_eq!(report.groups[1].max_improvement, Some(0.5));
    assert!(!report.ungrouped_present);
    assert_eq!((report.total_involved, report.total_active, report.total_blocking), (2, 1, 0));
    assert!(report.ordering_rule.contains("blocking desc"));
    engine.release(&view.id).unwrap();

    // 三份都不给：合法，全零。 / No report at all: legal and all zero.
    let view = aggregate(AnalysisGroupAggregateRequest { activity_json: None,
        effectiveness_json: None, conflict_json: None }).unwrap();
    let finished = settle(&engine, &view);
    assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
    let report: AnalysisGroupAggregateReport =
        serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
    assert_eq!(report.group_count, 0);
    assert!(report.sources.is_empty());
    engine.release(&view.id).unwrap();

    // 坏报告：指明是哪一份。 / A broken report names which one failed.
    for (json, label) in [(String::new(), "activity"), ("not json".to_owned(), "activity"),
        ("{}".to_owned(), "effectiveness")]
    {
        let document = if label == "activity" {
            AnalysisGroupAggregateRequest { activity_json: Some(json), effectiveness_json: None,
                conflict_json: None }
        } else {
            AnalysisGroupAggregateRequest { activity_json: None, effectiveness_json: Some(json),
                conflict_json: None }
        };
        let error = aggregate(document).unwrap_err();
        assert!(error.message.contains(label), "{}: {}", label, error.message);
    }

    // 缺请求。 / A missing request.
    let mut missing = base_request();
    missing.model_kind = Some(ModelKind::AnalysisGroupAggregate);
    assert_eq!(engine.submit(missing).unwrap_err().code, "InvalidInput");
}
#[test]
fn predicate_field_resolution_entry_separates_trimming_from_identifier_validation() {
    use ospf_rust_bridge::{PredicateFieldMapping, PredicateFieldResolutionReport,
        PredicateFieldResolutionRequest};

    let mapping = |path: &str, backend: &str| PredicateFieldMapping {
        path: path.to_owned(), backend_field: backend.to_owned() };
    let engine = NativeEngine::default();
    let verify = |document: PredicateFieldResolutionRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::PredicateFieldResolutionVerify);
        request.predicate_field_resolution = Some(document);
        engine.submit(request)
    };
    let document = PredicateFieldResolutionRequest {
        schema_name: "  orders  ".to_owned(),
        mappings: vec![mapping("id", "orders.id"), mapping("status", "orders.status"),
            mapping("status", "legacy.status"), mapping("name", "orders.name"),
            mapping("name", "orders.name"), mapping("a b", "orders.a_b")],
        probes: vec!["id".to_owned(), "status".to_owned(), " nope ".to_owned(),
            "a b".to_owned(), "a..b".to_owned(), String::new(), "a.b.c".to_owned()],
    };
    let view = verify(document.clone()).unwrap();
    let finished = settle(&engine, &view);
    assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
    let report: PredicateFieldResolutionReport =
        serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
    assert_eq!(report.schema_name, "orders");
    assert_eq!(report.mapping_count, 6);
    assert_eq!(report.mappings.len(), 6);
    let probe = |path: &str| report.probes.iter().find(|item| item.path == path).unwrap().clone();
    assert_eq!(probe("id").resolution, "resolved");
    assert_eq!(probe("id").backend_field.as_deref(), Some("orders.id"));
    assert_eq!(probe("status").resolution, "ambiguous");
    assert_eq!(probe("status").candidates,
        vec!["legacy.status".to_owned(), "orders.status".to_owned()]);
    assert_eq!(probe(" nope ").trimmed, "nope");
    assert_eq!(probe(" nope ").resolution, "missing");
    // `a b` 被 `parse` 接受但不是合法标识符路径。 / `a b` is accepted by `parse` yet is not a valid identifier path.
    assert!(!probe("a b").valid_identifiers);
    assert_eq!(probe("a b").resolution, "resolved");
    assert!(!probe("a..b").valid_identifiers);
    assert_eq!(probe("a..b").depth, 3);
    assert!(probe("").segments.is_empty());
    assert_eq!(probe("").depth, 0);
    assert_eq!(probe("a.b.c").root.as_deref(), Some("a"));
    assert_eq!(probe("a.b.c").leaf.as_deref(), Some("c"));
    // 重复同字段不算歧义；只有 `status` 是真歧义。 / A repeated identical field is not ambiguous; only `status` is.
    assert_eq!(report.ambiguous_paths, vec!["status".to_owned()]);
    assert_eq!(report.reachable_resolutions.len(), 3);
    engine.release(&view.id).unwrap();

    // 空白 schema 名 / 空白后端字段：拒绝。 / A blank schema name or backend field is refused.
    let mut blank_schema = document.clone();
    blank_schema.schema_name = " ".to_owned();
    assert!(verify(blank_schema).unwrap_err().message.contains("schema name"));
    let mut blank_field = document.clone();
    blank_field.mappings = vec![mapping("id", "  ")];
    assert!(verify(blank_field).unwrap_err().message.contains("backend fields"));
    // 空映射 + 空探针：合法。 / An empty mapping and no probes: legal.
    let view = verify(PredicateFieldResolutionRequest { schema_name: "s".to_owned(),
        mappings: Vec::new(), probes: Vec::new() }).unwrap();
    let finished = settle(&engine, &view);
    assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
    engine.release(&view.id).unwrap();

    // 缺请求。 / A missing request.
    let mut missing = base_request();
    missing.model_kind = Some(ModelKind::PredicateFieldResolutionVerify);
    assert_eq!(engine.submit(missing).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        let payload = serde_json::to_string(&document).unwrap();
        std::fs::write(directory.join("predicate-field-resolution.json"), payload.as_bytes()).unwrap();
    }
}
#[test]
fn predicate_field_filter_extract_entry_shows_three_leniencies() {
    use ospf_rust_bridge::{FilterConditionSpec, FilterValueSpec, PredicateFieldFilterReport,
        PredicateFieldFilterRequest};

    let text = |kind: &str, value: &str| FilterValueSpec {
        kind: kind.to_owned(), text: value.to_owned() };
    let condition = |path: &str, operator: &str, value: Option<FilterValueSpec>| {
        FilterConditionSpec { path: path.to_owned(), operator: operator.to_owned(), value,
            values: Vec::new() } };
    let engine = NativeEngine::default();
    let extract = |document: PredicateFieldFilterRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::PredicateFieldFilterExtract);
        request.predicate_field_filter = Some(document);
        engine.submit(request)
    };
    let run = |document: PredicateFieldFilterRequest| {
        let view = extract(document).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: PredicateFieldFilterReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        engine.release(&view.id).unwrap();
        report
    };

    // 纯等值：三个提取器都成功。 / Pure equalities: all three extractors succeed.
    let report = run(PredicateFieldFilterRequest { combinator: "and".to_owned(),
        conditions: vec![condition("status", "eq", Some(text("string", "active"))),
            condition("tier", "eq", Some(text("number", "2")))] });
    assert!(report.rejected.is_empty());
    assert_eq!(report.eq_filters.as_ref().unwrap().len(), 2);
    assert_eq!(report.eq_or_in_filters.as_ref().unwrap().len(), 2);
    assert_eq!(report.field_filters.as_ref().unwrap().len(), 2);
    assert!(report.duplicate_paths.is_empty());
    assert_eq!(report.condition_count, 2);

    // 区间比较：前两个拒绝，字段级接受。 / An interval comparison: the first two refuse, the field level accepts.
    let report = run(PredicateFieldFilterRequest { combinator: "and".to_owned(),
        conditions: vec![condition("score", "gt", Some(text("number", "1.5")))] });
    assert_eq!(report.rejected, vec!["eqFilters".to_owned(), "eqOrInFilters".to_owned()]);
    let field = report.field_filters.as_ref().unwrap();
    assert_eq!(field[0].gt.as_ref().unwrap().text, "1.5");

    // `or`：三个全拒。 / `or`: all three refuse.
    let report = run(PredicateFieldFilterRequest { combinator: "or".to_owned(),
        conditions: vec![condition("status", "eq", Some(text("string", "active"))),
            condition("tier", "eq", Some(text("number", "2")))] });
    assert_eq!(report.rejected.len(), 3);
    assert_eq!(report.eq_filters, None);
    assert_eq!(report.field_filters, None);

    // 重复路径：静默覆盖 + `duplicatePaths` 标出。 / A repeated path: silently overwritten and surfaced.
    let duplicate = PredicateFieldFilterRequest { combinator: "and".to_owned(),
        conditions: vec![condition("status", "eq", Some(text("string", "active"))),
            condition("status", "eq", Some(text("string", "archived")))] };
    let report = run(duplicate.clone());
    assert_eq!(report.eq_filters.as_ref().unwrap().len(), 1);
    assert_eq!(report.eq_filters.as_ref().unwrap()[0].value.text, "archived");
    assert_eq!(report.duplicate_paths, vec!["status".to_owned()]);

    // 拒绝：空条件（领域会 panic）、未知操作符、缺值、缺请求。 / Refusals, including the empty list.
    assert!(extract(PredicateFieldFilterRequest { combinator: "and".to_owned(), conditions: Vec::new() })
        .unwrap_err().message.contains("at least one condition"));
    assert!(extract(PredicateFieldFilterRequest { combinator: "and".to_owned(),
        conditions: vec![condition("a", "like", Some(text("string", "x")))] })
        .unwrap_err().message.contains("unknown operator"));
    assert!(extract(PredicateFieldFilterRequest { combinator: "and".to_owned(),
        conditions: vec![condition("a", "eq", None)] })
        .unwrap_err().message.contains("needs a value"));
    assert!(extract(PredicateFieldFilterRequest { combinator: "and".to_owned(),
        conditions: vec![condition("a", "eq", Some(text("number", "one")))] })
        .unwrap_err().message.contains("parse as a float"));
    let mut missing = base_request();
    missing.model_kind = Some(ModelKind::PredicateFieldFilterExtract);
    assert_eq!(engine.submit(missing).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        let payload = serde_json::to_string(&duplicate).unwrap();
        std::fs::write(directory.join("predicate-field-filter.json"), payload.as_bytes()).unwrap();
    }
}
#[test]
fn model_configuration_entry_labels_what_the_domain_never_validates() {
    use ospf_rust_bridge::{BuildingStatusSpec, ModelConfigurationReport,
        ModelConfigurationRequest};

    let status = |stage: &str, ready: u32, total: u32| BuildingStatusSpec {
        model_name: "plan".to_owned(), stage: stage.to_owned(), ready, total };
    let engine = NativeEngine::default();
    let verify = |document: ModelConfigurationRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::ModelConfigurationVerify);
        request.model_configuration = Some(document);
        engine.submit(request)
    };
    let document = ModelConfigurationRequest {
        name: "  plan-1  ".to_owned(),
        enable_cache: None, lazy_evaluation: None, function_expansion_policy: None,
        multi_objective: None, max_sub_objectives: None,
        statuses: vec![status("registerTokens", 0, 0),
            status("registerLinearConstraints", 3, 10),
            status("flattenLinearModel", 5, 5),
            status("buildObjective", 9, 4)],
    };
    let view = verify(document.clone()).unwrap();
    let finished = settle(&engine, &view);
    assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
    let report: ModelConfigurationReport =
        serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
    // 模型名被裁空白；其余是领域默认值。 / The name is trimmed; the rest are the domain defaults.
    assert_eq!(report.name, "plan-1");
    assert!(report.enable_cache);
    assert!(report.lazy_evaluation);
    assert_eq!(report.function_expansion_policy, "eager");
    assert!(report.multi_objective);
    assert_eq!(report.max_sub_objectives, 100);
    assert_eq!(report.defaults.max_sub_objectives, 100);
    assert_eq!(report.stage_vocabulary.len(), 7);
    // 四种进度标签，含领域不校验的两种。 / Four progress labels, including the two the domain never validates.
    assert_eq!(report.statuses[0].progress_kind, "empty");
    assert_eq!(report.statuses[0].progress, None);
    assert!(report.statuses[0].total_is_zero);
    assert_eq!(report.statuses[1].progress_kind, "partial");
    assert_eq!(report.statuses[2].progress_kind, "complete");
    assert_eq!(report.statuses[3].progress_kind, "overflow");
    assert_eq!(report.statuses[3].progress, Some(2.25));
    engine.release(&view.id).unwrap();

    // `auto` 落在 deferred 一侧。 / `auto` sits on the deferred side.
    let mut auto = document.clone();
    auto.function_expansion_policy = Some("auto".to_owned());
    auto.enable_cache = Some(false);
    auto.max_sub_objectives = Some(7);
    auto.statuses = Vec::new();
    let view = verify(auto).unwrap();
    let finished = settle(&engine, &view);
    let report: ModelConfigurationReport =
        serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
    assert_eq!(report.function_expansion_policy, "auto");
    assert!(!report.policy_is_eager);
    assert!(report.policy_is_deferred);
    assert!(!report.enable_cache);
    assert_eq!(report.max_sub_objectives, 7);
    engine.release(&view.id).unwrap();

    // 拒绝：未知阶段 / 未知策略 / 空白模型名 / 缺请求。 / Refusals.
    let mut bad_stage = document.clone();
    bad_stage.statuses = vec![status("compile", 1, 2)];
    assert!(verify(bad_stage).unwrap_err().message.contains("unknown building stage"));
    let mut bad_policy = document.clone();
    bad_policy.function_expansion_policy = Some("lazy".to_owned());
    assert!(verify(bad_policy).unwrap_err().message.contains("unknown function expansion policy"));
    let mut blank = document.clone();
    blank.statuses = vec![BuildingStatusSpec { model_name: " ".to_owned(),
        stage: "registerTokens".to_owned(), ready: 1, total: 2 }];
    assert!(verify(blank).unwrap_err().message.contains("must not be blank"));
    let mut missing = base_request();
    missing.model_kind = Some(ModelKind::ModelConfigurationVerify);
    assert_eq!(engine.submit(missing).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        let payload = serde_json::to_string(&document).unwrap();
        std::fs::write(directory.join("model-configuration.json"), payload.as_bytes()).unwrap();
    }
}
#[test]
fn cache_key_identity_entry_shows_the_two_caches_differ() {
    use ospf_rust_bridge::{CacheKeyIdentityReport, CacheKeyIdentityRequest, CacheOperationSpec,
        RangeKeySpec, ValueKeySpec};

    let value_key = |kind: &str, object_id: u64| ValueKeySpec {
        kind: kind.to_owned(), object_id };
    let value_op = |name: &str, kind: &str, object_id: u64, value: Option<f64>| {
        CacheOperationSpec { operation: name.to_owned(), value_key: Some(value_key(kind, object_id)),
            range_key: None, value, range: false } };
    let range_op = |name: &str, object_id: u64, value: Option<f64>| CacheOperationSpec {
        operation: name.to_owned(), value_key: None,
        range_key: Some(RangeKeySpec { object_id }), value, range: true };

    let engine = NativeEngine::default();
    let verify = |document: CacheKeyIdentityRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::CacheKeyIdentityVerify);
        request.cache_key_identity = Some(document);
        engine.submit(request)
    };
    let document = CacheKeyIdentityRequest {
        value_keys: vec![value_key("polynomial", 7), value_key("intermediateSymbol", 7),
            value_key("objective", 7)],
        range_keys: vec![RangeKeySpec { object_id: 7 }, RangeKeySpec { object_id: 7 }],
        operations: vec![value_op("set", "polynomial", 1, Some(2.5)),
            value_op("getOrCompute", "polynomial", 1, Some(99.0)),
            value_op("clearObject", "polynomial", 1, None),
            range_op("set", 3, Some(1.5)), range_op("get", 3, None)],
    };
    let view = verify(document.clone()).unwrap();
    let finished = settle(&engine, &view);
    assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
    let report: CacheKeyIdentityReport =
        serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
    // 同一 ID 三种 kind 是三个键；范围键只有 ID，两次同 ID 是一个键。
    // One ID under three kinds is three keys; a range key is the ID alone, so two are one key.
    assert_eq!(report.value_distinct, 3);
    assert_eq!(report.range_distinct, 1);
    assert!(report.kinds_separate_value_keys);
    assert!(report.value_key_includes_kind);
    assert!(!report.range_key_includes_kind);
    assert!(report.same_id_collides_in_range_cache);
    assert_eq!(report.value_equal_pairs, vec![vec![false, false], vec![false], vec![]]);
    assert_eq!(report.range_equal_pairs, vec![vec![true], vec![]]);
    // 命中不计算、清到报 true、范围缓存同 ID 同槽。 / No compute on a hit, `true` on a hit, one range slot.
    assert_eq!(report.operations[1].result, Some(2.5));
    assert_eq!(report.operations[1].computed, Some(false));
    assert_eq!(report.operations[2].removed, Some(true));
    assert_eq!(report.operations[3].cache, "range");
    assert_eq!(report.operations[4].result, Some(1.5));
    engine.release(&view.id).unwrap();

    // 空请求：合法。 / A bare request is legal.
    let view = verify(CacheKeyIdentityRequest { value_keys: Vec::new(), range_keys: Vec::new(),
        operations: Vec::new() }).unwrap();
    let finished = settle(&engine, &view);
    assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
    engine.release(&view.id).unwrap();

    // 拒绝：未知 kind / 未知操作 / 缺键 / 缺值 / 缺请求。 / Refusals.
    let mut bad_kind = document.clone();
    bad_kind.value_keys = vec![value_key("range", 1)];
    assert!(verify(bad_kind).unwrap_err().message.contains("unknown cache kind"));
    let mut bad_op = document.clone();
    bad_op.operations = vec![CacheOperationSpec { operation: "peek".to_owned(),
        value_key: None, range_key: None, value: None, range: false }];
    assert!(verify(bad_op).unwrap_err().message.contains("needs a value key"));
    let mut no_value = document.clone();
    no_value.operations = vec![value_op("set", "polynomial", 1, None)];
    assert!(verify(no_value).unwrap_err().message.contains("needs a value"));
    let mut mismatched = document.clone();
    let mut spec = value_op("set", "polynomial", 1, Some(1.0));
    spec.range = true;
    mismatched.operations = vec![spec];
    assert!(verify(mismatched).unwrap_err().message.contains("needs a range key"));
    let mut missing = base_request();
    missing.model_kind = Some(ModelKind::CacheKeyIdentityVerify);
    assert_eq!(engine.submit(missing).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        let payload = serde_json::to_string(&document).unwrap();
        std::fs::write(directory.join("cache-key-identity.json"), payload.as_bytes()).unwrap();
    }
}
#[test]
fn cache_key_address_entry_shows_address_identity() {
    use ospf_rust_bridge::{CacheKeyAddressReport, CacheKeyAddressRequest, EqualValueGroup};

    let group = |value: i64, count: u32| EqualValueGroup { value, count };
    let engine = NativeEngine::default();
    let verify = |document: CacheKeyAddressRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::CacheKeyAddressVerify);
        request.cache_key_address = Some(document);
        engine.submit(request)
    };
    let document = CacheKeyAddressRequest {
        arc_groups: vec![group(42, 3)],
        box_groups: vec![group(7, 3)],
        raw_addresses: vec![0, 4096, 9_007_199_254_740_991],
    };
    let view = verify(document.clone()).unwrap();
    let finished = settle(&engine, &view);
    assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
    let report: CacheKeyAddressReport =
        serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
    // 同一个对象同键；值相等但对象不同 → 键不同。 / One object one key; equal values still differ.
    assert!(report.same_arc_same_key);
    assert!(report.same_box_same_key);
    assert!(report.distinct_arcs_distinct_keys);
    assert!(report.distinct_boxes_distinct_keys);
    assert!(report.address_identity);
    assert_eq!(report.arc_keys[0].len(), 3);
    assert_eq!(report.arc_keys[0].iter().collect::<std::collections::HashSet<_>>().len(), 3);
    assert_eq!(report.box_keys[0].iter().collect::<std::collections::HashSet<_>>().len(), 3);
    // 键是真实地址，不为 0。 / The keys are real addresses, never zero.
    assert!(report.arc_keys[0].iter().all(|key| *key != 0));
    // `from_raw` 原样保留，0 与 JS 安全整数上限都能表示。 / `from_raw` keeps everything, zero and the JS ceiling included.
    assert_eq!(report.raw_keys, vec![0, 4096, 9_007_199_254_740_991]);
    assert_eq!(report.raw_address_ceiling, 9_007_199_254_740_991);
    assert!(report.zero_address_is_representable);
    assert!(report.address_reuse_hazard);
    assert!(report.cacheable_requirement.contains("stable address"));
    engine.release(&view.id).unwrap();

    // 空请求合法。 / A bare request is legal.
    let view = verify(CacheKeyAddressRequest { arc_groups: Vec::new(), box_groups: Vec::new(),
        raw_addresses: Vec::new() }).unwrap();
    let finished = settle(&engine, &view);
    assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
    engine.release(&view.id).unwrap();

    // 拒绝：组内对象为 0 / 缺请求。 / Refusals: a zero object count and a missing request.
    let mut empty = document.clone();
    empty.arc_groups = vec![group(1, 0)];
    assert!(verify(empty).unwrap_err().message.contains("at least one object"));
    // 超过 JS 安全整数：入口就拒——这是实测打出来的边界，`u64::MAX` 无法经 JSON number 往返。
    // Above the JS safe integer it is refused at the entry: `u64::MAX` cannot round-trip through a JSON number.
    let mut overflow = document.clone();
    overflow.raw_addresses = vec![u64::MAX];
    assert!(verify(overflow).unwrap_err().message.contains("exceeds the JS safe integer ceiling"));
    let mut missing = base_request();
    missing.model_kind = Some(ModelKind::CacheKeyAddressVerify);
    assert_eq!(engine.submit(missing).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        let payload = serde_json::to_string(&document).unwrap();
        std::fs::write(directory.join("cache-key-address.json"), payload.as_bytes()).unwrap();
    }
}
#[test]
fn flatten_cache_entry_shows_the_three_paths() {
    use ospf_rust_bridge::{FlattenCacheOperation, FlattenCacheReport, FlattenCacheRequest,
        FlattenSymbolSpec};

    let symbol = |identifier: u64, value: f64| FlattenSymbolSpec { identifier, value,
        dependencies: Vec::new() };
    let op = |name: &str, symbol: Option<FlattenSymbolSpec>| FlattenCacheOperation {
        operation: name.to_owned(), symbol, identifiers: Vec::new() };
    let clearing = |name: &str, identifiers: &[u64]| FlattenCacheOperation {
        operation: name.to_owned(), symbol: None, identifiers: identifiers.to_vec() };
    let engine = NativeEngine::default();
    let verify = |document: FlattenCacheRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::FlattenCacheVerify);
        request.flatten_cache = Some(document);
        engine.submit(request)
    };
    let document = FlattenCacheRequest { operations: vec![
        op("flattenCached", Some(symbol(1, 2.0))),
        op("flattenCached", Some(symbol(1, 99.0))),
        op("flattenForce", Some(symbol(1, 3.0))),
        op("flattenCached", Some(symbol(1, 99.0))),
        op("cacheSize", None),
        clearing("clearPolynomial", &[1]),
        clearing("clearPolynomial", &[1]),
        op("isCacheEmpty", None),
    ] };
    let view = verify(document.clone()).unwrap();
    let finished = settle(&engine, &view);
    assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
    let report: FlattenCacheReport =
        serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
    // 首次平展 → 命中返回缓存 → 强制覆盖 → 再命中拿新值。 / First flatten, hit, force, hit the new value.
    assert_eq!(report.operations[0].value, Some(2.0));
    assert_eq!(report.operations[0].flattened, Some(true));
    assert_eq!(report.operations[1].value, Some(2.0));
    assert_eq!(report.operations[1].flattened, Some(false));
    assert_eq!(report.operations[2].value, Some(3.0));
    assert_eq!(report.operations[2].flattened, Some(true));
    assert_eq!(report.operations[3].value, Some(3.0));
    assert_eq!(report.operations[3].flattened, Some(false));
    assert_eq!(report.operations[4].cache_size, Some(1));
    // 清到 / 没清到。 / A hit and a miss of `clearPolynomial`.
    assert_eq!(report.operations[5].removed, Some(true));
    assert_eq!(report.operations[6].removed, Some(false));
    assert_eq!(report.operations[7].flag, Some(true));
    // 只平展了两次。 / Exactly two flattens.
    assert_eq!(report.flatten_calls, 2);
    assert_eq!(report.operation_vocabulary.len(), 11);
    engine.release(&view.id).unwrap();

    // 标识符相同的两个符号共用一槽：第二个拿到第一个的值。 / Two symbols sharing an identifier share one slot.
    let view = verify(FlattenCacheRequest { operations: vec![
        op("flattenCached", Some(symbol(7, 1.0))),
        op("flattenCached", Some(symbol(7, 2.0))),
    ] }).unwrap();
    let finished = settle(&engine, &view);
    let report: FlattenCacheReport =
        serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
    assert_eq!(report.operations[1].value, Some(1.0));
    assert_eq!(report.operations[1].flattened, Some(false));
    assert_eq!(report.flatten_calls, 1);
    engine.release(&view.id).unwrap();

    // 空请求：合法。 / A bare request is legal.
    let view = verify(FlattenCacheRequest { operations: Vec::new() }).unwrap();
    let finished = settle(&engine, &view);
    assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
    engine.release(&view.id).unwrap();

    // 拒绝：缺符号 / 未知操作 / 标识符个数不对 / 缺请求。 / Refusals.
    assert!(verify(FlattenCacheRequest { operations: vec![op("flattenForce", None)] })
        .unwrap_err().message.contains("needs a symbol"));
    let mut unknown = document.clone();
    unknown.operations = vec![{ let mut row = op("cacheSize", None);
        row.operation = "peek".to_owned(); row }];
    assert!(verify(unknown).unwrap_err().message.contains("unknown operation"));
    assert!(verify(FlattenCacheRequest { operations: vec![clearing("clearSymbol", &[1, 2])] })
        .unwrap_err().message.contains("exactly one identifier"));
    let mut missing = base_request();
    missing.model_kind = Some(ModelKind::FlattenCacheVerify);
    assert_eq!(engine.submit(missing).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        let payload = serde_json::to_string(&document).unwrap();
        std::fs::write(directory.join("flatten-cache.json"), payload.as_bytes()).unwrap();
    }
}
#[test]
fn update_assignment_entry_keeps_order_and_three_kinds() {
    use ospf_rust_bridge::{AssignmentExpressionSpec, AssignmentGroup, UpdateAssignmentReport,
        UpdateAssignmentRequest, UpdateAssignmentSpec, UpdateAssignmentValue};

    let set_value = |path: &str, kind: &str, text: &str| UpdateAssignmentSpec {
        path: path.to_owned(), kind: "setValue".to_owned(),
        value: Some(UpdateAssignmentValue { kind: kind.to_owned(), text: text.to_owned() }),
        expression: None };
    let set_null = |path: &str| UpdateAssignmentSpec { path: path.to_owned(),
        kind: "setNull".to_owned(), value: None, expression: None };
    let set_expression = |path: &str, expression: AssignmentExpressionSpec| UpdateAssignmentSpec {
        path: path.to_owned(), kind: "setExpression".to_owned(), value: None,
        expression: Some(expression) };
    let reference = |path: &str| AssignmentExpressionSpec { kind: "reference".to_owned(),
        path: Some(path.to_owned()), value: None, operands: Vec::new() };
    let constant = |text: &str| AssignmentExpressionSpec { kind: "constant".to_owned(), path: None,
        value: Some(UpdateAssignmentValue { kind: "number".to_owned(), text: text.to_owned() }),
        operands: Vec::new() };

    let engine = NativeEngine::default();
    let verify = |document: UpdateAssignmentRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::UpdateAssignmentVerify);
        request.update_assignment = Some(document);
        engine.submit(request)
    };
    let document = UpdateAssignmentRequest { groups: vec![
        AssignmentGroup { name: "first".to_owned(), assignments: vec![
            set_value("status", "string", "inactive"), set_null("deleted_at"),
            set_expression("score", AssignmentExpressionSpec { kind: "add".to_owned(), path: None,
                value: None, operands: vec![reference("score"), constant("1")] }) ] },
        AssignmentGroup { name: "second".to_owned(), assignments: vec![
            set_value("status", "string", "archived")] },
    ] };
    let view = verify(document.clone()).unwrap();
    let finished = settle(&engine, &view);
    assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
    let report: UpdateAssignmentReport =
        serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
    // 顺序即执行顺序，跨组也是。 / Order is execution order, across groups too.
    assert_eq!(report.paths, vec!["status".to_owned(), "deleted_at".to_owned(),
        "score".to_owned(), "status".to_owned()]);
    assert_eq!(report.items[3].group, "second");
    assert_eq!(report.group_sizes, vec![3, 1]);
    // 三型各自的名字与载荷。 / Each kind's name and payload.
    assert_eq!(report.items[0].kind, "setValue");
    assert_eq!(report.items[0].payload.as_deref(), Some("inactive"));
    assert_eq!(report.items[1].kind, "setNull");
    assert_eq!(report.items[1].payload, None);
    assert_eq!(report.items[2].kind, "setExpression");
    assert!(report.items[2].payload.as_deref().is_some_and(|text| text.contains("score")));
    // 重复路径只列出、不拒。 / A duplicate path is listed, not refused.
    assert_eq!(report.duplicate_paths, vec!["status".to_owned()]);
    assert_eq!(report.item_count, 4);
    engine.release(&view.id).unwrap();

    // `setNull` 与 `setValue(null)` 保持不同。 / `setNull` and `setValue(null)` stay different.
    let view = verify(UpdateAssignmentRequest { groups: vec![AssignmentGroup {
        name: "g".to_owned(), assignments: vec![set_null("deleted_at"),
            set_value("archived_at", "null", "")] }] }).unwrap();
    let finished = settle(&engine, &view);
    let report: UpdateAssignmentReport =
        serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
    assert_eq!(report.items[0].kind, "setNull");
    assert_eq!(report.items[1].kind, "setValue");
    assert_eq!(report.items[1].payload.as_deref(), Some("null"));
    engine.release(&view.id).unwrap();

    // 空组 / 空请求都是合法的。 / An empty group and a bare request are both legal.
    let view = verify(UpdateAssignmentRequest { groups: vec![AssignmentGroup {
        name: "empty".to_owned(), assignments: Vec::new() }] }).unwrap();
    let finished = settle(&engine, &view);
    let report: UpdateAssignmentReport =
        serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
    assert!(report.is_empty);
    assert_eq!(report.item_count, 0);
    engine.release(&view.id).unwrap();

    // 拒绝：空白路径 / 缺值 / 未知类型 / 坏字面值 / 缺请求。 / Refusals.
    assert!(verify(UpdateAssignmentRequest { groups: vec![AssignmentGroup { name: "g".to_owned(),
        assignments: vec![set_value("  ", "number", "1")] }] }).unwrap_err().message
        .contains("must not be blank"));
    assert!(verify(UpdateAssignmentRequest { groups: vec![AssignmentGroup { name: "g".to_owned(),
        assignments: vec![UpdateAssignmentSpec { path: "a".to_owned(), kind: "setValue".to_owned(),
            value: None, expression: None }] }] }).unwrap_err().message.contains("needs a value"));
    assert!(verify(UpdateAssignmentRequest { groups: vec![AssignmentGroup { name: "g".to_owned(),
        assignments: vec![UpdateAssignmentSpec { path: "a".to_owned(), kind: "setExpression".to_owned(),
            value: None, expression: None }] }] }).unwrap_err().message.contains("needs an expression"));
    assert!(verify(UpdateAssignmentRequest { groups: vec![AssignmentGroup { name: "g".to_owned(),
        assignments: vec![UpdateAssignmentSpec { path: "a".to_owned(), kind: "addOne".to_owned(),
            value: None, expression: None }] }] }).unwrap_err().message
        .contains("unknown assignment kind"));
    assert!(verify(UpdateAssignmentRequest { groups: vec![AssignmentGroup { name: "g".to_owned(),
        assignments: vec![set_value("a", "number", "one")] }] }).unwrap_err().message
        .contains("parse as a float"));
    let mut missing = base_request();
    missing.model_kind = Some(ModelKind::UpdateAssignmentVerify);
    assert_eq!(engine.submit(missing).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        let payload = serde_json::to_string(&document).unwrap();
        std::fs::write(directory.join("update-assignment.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn boolean_scope_entry_shows_identity_elements_and_corrections() {
    use ospf_rust_bridge::{BooleanScopeReport, BooleanScopeRequest, ScopeStep, ScopeValue};

    let number = |text: &str| ScopeValue { kind: "number".to_owned(), text: text.to_owned() };
    let with_value = |collector: &str, path: &str, text: &str| ScopeStep {
        collector: collector.to_owned(), path: path.to_owned(), value: Some(number(text)),
        values: Vec::new() };
    let engine = NativeEngine::default();
    let verify = |document: BooleanScopeRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::BooleanScopeVerify);
        request.boolean_scope = Some(document);
        engine.submit(request)
    };
    let run = |document: BooleanScopeRequest| {
        let view = verify(document).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: BooleanScopeReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        engine.release(&view.id).unwrap();
        report
    };
    let document = BooleanScopeRequest { build: "and".to_owned(), steps: vec![
        with_value("eq", "status", "1"), with_value("gt", "score", "2")] };

    // 两条 → `and(2)`。 / Two items → `and(2)`.
    let report = run(document.clone());
    assert_eq!(report.step_count, 2);
    assert_eq!(report.shape, "and");
    assert_eq!(report.operand_count, 2);
    assert!(!report.is_constant_true);
    assert!(!report.is_constant_false);
    // 单条不包一层。 / A single item is never wrapped.
    assert!(!report.single_is_wrapped);
    assert_eq!(report.steps[0].shape, "comparison");
    // 空作用域的恒等元：`and` 恒真、`or` 恒假。 / The identity elements: empty `and` true, empty `or` false.
    assert_eq!(report.empty_and_shape, "true");
    assert_eq!(report.empty_or_shape, "false");
    assert_eq!(report.collector_vocabulary.len(), 10);

    // 空作用域直接跑一遍：`and` 给恒真、`or` 给恒假。 / Run the empty scope: true under `and`, false under `or`.
    let view = verify(BooleanScopeRequest { build: "and".to_owned(), steps: Vec::new() }).unwrap();
    let finished = settle(&engine, &view);
    let report: BooleanScopeReport =
        serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
    assert_eq!(report.shape, "true");
    assert!(report.is_constant_true);
    engine.release(&view.id).unwrap();
    let view = verify(BooleanScopeRequest { build: "or".to_owned(), steps: Vec::new() }).unwrap();
    let finished = settle(&engine, &view);
    let report: BooleanScopeReport =
        serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
    assert_eq!(report.shape, "false");
    assert!(report.is_constant_false);
    engine.release(&view.id).unwrap();

    // `notIn` 是 `In { negated }` 而不是 `Not(In)`。 / `notIn` is `In { negated }`, not `Not(In)`.
    let not_in = ScopeStep { collector: "notIn".to_owned(), path: "region".to_owned(),
        value: None, values: vec![number("1")] };
    let in_list = ScopeStep { collector: "in".to_owned(), path: "region".to_owned(),
        value: None, values: vec![number("1"), number("2")] };
    let report = run(BooleanScopeRequest { build: "or".to_owned(),
        steps: vec![in_list.clone(), not_in.clone()] });
    assert_eq!(report.shape, "or");
    assert_eq!(report.steps[0].shape, "in");
    assert_eq!(report.steps[1].shape, "inNegated");
    assert_ne!(report.steps[0].shape, report.steps[1].shape);

    // 拒绝：未知收集器 / 未知方向 / 空白路径 / 缺值 / `in` 空列表 / 缺请求。 / Refusals.
    assert!(verify(BooleanScopeRequest { build: "and".to_owned(),
        steps: vec![ScopeStep { collector: "between".to_owned(), path: "a".to_owned(),
            value: None, values: Vec::new() }] }).unwrap_err().message
        .contains("unknown collector"));
    assert!(verify(BooleanScopeRequest { build: "xor".to_owned(), steps: Vec::new() })
        .unwrap_err().message.contains("unknown build direction"));
    assert!(verify(BooleanScopeRequest { build: "and".to_owned(),
        steps: vec![with_value("eq", "  ", "1")] }).unwrap_err().message
        .contains("need a field path"));
    assert!(verify(BooleanScopeRequest { build: "and".to_owned(),
        steps: vec![ScopeStep { collector: "eq".to_owned(), path: "a".to_owned(),
            value: None, values: Vec::new() }] }).unwrap_err().message.contains("needs a value"));
    let mut missing = base_request();
    missing.model_kind = Some(ModelKind::BooleanScopeVerify);
    assert_eq!(engine.submit(missing).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        let payload = serde_json::to_string(&document).unwrap();
        std::fs::write(directory.join("boolean-scope.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn unsupported_predicate_entry_shows_aliases_and_client_filter() {
    use ospf_rust_bridge::{AnnotationSpec, UnsupportedPredicateReport,
        UnsupportedPredicateRequest};

    let engine = NativeEngine::default();
    let verify = |document: UnsupportedPredicateRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::UnsupportedPredicateVerify);
        request.unsupported_predicate = Some(document);
        engine.submit(request)
    };
    let run = |document: UnsupportedPredicateRequest| {
        let view = verify(document).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: UnsupportedPredicateReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        engine.release(&view.id).unwrap();
        report
    };
    let document = UnsupportedPredicateRequest {
        policies: vec!["alwaysFalse".to_owned(), "failFast".to_owned(),
            "clientFilter".to_owned(), "ignore".to_owned(), "error".to_owned(),
            "default".to_owned()],
        annotations: vec![
            AnnotationSpec { key: "hint".to_owned(), value: "index-scan".to_owned() },
            AnnotationSpec { key: "ttl".to_owned(), value: "60".to_owned() },
            AnnotationSpec { key: "hint".to_owned(), value: "sequential".to_owned() },
        ],
    };

    // 默认是恒假而不是快失败。 / The default is always-false, not fail-fast.
    let report = run(document.clone());
    assert_eq!(report.default_policy, "alwaysFalse");
    assert_eq!(report.variant_vocabulary.len(), 3);
    assert_eq!(report.policies.len(), 6);
    let row = |name: &str| report.policies.iter().find(|row| row.name == name).unwrap();
    assert!(row("default").is_default);
    assert!(row("default").always_false);
    assert!(!row("default").fail_fast);
    // 别名归一，不增加变体。 / The aliases normalise and add no variants.
    assert_eq!(row("ignore").resolved, row("alwaysFalse").resolved);
    assert_eq!(row("error").resolved, row("failFast").resolved);
    assert_eq!(report.aliases.len(), 2);
    assert_eq!(report.aliases[0].resolves_to, "alwaysFalse");
    assert_eq!(report.aliases[1].resolves_to, "failFast");
    // `clientFilter` 两边都不做。 / `clientFilter` does neither.
    assert!(!row("clientFilter").fail_fast);
    assert!(!row("clientFilter").always_false);
    assert_eq!(report.neutral_policies, vec!["clientFilter".to_owned()]);
    // 注解原序保留、重复键列出。 / Annotations keep their order; duplicate keys are listed.
    assert_eq!(report.annotation_count, 3);
    assert_eq!(report.annotation_keys, vec!["hint".to_owned(), "ttl".to_owned(),
        "hint".to_owned()]);
    assert_eq!(report.duplicate_keys, vec!["hint".to_owned()]);

    // 空请求合法。 / A bare request is legal.
    let report = run(UnsupportedPredicateRequest { policies: Vec::new(), annotations: Vec::new() });
    assert!(report.policies.is_empty());
    assert_eq!(report.annotation_count, 0);
    assert_eq!(report.default_policy, "alwaysFalse");

    // 拒绝：未知策略 / 空白注解键 / 缺请求。 / Refusals.
    assert!(verify(UnsupportedPredicateRequest { policies: vec!["alwaysTrue".to_owned()],
        annotations: Vec::new() }).unwrap_err().message.contains("unknown policy"));
    assert!(verify(UnsupportedPredicateRequest { policies: Vec::new(),
        annotations: vec![AnnotationSpec { key: " ".to_owned(), value: "x".to_owned() }] })
        .unwrap_err().message.contains("must not be blank"));
    let mut missing = base_request();
    missing.model_kind = Some(ModelKind::UnsupportedPredicateVerify);
    assert_eq!(engine.submit(missing).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        let payload = serde_json::to_string(&document).unwrap();
        std::fs::write(directory.join("unsupported-predicate.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn repository_call_entry_shows_default_method_targets() {
    use ospf_rust_bridge::{RepositoryCallReport, RepositoryCallRequest};

    let engine = NativeEngine::default();
    let verify = |document: RepositoryCallRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::RepositoryCallVerify);
        request.repository_call = Some(document);
        engine.submit(request)
    };
    let run = |document: RepositoryCallRequest| {
        let view = verify(document).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: RepositoryCallReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        engine.release(&view.id).unwrap();
        report
    };
    let document = RepositoryCallRequest { count: 0,
        rows: vec!["a".to_owned(), "b".to_owned(), "c".to_owned()], limit: Some(1), fail: None,
        operations: vec!["find".to_owned(), "findWithOptions".to_owned(), "count".to_owned(),
            "exists".to_owned(), "update".to_owned(), "delete".to_owned()] };

    // `find` 走默认选项不截断；`findWithOptions` 带上限才截断。 / Only the capped call truncates.
    let report = run(document.clone());
    let row = |name: &str| report.calls.iter().find(|row| row.operation == name).unwrap();
    assert_eq!(row("find").row_count, Some(3));
    assert_eq!(row("findWithOptions").row_count, Some(1));
    assert!(!report.default_query_has_limit);
    assert_eq!(report.operation_vocabulary.len(), 6);
    // 自相矛盾：`count` 说 0、`find` 有三行 → `exists` 跟 `count` 走，判为不存在。 
    // Self-contradictory: count says 0 while find returns three rows, so `exists` follows `count`.
    assert_eq!(row("count").count, Some(0));
    assert_eq!(row("exists").flag, Some(false));
    assert!(report.exists_contradicts_find);
    // 空标记 trait 零方法可实现。 / The marker trait needs zero methods.
    assert!(report.marker_is_empty);
    assert_eq!(row("update").count, Some(0));
    assert_eq!(row("delete").count, Some(0));

    // 错误原样穿透。 / Errors pass through untouched.
    let report = run(RepositoryCallRequest { count: 1, rows: vec!["a".to_owned()], limit: None,
        fail: Some("backend is down".to_owned()),
        operations: vec!["find".to_owned(), "exists".to_owned()] });
    for name in ["find", "exists"] {
        let row = report.calls.iter().find(|row| row.operation == name).unwrap();
        assert!(row.errored, "{name}");
        assert_eq!(row.error.as_deref(), Some("backend is down"), "{name}");
    }

    // 空白操作列表合法。 / An empty operation list is legal.
    let report = run(RepositoryCallRequest { count: 0, rows: Vec::new(), limit: None, fail: None,
        operations: Vec::new() });
    assert!(report.calls.is_empty());

    // 拒绝：未知操作 / 缺请求。 / Refusals.
    assert!(verify(RepositoryCallRequest { count: 0, rows: Vec::new(), limit: None, fail: None,
        operations: vec!["peek".to_owned()] }).unwrap_err().message.contains("unknown operation"));
    let mut missing = base_request();
    missing.model_kind = Some(ModelKind::RepositoryCallVerify);
    assert_eq!(engine.submit(missing).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        let payload = serde_json::to_string(&document).unwrap();
        std::fs::write(directory.join("repository-calls.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn function_usage_entry_separates_consumed_from_referenceable() {
    use ospf_rust_bridge::{FunctionUsageReport, FunctionUsageRequest, UsageFlags};

    let flags = |objective: bool, constraint: bool, nested: bool, external: bool, fixed: bool| {
        UsageFlags { in_objective: objective, in_constraint: constraint,
            nested_as_input: nested, externally_referenced: external, result_is_fixed: fixed }
    };
    let engine = NativeEngine::default();
    let verify = |document: FunctionUsageRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::FunctionUsageVerify);
        request.function_usage = Some(document);
        engine.submit(request)
    };
    let run = |document: FunctionUsageRequest| {
        let view = verify(document).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: FunctionUsageReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        engine.release(&view.id).unwrap();
        report
    };
    let document = FunctionUsageRequest { cases: vec![
        flags(true, false, false, false, false),   // 只进目标 / objective only
        flags(false, true, false, false, false),   // 进约束 / in a constraint
        flags(false, false, true, false, false),   // 被嵌套 / nested
        flags(false, false, false, false, true),   // 结果被固定 / fixed
        flags(true, true, true, true, true),       // 全真 / everything
    ] };

    let report = run(document.clone());
    assert_eq!(report.case_count, 5);
    // 只进目标：被消费，但**不要求**可被引用；也不禁止原生写入。
    // Objective only: consumed but **not** required to be referenceable, and no write forbidden.
    assert!(report.cases[0].result_is_consumed);
    assert!(!report.cases[0].requires_referenceable_result);
    assert!(!report.cases[0].forbids_native_write);
    assert!(report.cases[0].helpers_are_exclusive);
    // 进约束 / 被嵌套 → 要求可被引用。 / In a constraint / nested → referenceable required.
    assert!(report.cases[1].requires_referenceable_result);
    assert!(report.cases[2].requires_referenceable_result);
    // 结果被固定 → 禁止原生写入，即使什么都没消费它。 / Fixed → forbidden, even with nothing consuming it.
    assert!(report.cases[3].forbids_native_write);
    assert!(!report.cases[3].result_is_consumed);
    assert!(!report.cases[3].requires_referenceable_result);
    // 全真：三个为真，辅助列不再独占。 / Everything true: three hold, helpers are no longer exclusive.
    assert!(report.cases[4].requires_referenceable_result);
    assert!(report.cases[4].forbids_native_write);
    assert!(!report.cases[4].helpers_are_exclusive);
    // 全格结论：32 种，四条恒等式成立，数字按实测。 / The grid: 32 combos, four identities, measured counts.
    assert_eq!(report.grid_size, 32);
    assert!(report.requires_implies_consumed);
    assert!(report.forbids_equals_fixed);
    assert!(report.exclusive_equals_not_referenced);
    assert_eq!(report.consumed_but_not_referenceable, 4);
    assert_eq!(report.referenceable_count, 24);
    assert_eq!(report.forbidden_count, 16);
    assert_eq!(report.exclusive_count, 16);
    assert_eq!(report.referenceable_and_forbidden, 12);

    // 空用例合法，仍然给全格结论。 / A bare request is legal and still reports the grid.
    let report = run(FunctionUsageRequest { cases: Vec::new() });
    assert_eq!(report.case_count, 0);
    assert!(report.cases.is_empty());
    assert_eq!(report.grid_size, 32);

    // 拒绝：用例超限 / 缺请求。 / Refusals.
    assert!(verify(FunctionUsageRequest {
        cases: vec![flags(false, false, false, false, false); 65] })
        .unwrap_err().message.contains("at most"));
    let mut missing = base_request();
    missing.model_kind = Some(ModelKind::FunctionUsageVerify);
    assert_eq!(engine.submit(missing).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        let payload = serde_json::to_string(&document).unwrap();
        std::fs::write(directory.join("function-usage.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn input_domain_entry_shows_interval_edges_and_fingerprint_edges() {
    use ospf_rust_bridge::{DomainPairSpec, DomainSpec, FloatSpec, InputDomainReport,
        InputDomainRequest};

    let number = |text: &str| FloatSpec { kind: "number".to_owned(), text: text.to_owned() };
    let special = |kind: &str| FloatSpec { kind: kind.to_owned(), text: String::new() };
    let domain = |lower: FloatSpec, upper: FloatSpec| DomainSpec { lower, upper };
    let engine = NativeEngine::default();
    let verify = |document: InputDomainRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::InputDomainVerify);
        request.input_domain = Some(document);
        engine.submit(request)
    };
    let run = |document: InputDomainRequest| {
        let view = verify(document).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: InputDomainReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        engine.release(&view.id).unwrap();
        report
    };
    let document = InputDomainRequest {
        domains: vec![
            domain(number("2"), number("2")),
            domain(number("5"), number("1")),
            domain(special("nan"), number("1")),
            domain(number("-3"), number("4")),
        ],
        pairs: vec![
            DomainPairSpec { left: domain(number("1"), number("2")),
                right: domain(number("2"), number("3")) },
            DomainPairSpec { left: domain(number("1"), number("2")),
                right: domain(number("3"), number("4")) },
            DomainPairSpec { left: domain(number("1"), number("5")),
                right: domain(number("2"), number("3")) },
        ],
        floats: vec![number("0"), number("-0"), number("1"), special("nan"), special("nan"),
            special("inf"), special("negInf")],
    };

    let report = run(document.clone());
    // 退化点是合法证明；两种非法输入各报各的。 / A degenerate point is legal; the two refusals differ.
    assert!(report.domains[0].ok);
    assert!(report.domains[0].degenerate);
    assert!(!report.domains[1].ok);
    assert!(report.domains[1].error.as_deref().unwrap().contains("lower <= upper"));
    assert!(!report.domains[2].ok);
    assert!(report.domains[2].error.as_deref().unwrap().contains("finite bounds"));
    assert!(report.domains[3].ok);
    // 失败的区间在 `from_optional_bounds` 里全是"没有证明"（错误被吞）。
    // Failed intervals have no optional proof — the error is swallowed.
    assert!(!report.domains[1].optional_proof);
    assert!(!report.domains[2].optional_proof);
    assert!(report.domains[3].optional_proof);
    assert!(report.optional_swallows_errors);
    // 端点相接 → 退化点，不是空；真正不相交才是空。 / Touching gives a point; disjoint is empty.
    assert!(report.pairs[0].has_intersection);
    assert!(report.pairs[0].intersection_is_point);
    assert_eq!(report.pairs[0].intersection_lower, Some(2.0));
    assert_eq!(report.pairs[0].intersection_upper, Some(2.0));
    assert!(!report.pairs[1].has_intersection);
    assert!(!report.pairs[1].intersection_is_point);
    assert!(report.pairs[2].right_in_left);
    for pair in &report.pairs {
        assert!(pair.left_reflexive);
    }
    // 指纹：`0`/`-0` 不同，两个 `NaN` 相同，三个标记互不相同。 / Fingerprint edges.
    assert_eq!(report.floats[0].fingerprint, "0.00000000000000000e0");
    assert_eq!(report.floats[1].fingerprint, "-0.00000000000000000e0");
    assert_ne!(report.floats[0].fingerprint, report.floats[1].fingerprint);
    assert!(report.zero_signs_differ);
    assert_eq!(report.floats[3].fingerprint, "nan");
    assert_eq!(report.floats[3].fingerprint, report.floats[4].fingerprint);
    assert!(report.nan_fingerprint_stable);
    assert_eq!(report.floats[5].fingerprint, "inf");
    assert_eq!(report.floats[6].fingerprint, "-inf");
    assert_ne!(report.floats[5].fingerprint, report.floats[6].fingerprint);
    assert!(report.markers_are_distinct);

    // 空请求合法，三处结论照样回报。 / A bare request is legal and still reports the findings.
    let report = run(InputDomainRequest { domains: Vec::new(), pairs: Vec::new(),
        floats: Vec::new() });
    assert!(report.domains.is_empty());
    assert!(report.pairs.is_empty());
    assert!(report.floats.is_empty());
    assert!(report.zero_signs_differ);
    assert!(report.optional_swallows_errors);

    // 拒绝：坏数字 / 未知浮点类型 / 区间对里非法 / 缺请求。 / Refusals.
    assert!(verify(InputDomainRequest { domains: vec![domain(number("one"), number("2"))],
        pairs: Vec::new(), floats: Vec::new() }).unwrap_err().message.contains("is not a float"));
    assert!(verify(InputDomainRequest { domains: vec![domain(special("zero"), number("2"))],
        pairs: Vec::new(), floats: Vec::new() }).unwrap_err().message
        .contains("unknown float kind"));
    assert!(verify(InputDomainRequest { domains: Vec::new(), pairs: vec![DomainPairSpec {
        left: domain(number("5"), number("1")), right: domain(number("1"), number("2")) }],
        floats: Vec::new() }).unwrap_err().message.contains("lower <= upper"));
    let mut missing = base_request();
    missing.model_kind = Some(ModelKind::InputDomainVerify);
    assert_eq!(engine.submit(missing).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        let payload = serde_json::to_string(&document).unwrap();
        std::fs::write(directory.join("input-domain.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn native_write_entry_checks_fingerprints_pairwise_and_proofs_one_way() {
    use ospf_rust_bridge::{BindingSpec, FingerprintCase, NativeWriteReport, NativeWriteRequest,
        ProofPair};

    let case = |recorded: Option<&str>, current: Option<&str>| FingerprintCase {
        recorded: recorded.map(str::to_owned), current: current.map(str::to_owned) };
    let engine = NativeEngine::default();
    let verify = |document: NativeWriteRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::NativeWriteVerify);
        request.native_write = Some(document);
        engine.submit(request)
    };
    let run = |document: NativeWriteRequest| {
        let view = verify(document).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: NativeWriteReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        engine.release(&view.id).unwrap();
        report
    };
    let document = NativeWriteRequest {
        cases: vec![case(Some("abc"), Some("abc")), case(Some("abc"), Some("def")),
            case(Some("abc"), None), case(None, Some("abc")), case(None, None)],
        bindings: vec![
            BindingSpec { source_symbol_id: 7, result_group: 1, result_index: 2,
                helpers: vec![vec![1, 2], vec![1, 3]] },
            BindingSpec { source_symbol_id: 9, result_group: 2, result_index: 0,
                helpers: vec![vec![2, 1], vec![2, 1]] },
        ],
        proofs: vec![
            ProofPair { proposed_lower: 2.0, proposed_upper: 3.0, recorded_lower: 1.0,
                recorded_upper: 5.0 },
            ProofPair { proposed_lower: 1.0, proposed_upper: 5.0, recorded_lower: 1.0,
                recorded_upper: 5.0 },
            ProofPair { proposed_lower: 0.0, proposed_upper: 9.0, recorded_lower: 1.0,
                recorded_upper: 5.0 },
        ],
    };

    let report = run(document.clone());
    // 只有"两侧都有且相等"通过。 / Only both-present-and-equal passes.
    assert!(report.cases[0].accepted);
    assert_eq!(report.cases[0].reason, "match");
    assert!(!report.cases[1].accepted);
    assert_eq!(report.cases[1].reason, "mismatch");
    assert!(!report.cases[2].accepted);
    assert_eq!(report.cases[2].reason, "structureOffersNone");
    assert!(!report.cases[3].accepted);
    assert_eq!(report.cases[3].reason, "recordLost");
    assert!(!report.cases[4].accepted);
    assert_eq!(report.cases[4].reason, "bothAbsent");
    // 缺一侧从不通过。 / Missing a side never passes.
    assert!(!report.both_absent_passes);
    assert!(!report.one_absent_passes);
    assert!(!report.bare_structure_has_fingerprint);
    assert!(!report.bare_structure_has_binding);
    // 绑定原样保留，且不替调用方拦结果列混入或重复。 / Bindings keep their fields without policing.
    assert_eq!(report.bindings[0].source_symbol_id, 7);
    assert!(report.bindings[0].result_also_helper);
    assert_eq!(report.bindings[0].helper_count, 2);
    assert_eq!(report.bindings[1].duplicate_helpers, 1);
    // 域证明只可收紧：收紧允许、持平允许、扩大拒绝。 / Tighten only.
    assert!(report.proofs[0].allowed);
    assert!(!report.proofs[0].unchanged);
    assert!(report.proofs[1].allowed);
    assert!(report.proofs[1].unchanged);
    assert!(!report.proofs[2].allowed);
    assert!(report.proofs[2].widened);
    assert_eq!(report.proofs[2].refusal.as_deref(), Some(report.widened_prefix.as_str()));
    assert!(report.widened_prefix.contains("may be tightened but not widened"));
    assert!(report.lost_prefix.contains("must not be dropped"));

    // 空请求合法，三处实测照样回报。 / A bare request is legal.
    let report = run(NativeWriteRequest { cases: Vec::new(), bindings: Vec::new(),
        proofs: Vec::new() });
    assert!(report.cases.is_empty());
    assert!(!report.both_absent_passes);
    assert!(!report.bare_structure_has_fingerprint);

    // 拒绝：辅助列不是对 / 区间非法 / 缺请求。 / Refusals.
    assert!(verify(NativeWriteRequest { cases: Vec::new(), bindings: vec![BindingSpec {
        source_symbol_id: 1, result_group: 0, result_index: 0,
        helpers: vec![vec![1, 2, 3]] }], proofs: Vec::new() }).unwrap_err().message
        .contains("[group, index]"));
    assert!(verify(NativeWriteRequest { cases: Vec::new(), bindings: Vec::new(),
        proofs: vec![ProofPair { proposed_lower: 5.0, proposed_upper: 1.0, recorded_lower: 1.0,
            recorded_upper: 5.0 }] }).unwrap_err().message.contains("lower <= upper"));
    let mut missing = base_request();
    missing.model_kind = Some(ModelKind::NativeWriteVerify);
    assert_eq!(engine.submit(missing).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        let payload = serde_json::to_string(&document).unwrap();
        std::fs::write(directory.join("native-write.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn elastic_model_entry_shows_slacks_direction_and_relaxation() {
    use ospf_rust_bridge::{ElasticModelReport, ElasticModelRequest, ModelRowSpec,
        ModelVariableSpec};

    let variable = |name: &str, group: u64, kind: &str, cost: f64| ModelVariableSpec {
        name: name.to_owned(), group, kind: kind.to_owned(), lower: 0.0, upper: 10.0, cost };
    let engine = NativeEngine::default();
    let verify = |document: ElasticModelRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::ElasticModelVerify);
        request.elastic_model = Some(document);
        engine.submit(request)
    };
    let run = |document: ElasticModelRequest| {
        let view = verify(document).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: ElasticModelReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        engine.release(&view.id).unwrap();
        report
    };
    let document = ElasticModelRequest { name: "m".to_owned(),
        variables: vec![variable("x", 0, "continuous", 1.0), variable("y", 1, "binary", 2.0),
            variable("z", 2, "integer", 3.0), variable("w", 3, "percentage", 4.0)],
        rows: vec![
            ModelRowSpec { coefficients: vec![vec![0.0, 1.0], vec![1.0, 1.0]], rhs: 4.0 },
            ModelRowSpec { coefficients: vec![vec![0.0, 2.0]], rhs: 3.0 },
        ],
        category: "max".to_owned(), penalty: None };

    let report = run(document.clone());
    assert_eq!(report.transforms.len(), 4);
    assert_eq!(report.transforms[0].variables, 4);
    assert_eq!(report.transforms[0].category, "max");
    // 每条约束一个松弛变量，方向被强制成 `min`。 / One slack per row; the direction becomes `min`.
    assert_eq!(report.transforms[1].variables, 6);
    assert_eq!(report.transforms[1].category, "min");
    assert_eq!(report.slacks_per_constraint, 1);
    assert_eq!(report.slacks.len(), 2);
    assert!(report.category_forced_to_minimum);
    for slack in &report.slacks {
        assert_eq!(slack.lower, 0.0);
        assert!(slack.upper_is_infinite);
        assert_eq!(slack.kind, "continuous");
        assert_eq!(slack.row_coefficient, Some(-1.0));
        // 默认罚项就是构建器的 1_000。 / The default penalty is the builder's 1_000.
        assert_eq!(slack.objective_coefficient, 1_000.0);
    }
    // 可行性模型：目标全零、方向 `min`。 / The feasibility model zeroes the objective.
    assert!(report.transforms[2].objective_is_zero);
    assert_eq!(report.transforms[2].category, "min");
    // 固定松弛映射。 / The fixed relaxation map.
    let map = |before: &str| report.relaxation_map.iter()
        .find(|pair| pair.before == before).map(|pair| pair.after.clone()).unwrap();
    assert_eq!(map("binary"), "percentage");
    assert_eq!(map("integer"), "continuous");
    assert_eq!(map("continuous"), "continuous");
    assert_eq!(map("percentage"), "percentage");
    // 负 / 零罚项都不拒。 / Negative and zero penalties are both accepted.
    assert!(!report.negative_penalty_refused);
    assert!(!report.zero_penalty_refused);

    // 显式罚项直接落进目标；负罚项也不拒。 / An explicit penalty lands in the objective; negatives included.
    let mut negative = document.clone();
    negative.penalty = Some(-1.0);
    let report = run(negative);
    assert_eq!(report.slacks[0].objective_coefficient, -1.0);
    assert!(!report.negative_penalty_refused);

    // 没有约束：不加松弛，但方向照样被强制。 / No rows: no slacks, but the direction is still forced.
    let mut empty = document.clone();
    empty.rows = Vec::new();
    let report = run(empty);
    assert!(report.slacks.is_empty());
    assert_eq!(report.slacks_per_constraint, 0);
    assert_eq!(report.transforms[1].variables, 4);
    assert!(report.category_forced_to_minimum);

    // 拒绝：未知方向 / 未知类型 / 界颠倒 / 行系数不是对 / 列越界 / 缺请求。 / Refusals.
    let mut bad = document.clone();
    bad.category = "both".to_owned();
    assert!(verify(bad).unwrap_err().message.contains("unknown objective direction"));
    let mut bad = document.clone();
    bad.variables[0].kind = "semi".to_owned();
    assert!(verify(bad).unwrap_err().message.contains("unknown variable kind"));
    let mut bad = document.clone();
    bad.variables[0].lower = 5.0;
    bad.variables[0].upper = 1.0;
    assert!(verify(bad).unwrap_err().message.contains("lower bounds must not exceed"));
    let mut bad = document.clone();
    bad.rows[0].coefficients = vec![vec![0.0, 1.0, 2.0]];
    assert!(verify(bad).unwrap_err().message.contains("coefficients are [column, value] pairs"));
    let mut bad = document.clone();
    bad.rows[0].coefficients = vec![vec![9.0, 1.0]];
    assert!(verify(bad).unwrap_err().message.contains("column that does not exist"));
    let mut missing = base_request();
    missing.model_kind = Some(ModelKind::ElasticModelVerify);
    assert_eq!(engine.submit(missing).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        let payload = serde_json::to_string(&document).unwrap();
        std::fs::write(directory.join("elastic-model.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn combine_status_entry_shows_lattice_dominance_and_folding() {
    use ospf_rust_bridge::{CombineStatusReport, CombineStatusRequest};

    let engine = NativeEngine::default();
    let verify = |document: CombineStatusRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::CombineStatusVerify);
        request.combine_status = Some(document);
        engine.submit(request)
    };
    let run = |document: CombineStatusRequest| {
        let view = verify(document).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: CombineStatusReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        engine.release(&view.id).unwrap();
        report
    };
    let document = CombineStatusRequest { sequence: vec!["reachable".to_owned(),
        "unknown".to_owned(), "unreachable".to_owned()] };

    let report = run(document.clone());
    // 全表 20 格，词表按 rank 降序。 / Twenty cells; the vocabulary is ordered by rank descending.
    assert_eq!(report.table.len(), 20);
    assert_eq!(report.vocabulary.len(), 4);
    assert_eq!(report.vocabulary[0], "reachable");
    assert_eq!(report.vocabulary[3], "unsupported");
    // 四条代数性质都成立。 / All four algebraic properties hold.
    assert!(report.none_is_identity);
    assert!(report.commutative);
    assert!(report.associative);
    assert!(report.idempotent);
    // `Unsupported` 压住一切；`Reachable` 最弱；`Unknown` **不**压住 `Unsupported`。
    // `Unsupported` dominates, `Reachable` is the weakest, `Unknown` does **not** beat `Unsupported`.
    assert!(report.unsupported_dominates);
    assert!(report.reachable_is_weakest);
    assert!(!report.unknown_beats_unsupported);
    let cell = |current: &str, next: &str| report.table.iter()
        .find(|cell| cell.current == current && cell.next == next)
        .unwrap().combined.clone();
    assert_eq!(cell("unknown", "unsupported"), "unsupported");
    assert_eq!(cell("unsupported", "unknown"), "unsupported");
    assert_eq!(cell("reachable", "unreachable"), "unreachable");
    assert_eq!(cell("unreachable", "unknown"), "unknown");
    assert_eq!(cell("unknown", "none"), "unknown");
    // 折叠：三个状态收敛到 rank 最小的那个。 / The fold converges on the lowest rank.
    assert_eq!(report.folded, "unknown");
    assert_eq!(report.trace, vec!["reachable".to_owned(), "unknown".to_owned(),
        "unknown".to_owned()]);

    // 顺序无关。 / Order independent.
    let reversed = run(CombineStatusRequest { sequence: vec!["unreachable".to_owned(),
        "unknown".to_owned(), "reachable".to_owned()] });
    assert_eq!(reversed.folded, report.folded);
    // 一旦出现 `Unsupported` 就翻不了身。 / An early `Unsupported` is final.
    let poisoned = run(CombineStatusRequest { sequence: vec!["reachable".to_owned(),
        "unsupported".to_owned(), "reachable".to_owned()] });
    assert_eq!(poisoned.folded, "unsupported");
    assert_eq!(poisoned.trace[2], "unsupported");
    // 空序列：回报 `unknown`，没有步骤。 / An empty sequence reports `unknown` with no steps.
    let empty = run(CombineStatusRequest { sequence: Vec::new() });
    assert_eq!(empty.folded, "unknown");
    assert!(empty.trace.is_empty());
    assert_eq!(empty.table.len(), 20);

    // 拒绝：未知状态 / 序列超限 / 缺请求。 / Refusals.
    assert!(verify(CombineStatusRequest { sequence: vec!["unknownish".to_owned()] })
        .unwrap_err().message.contains("unknown status"));
    let mut missing = base_request();
    missing.model_kind = Some(ModelKind::CombineStatusVerify);
    assert_eq!(engine.submit(missing).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        let payload = serde_json::to_string(&document).unwrap();
        std::fs::write(directory.join("combine-status.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn number_text_entry_shows_kotlin_shape_and_one_way_non_finite() {
    use ospf_rust_bridge::{NumberTextReport, NumberTextRequest, TextFloatSpec};

    let number = |text: &str| TextFloatSpec { kind: "number".to_owned(),
        text: text.to_owned() };
    let special = |kind: &str| TextFloatSpec { kind: kind.to_owned(), text: String::new() };
    let engine = NativeEngine::default();
    let verify = |document: NumberTextRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::NumberTextVerify);
        request.number_text = Some(document);
        engine.submit(request)
    };
    let run = |document: NumberTextRequest| {
        let view = verify(document).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: NumberTextReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        engine.release(&view.id).unwrap();
        report
    };
    let document = NumberTextRequest {
        floats: vec![number("0"), number("-0"), number("1"), number("0.1"), number("1e7"),
            special("nan"), special("inf")],
        vector: vec![number("1.5"), number("-2")],
        matrix: vec![vec![number("1"), number("2")], vec![number("3")]],
        raw_matrix: vec![vec![number("1.5"), number("2")]],
    };

    let report = run(document.clone());
    // 边界：门槛左闭右开、整数值补 `.0`、`-0.0` 保留符号。 
    // Boundaries: half-open threshold, integral gets `.0`, `-0.0` keeps its sign.
    assert_eq!(report.integral, "1.0");
    assert_eq!(report.boundary_lower, "0.001");
    assert_eq!(report.boundary_upper, "1.0E7");
    assert_eq!(report.negative_zero, "-0.0");
    // 有限值往返。 / Finite values round-trip.
    for row in &report.floats[..5] {
        assert!(row.reparsed, "{}", row.input);
        assert!(row.round_trips, "{}", row.input);
        assert!(row.codec_encodes, "{}", row.input);
    }
    assert_eq!(report.floats[4].formatted, "1.0E7");
    // 非有限值：能格式化成文本，但**读不回来**，编解码器也不收。 
    // Non-finite values format into text but cannot be read back, and the codec refuses them.
    assert_eq!(report.nan_text, "NaN");
    assert!(!report.nan_round_trips);
    for row in &report.floats[5..] {
        assert!(!row.reparsed, "{}", row.input);
        assert!(!row.round_trips, "{}", row.input);
        assert!(!row.codec_encodes, "{}", row.input);
    }
    assert_eq!(report.floats[6].formatted, "inf");
    // 向量 / 矩阵走字符串编解码，往返成功。 / Vector and matrix ride the string codecs.
    assert!(report.vector_round_trips);
    assert!(report.matrix_round_trips);
    assert!(report.vector_json.contains("\"1.5\""));
    // 原生矩阵出 **JSON 数字**，解码器却同时接受字符串。 
    // The raw matrix emits **JSON numbers** while its decoder also accepts strings.
    assert_eq!(report.raw_json, "[[1.5,2.0]]");
    assert!(report.raw_round_trips);
    assert!(report.raw_accepts_legacy_strings);
    // `json::numbers` 把非有限值变成 0。 / `json::numbers` degrades non-finite to zero.
    assert!(report.numbers_degrades_non_finite);

    // 空请求合法，结论照样回报。 / A bare request is legal and still reports the findings.
    let report = run(NumberTextRequest { floats: Vec::new(), vector: Vec::new(),
        matrix: Vec::new(), raw_matrix: Vec::new() });
    assert!(report.floats.is_empty());
    assert_eq!(report.vector_json, "[]");
    assert_eq!(report.integral, "1.0");
    assert!(report.raw_accepts_legacy_strings);
    assert!(report.numbers_degrades_non_finite);

    // 拒绝：坏数字 / 未知类型 / 向量里有非有限值 / 原生矩阵里有非有限值 / 缺请求。 / Refusals.
    assert!(verify(NumberTextRequest { floats: vec![number("one")], vector: Vec::new(),
        matrix: Vec::new(), raw_matrix: Vec::new() }).unwrap_err().message.contains("is not a float"));
    assert!(verify(NumberTextRequest { floats: vec![special("zero")], vector: Vec::new(),
        matrix: Vec::new(), raw_matrix: Vec::new() }).unwrap_err().message
        .contains("unknown float kind"));
    assert!(verify(NumberTextRequest { floats: Vec::new(), vector: vec![special("nan")],
        matrix: Vec::new(), raw_matrix: Vec::new() }).unwrap_err().message.contains("non-finite"));
    assert!(verify(NumberTextRequest { floats: Vec::new(), vector: Vec::new(),
        matrix: Vec::new(), raw_matrix: vec![vec![special("inf")]] }).unwrap_err().message
        .contains("non-finite"));
    let mut missing = base_request();
    missing.model_kind = Some(ModelKind::NumberTextVerify);
    assert_eq!(engine.submit(missing).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        let payload = serde_json::to_string(&document).unwrap();
        std::fs::write(directory.join("number-text.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn big_m_entry_shows_clamping_signs_and_row_orders() {
    use ospf_rust_bridge::{BigMPolicyReport, BigMPolicyRequest, BigMPolynomialSpec};

    let engine = NativeEngine::default();
    let verify = |document: BigMPolicyRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::BigMPolicyVerify);
        request.big_m_policy = Some(document);
        engine.submit(request)
    };
    let run = |document: BigMPolicyRequest| {
        let view = verify(document).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: BigMPolicyReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        engine.release(&view.id).unwrap();
        report
    };
    let document = BigMPolicyRequest {
        probes: vec!["0".to_owned(), "0.25".to_owned(), "3".to_owned(), "NaN".to_owned()],
        polynomial: BigMPolynomialSpec { terms: vec![vec![0.0, 2.0], vec![1.0, -3.0]],
            constant: 5.0 },
        indicator_index: 2,
        big_m: 10.0,
        name_prefix: "logic".to_owned(),
    };

    let report = run(document.clone());
    // 常量与两段式校验。 / The constants and the two stages.
    assert_eq!(report.default_big_m, 1_000_000.0);
    assert_eq!(report.min_big_m, 1.0);
    assert!(!report.probes[0].accepted);
    assert!(report.zero_is_refused);
    assert!(report.probes[1].accepted);
    assert!(report.probes[1].clamped);
    assert_eq!(report.probes[1].value, Some(1.0));
    assert_eq!(report.probes[2].value, Some(3.0));
    assert!(!report.probes[3].accepted);
    // 三个构造器各 2 行，顺序不一致。 / Two rows each, with different orders.
    assert!(report.two_rows_each);
    assert_eq!(report.rows.len(), 6);
    assert!(report.orders_differ);
    let row = |builder: &str, name: &str| report.rows.iter()
        .find(|row| row.builder == builder && row.name == name)
        .unwrap_or_else(|| panic!("{builder}/{name}"));
    assert_eq!(row("positive", "logic_ub").comparison, "LessEqual");
    assert_eq!(row("positive", "logic_lb").comparison, "GreaterEqual");
    assert_eq!(row("negative", "logic_lb").comparison, "GreaterEqual");
    assert_eq!(row("negative", "logic_ub").comparison, "LessEqual");
    // 指示系数：两个 `-M`，一个 `+M`。 / Two use `-M`, `negative` uses `+M`.
    assert_eq!(row("positive", "logic_ub").indicator_coefficient, -10.0);
    assert_eq!(row("nonnegative", "logic_lb").indicator_coefficient, -10.0);
    assert_eq!(row("negative", "logic_ub").indicator_coefficient, 10.0);
    assert!(report.negative_flips_indicator_sign);
    // 严格性 = 机器 epsilon；常数留在左端。 / Strictness is the machine epsilon; the constant stays left.
    assert_eq!(row("positive", "logic_lb").rhs, f64::EPSILON - 10.0);
    assert_eq!(row("negative", "logic_ub").rhs, 10.0 - f64::EPSILON);
    assert_eq!(row("nonnegative", "logic_lb").rhs, -10.0);
    assert!(report.strictness_is_machine_epsilon);
    for row in &report.rows {
        assert_eq!(row.lhs_constant, 5.0);
        assert_eq!(row.terms, vec![vec![0.0, 2.0], vec![1.0, -3.0]]);
    }

    // 小于 1 的 M 被夹紧，行里也出现夹紧值。 / A sub-unit M is clamped, in the rows too.
    let mut clamped = document.clone();
    clamped.big_m = 0.5;
    let report = run(clamped);
    assert!(report.small_positive_is_clamped);
    assert_eq!(report.rows[0].indicator_coefficient, -1.0);

    // 拒绝：坏探测 / 非正 M / 空前缀 / 项不是对 / 列不是整数 / 常数非有限 / 缺请求。
    // Refusals.
    let mut bad = document.clone();
    bad.probes = vec!["one".to_owned()];
    assert!(verify(bad).unwrap_err().message.contains("is not a float"));
    let mut bad = document.clone();
    bad.big_m = 0.0;
    assert!(verify(bad).unwrap_err().message.contains("finite and positive"));
    let mut bad = document.clone();
    bad.name_prefix = "   ".to_owned();
    assert!(verify(bad).unwrap_err().message.contains("prefix"));
    let mut bad = document.clone();
    bad.polynomial.terms = vec![vec![0.0]];
    assert!(verify(bad).unwrap_err().message.contains("pairs"));
    let mut bad = document.clone();
    bad.polynomial.terms = vec![vec![1.5, 1.0]];
    assert!(verify(bad).unwrap_err().message.contains("non-negative integer"));
    let mut missing = base_request();
    missing.model_kind = Some(ModelKind::BigMPolicyVerify);
    assert_eq!(engine.submit(missing).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        let payload = serde_json::to_string(&document).unwrap();
        std::fs::write(directory.join("big-m-policy.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn conditional_semantics_entry_shows_three_valued_rules_and_the_type_split() {
    use ospf_rust_bridge::{ConditionBoundsSpec, ConditionPolynomialSpec,
        ConditionalSemanticsReport, ConditionalSemanticsRequest};

    let engine = NativeEngine::default();
    let verify = |document: ConditionalSemanticsRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::ConditionalSemanticsVerify);
        request.conditional_semantics = Some(document);
        engine.submit(request)
    };
    let run = |document: ConditionalSemanticsRequest| {
        let view = verify(document).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: ConditionalSemanticsReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        engine.release(&view.id).unwrap();
        report
    };
    let document = ConditionalSemanticsRequest {
        condition: ConditionPolynomialSpec { terms: vec![vec![0.0, 1.0]], constant: 0.0 },
        bounds: ConditionBoundsSpec { lower: -5.0, upper: 5.0 },
        strict_boundary: 1.0,
        name: Some("rule".to_owned()),
        probes: vec![-2.0, -0.5, 0.0, 0.5, 1.0, 2.0],
        bound_probes: vec![
            ConditionBoundsSpec { lower: -5.0, upper: 5.0 },
            ConditionBoundsSpec { lower: 0.0, upper: 2.0 },
            ConditionBoundsSpec { lower: 0.1, upper: 0.9 },
        ],
    };

    let report = run(document.clone());
    assert_eq!(report.strict_boundary, 1.0);
    // 不可判定窗口随关系翻转。 / The undecidable window flips with the relation.
    assert!(report.window_flips_with_relation);
    assert_eq!(report.relations.len(), 4);
    let relation = |name: &str| report.relations.iter().find(|row| row.relation == name).unwrap();
    assert_eq!(relation("greater").undefined_lower, 0.0);
    assert_eq!(relation("greater").undefined_upper, 1.0);
    assert_eq!(relation("greaterEqual").undefined_lower, -1.0);
    assert_eq!(relation("greaterEqual").undefined_upper, 0.0);
    let truth = |name: &str, difference: f64| relation(name).classifications.iter()
        .find(|cell| cell.difference == difference).unwrap().truth.clone();
    // 非严格关系照样有死区。 / A non-strict relation still has a dead band.
    assert_eq!(truth("greater", 1.0), "true");
    assert_eq!(truth("greater", 0.0), "false");
    assert_eq!(truth("greater", 0.5), "undefined");
    assert_eq!(truth("greaterEqual", 0.0), "true");
    assert_eq!(truth("greaterEqual", -0.5), "undefined");
    assert_eq!(truth("less", -0.5), "undefined");
    // 不可判定 ⇒ 没有二值结果。 / Undecidable means no binary result.
    assert!(report.undefined_has_no_binary);
    let cell = relation("greater").classifications.iter()
        .find(|cell| cell.difference == 0.5).unwrap();
    assert_eq!(cell.binary, None);
    // 界：贴端点接受、完全在窗口内被拒。 / Bounds: touching accepted, interior refused.
    assert!(report.touching_endpoint_is_accepted);
    assert!(report.interior_is_refused);
    let probe = |name: &str, lower: f64, upper: f64| report.bounds_probes.iter()
        .find(|row| row.relation == name && row.lower == lower && row.upper == upper).unwrap();
    assert!(probe("greater", 0.0, 2.0).accepted);
    assert!(!probe("greater", 0.1, 0.9).accepted);
    assert!(probe("greaterEqual", 0.1, 0.9).accepted);
    // 两条入口类型不同。 / The two entries return different types.
    assert_eq!(report.entries.len(), 6);
    assert!(report.legacy_threshold_is_zero);
    assert!(report.aliases_are_not_shared);
    let entry = |name: &str| report.entries.iter().find(|row| row.entry == name).unwrap();
    assert_eq!(entry("if_named").kind, "conditionalIndicator");
    assert_eq!(entry("if_named").result_variable, "rule_if");
    assert_eq!(entry("if_named").indicator_variable.as_deref(), Some("rule_if_nz"));
    // `if_` 用自动名，不接调用方的名字。 / `if_` auto-names instead of using the caller's name.
    assert!(entry("if_").result_variable.starts_with("conditional_indicator_"));
    assert_eq!(entry("if_legacy").kind, "thresholdBinaryzation");
    assert_eq!(entry("if_legacy").threshold, Some(0.0));
    assert_eq!(entry("legacy_if_named").threshold, Some(0.0));
    for row in &report.aliases {
        assert!(row.same_shape);
        assert!(!row.same_identity);
    }

    // 拒绝：非正严格边界 / NaN 边界 / 项不是对 / 常数非有限 / 探测非有限 / 缺请求。 / Refusals.
    let mut bad = document.clone();
    bad.strict_boundary = 0.0;
    assert!(verify(bad).unwrap_err().message.contains("finite and positive"));
    let mut bad = document.clone();
    bad.strict_boundary = f64::NAN;
    assert!(verify(bad).unwrap_err().message.contains("strict boundary"));
    let mut bad = document.clone();
    bad.condition.terms = vec![vec![0.0]];
    assert!(verify(bad).unwrap_err().message.contains("pairs"));
    let mut bad = document.clone();
    bad.condition.constant = f64::INFINITY;
    assert!(verify(bad).unwrap_err().message.contains("constant"));
    let mut bad = document.clone();
    bad.probes = vec![f64::INFINITY];
    assert!(verify(bad).unwrap_err().message.contains("must be finite"));
    let mut missing = base_request();
    missing.model_kind = Some(ModelKind::ConditionalSemanticsVerify);
    assert_eq!(engine.submit(missing).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        let payload = serde_json::to_string(&document).unwrap();
        std::fs::write(directory.join("conditional-semantics.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn search_budget_entry_shows_nested_accounting_and_resumable_random() {
    use ospf_rust_bridge::{RandomVerdict, SearchBudgetReport, SearchBudgetRequest};

    let engine = NativeEngine::default();
    let verify = |document: SearchBudgetRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::SearchBudgetVerify);
        request.search_budget = Some(document);
        engine.submit(request)
    };
    let run = |document: SearchBudgetRequest| {
        let view = verify(document).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: SearchBudgetReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        engine.release(&view.id).unwrap();
        report
    };
    let document = SearchBudgetRequest { limit: 10, used: 1, objective_calls: 3, evaluations: 6,
        steps: 1, grandchild_evaluations: 2, claims: 4, grandchild_claims: 2, random_state: 7,
        random_draws: 4, split_id: "seed".to_owned() };

    let report = run(document.clone());
    // 全局：起始已用 1、子片段获批 4（本地额度）、孙片段再 2 次（本地额度 2）。 
    // Global: starting 1, the child grants 4 (its allowance), the grandchild 2 more (its allowance).
    assert_eq!(report.used_before, 1);
    assert_eq!(report.child.granted, 4);
    assert_eq!(report.child.evaluations, 6);
    assert!(!report.child.denied);
    assert_eq!(report.grandchild.granted, 2);
    assert_eq!(report.grandchild.used, 2);
    // 孙片段的领取同时计入子片段本地额度与全局：子片段 4 + 2 = 6、全局 1 + 6 = 7。 
    // Grandchild claims count against the child's local allowance and the global: 4 + 2 = 6, 1 + 6 = 7.
    assert_eq!(report.child.used, 6);
    assert_eq!(report.used_after, 7);
    assert_eq!(report.remaining_after, 3);
    // `steps` 不参与准入。 / `steps` does not gate admission.
    assert!(report.steps_do_not_gate_admission);
    // 孙片段的消耗计入子片段与全局。 / Grandchild claims charge both.
    assert!(report.child_charges_parent);
    // 其余三条发现都由内部探测验证。 / The other three findings come from internal probes.
    assert!(report.global_stops_at_limit);
    assert!(report.parent_refusal_rolls_back);
    assert!(report.denied_is_sticky);
    assert!(report.remaining_is_minimum);
    // 目标调用次数不随领取变化。 / The objective call count is untouched by claims.
    assert_eq!(report.objective_calls, 3);
    // 随机流：可恢复、`split` 不消费父流、同种子同序列。 
    // The random stream: resumable, non-consuming `split`, identical seeds agree.
    let random: &RandomVerdict = &report.random;
    assert_eq!(random.draws.len(), 4);
    assert!(random.draws_in_unit_interval);
    assert!(random.identical_seeds_agree);
    assert!(random.resume_matches);
    assert_eq!(random.resumed_draw, random.draws[3]);
    assert!(!random.split_moves_parent);
    assert_ne!(random.state_before, random.split_state);

    // 父片段本地额度被自身领满后，孙片段即使全局还有余量也被拒 —— 配额守恒。 
    // Once the parent's own local quota is used up, the grandchild is refused even with global room: quota is conserved.
    let mut blocked = document.clone();
    blocked.evaluations = 4;
    blocked.grandchild_evaluations = 5;
    blocked.grandchild_claims = 3;
    let report = run(blocked);
    assert_eq!(report.child.used, 4);
    assert!(report.child.denied);
    assert_eq!(report.grandchild.granted, 0);
    assert!(report.grandchild.denied);
    assert!(report.remaining_after > 0);
    // 本地额度比全局大时，剩余取全局的差额。 / When the local allowance is larger, the remainder is the global's.
    let mut wide = document.clone();
    wide.evaluations = 100;
    wide.grandchild_evaluations = 0;
    wide.grandchild_claims = 0;
    wide.claims = 100;
    let report = run(wide);
    assert_eq!(report.child.granted, 9);
    assert_eq!(report.child.used, 9);
    assert_eq!(report.used_after, 10);
    assert_eq!(report.child.remaining, 0);
    assert!(report.parent_refusal_rolls_back);

    // 拒绝：起始已用超过上限 / 申请数超限 / 抽取数超限 / 缺请求。 / Refusals.
    let mut bad = document.clone();
    bad.used = 11;
    assert!(verify(bad).unwrap_err().message.contains("exceeds the limit"));
    let mut missing = base_request();
    missing.model_kind = Some(ModelKind::SearchBudgetVerify);
    assert_eq!(engine.submit(missing).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        let payload = serde_json::to_string(&document).unwrap();
        std::fs::write(directory.join("search-budget.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn predicate_schema_entry_shows_two_lookups_and_duplicate_split() {
    use ospf_rust_bridge::{PredicateSchemaReport, PredicateSchemaRequest, SchemaFieldSpec};

    let field = |path: &str, backend: &str| SchemaFieldSpec { path: path.to_owned(),
        backend_field: backend.to_owned() };
    let engine = NativeEngine::default();
    let verify = |document: PredicateSchemaRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::PredicateSchemaVerify);
        request.predicate_schema = Some(document);
        engine.submit(request)
    };
    let run = |document: PredicateSchemaRequest| {
        let view = verify(document).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: PredicateSchemaReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        engine.release(&view.id).unwrap();
        report
    };
    let document = PredicateSchemaRequest { name: "orders".to_owned(),
        fields: vec![field("id", "orders.id"), field("id", "legacy_id"),
            field("user.name", "name"), field("total", "amount")],
        probes: vec!["id".to_owned(), "user.name".to_owned(), "missing".to_owned()],
        same_field_paths: vec!["user.name".to_owned(), "id".to_owned()] };

    let report = run(document.clone());
    assert_eq!(report.name, "orders");
    assert_eq!(report.field_count, 4);
    assert_eq!(report.distinct_paths, 3);
    assert_eq!(report.duplicate_paths, vec!["id".to_owned()]);
    // 重复路径：`resolve` 先到先得、字段表后到覆盖。 / Duplicates: first-wins vs last-wins.
    assert!(report.resolve_is_first_wins);
    assert!(report.map_is_last_wins);
    let row = |path: &str| report.resolves.iter().find(|row| row.path == path).unwrap();
    assert_eq!(row("id").resolved.as_deref(), Some("orders.id"));
    assert_eq!(row("id").mapped.as_deref(), Some("legacy_id"));
    assert!(!row("id").agrees);
    // 不重复的路径两套口径一致。 / Unique paths agree.
    assert!(row("user.name").agrees);
    assert_eq!(row("user.name").resolved.as_deref(), Some("name"));
    // 未知路径静默无值。 / An unknown path is silently absent.
    assert!(row("missing").resolved.is_none());
    assert!(row("missing").mapped.is_none());
    assert!(report.unknown_is_silent);
    assert!(report.resolver_matches_resolve);
    // 同名字段的后端名就是 `path.value()`。 / The same-name helper derives from `path.value()`.
    assert_eq!(report.same_fields.len(), 2);
    for row in &report.same_fields {
        assert_eq!(row.backend_field, row.value, "{}", row.path);
    }

    // 拒绝：空名字 / 缺后端名 / 缺请求。 / Refusals.
    let mut bad = document.clone();
    bad.name = "   ".to_owned();
    assert!(verify(bad).unwrap_err().message.contains("must not be blank"));
    let mut bad = document.clone();
    bad.fields = vec![field("id", "")];
    assert!(verify(bad).unwrap_err().message.contains("needs a path"));
    let mut missing = base_request();
    missing.model_kind = Some(ModelKind::PredicateSchemaVerify);
    assert_eq!(engine.submit(missing).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        let payload = serde_json::to_string(&document).unwrap();
        std::fs::write(directory.join("predicate-schema.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn sort_by_entry_shows_concat_semantics_and_support_matrix() {
    use ospf_rust_bridge::{SortByReport, SortByRequest, SortItemSpec};

    let item = |path: &str, direction: &str, nulls: Option<&str>| SortItemSpec {
        path: path.to_owned(), direction: direction.to_owned(),
        nulls: nulls.map(|value| value.to_owned()) };
    let engine = NativeEngine::default();
    let verify = |document: SortByRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::SortByVerify);
        request.sort_by = Some(document);
        engine.submit(request)
    };
    let run = |document: SortByRequest| {
        let view = verify(document).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: SortByReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        engine.release(&view.id).unwrap();
        report
    };
    let document = SortByRequest {
        left: vec![item("created_at", "desc", Some("last")), item("id", "asc", None)],
        right: vec![item("id", "asc", None), item("name", "asc", Some("first"))],
        append_asc: vec!["total".to_owned()],
        append_desc: vec!["updated_at".to_owned()],
        support_probe: String::new(),
    };

    let report = run(document.clone());
    // 拼接只拼不整理；`+` 与 `then` 等价。 / Joining only concatenates; `+` equals `then`.
    assert_eq!(report.combined.len(), 4);
    assert!(report.then_equals_add);
    assert!(report.join_keeps_order_and_duplicates);
    assert_eq!(report.combined[0].path, "created_at");
    assert_eq!(report.combined[0].direction, "desc");
    assert_eq!(report.combined[0].nulls.as_deref(), Some("last"));
    assert_eq!(report.combined[2].path, "id");
    assert_eq!(report.duplicate_paths, vec!["id".to_owned()]);
    // 追加到末尾且不去重。 / Appending goes to the end without dedupe.
    assert_eq!(report.appended_asc.len(), 3);
    assert_eq!(report.appended_asc[2].path, "total");
    assert_eq!(report.appended_desc[2].direction, "desc");
    // 支持矩阵 4×2，`Auto` 与 `Always` 一致。 / The 4×2 matrix; `Auto` answers like `Always`.
    assert_eq!(report.support_cells, 8);
    assert_eq!(report.support.len(), 4);
    let row = |policy: &str| report.support.iter().find(|row| row.policy == policy).unwrap();
    assert!(row("Auto").asc && row("Auto").desc);
    assert!(!row("Never").asc && !row("Never").desc);
    assert!(row("OnlyAsc").asc && !row("OnlyAsc").desc);
    assert!(report.auto_answers_like_always);
    assert!(report.never_never_supports);
    assert!(report.only_asc_follows_direction);
    // 默认不带空值顺序。 / The defaults carry no null ordering.
    assert!(report.defaults_carry_no_nulls);

    // 拒绝：空路径 / 未知方向 / 未知空值顺序 / 缺请求。 / Refusals.
    let mut bad = document.clone();
    bad.left = vec![item("", "asc", None)];
    assert!(verify(bad).unwrap_err().message.contains("needs a path"));
    let mut bad = document.clone();
    bad.left = vec![item("a", "up", None)];
    assert!(verify(bad).unwrap_err().message.contains("unknown sort direction"));
    let mut bad = document.clone();
    bad.left = vec![item("a", "asc", Some("middle"))];
    assert!(verify(bad).unwrap_err().message.contains("unknown null ordering"));
    let mut missing = base_request();
    missing.model_kind = Some(ModelKind::SortByVerify);
    assert_eq!(engine.submit(missing).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        let payload = serde_json::to_string(&document).unwrap();
        std::fs::write(directory.join("sort-by.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn variable_kind_domain_entry_shows_bounds_and_the_unenforced_integrality() {
    use ospf_rust_bridge::{VariableKindDomainReport, VariableKindDomainRequest};

    let engine = NativeEngine::default();
    let verify = |document: VariableKindDomainRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::VariableKindDomainVerify);
        request.variable_kind_domain = Some(document);
        engine.submit(request)
    };
    let run = |document: VariableKindDomainRequest| {
        let view = verify(document).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: VariableKindDomainReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        engine.release(&view.id).unwrap();
        report
    };
    let document = VariableKindDomainRequest {
        probes: vec![-1.0, 0.0, 0.5, 1.0, 2.0, 100.0, 101.0] };

    let report = run(document.clone());
    assert_eq!(report.kinds.len(), 8);
    assert!(report.kind_matches_type_name);
    assert!(report.bounds_are_ordered);
    let row = |kind: &str| report.kinds.iter().find(|row| row.kind == kind).unwrap();
    // 离散类型的界。 / The discrete bounds.
    assert_eq!(row("Binary").min, Some(0.0));
    assert_eq!(row("Binary").max, Some(1.0));
    assert_eq!(row("Ternary").max, Some(2.0));
    assert_eq!(row("BalancedTernary").min, Some(-1.0));
    // 实测：整数性根本没被校验，`0.5` 在离散类型里也合法。 
    // Measured: integrality is not enforced; `0.5` is legal even for the discrete kinds.
    assert!(report.integrality_is_not_enforced);
    assert!(row("Binary").is_integer && row("Binary").half_valid);
    assert_eq!(row("Binary").validity,
        vec![false, true, true, true, false, false, false]);
    // `Percentage` 的上界是 1，不是 100。 / `Percentage` is bounded by 1, not 100.
    assert!(report.percentage_upper_is_one);
    assert_eq!(row("Percentage").max, Some(1.0));
    assert_eq!(row("Percentage").validity,
        vec![false, true, true, true, false, false, false]);
    // 下界 0 却声明有符号；无符号的只有 UInteger / UContinuous。 
    // A zero lower bound with a signed declaration; only the two U-kinds are unsigned.
    assert!(report.signedness_is_independent);
    assert!(row("Binary").is_signed);
    assert!(!row("UInteger").is_signed);
    assert!(!row("UContinuous").is_signed);
    // 无界类型接受 1e9。 / Unbounded kinds accept 1e9.
    assert!(report.unbounded_kinds_accept_large);
    assert_eq!(row("Integer").min, None);
    assert_eq!(row("Continuous").max, None);
    assert!(row("Integer").large_valid);
    assert!(!row("Binary").large_valid);

    // 空探测表也合法。 / An empty probe list is legal.
    let empty = run(VariableKindDomainRequest { probes: Vec::new() });
    assert!(empty.kinds.iter().all(|row| row.validity.is_empty()));
    assert!(empty.integrality_is_not_enforced);

    // 拒绝：非有限探测 / 缺请求。 / Refusals.
    assert!(verify(VariableKindDomainRequest { probes: vec![f64::NAN] }).unwrap_err().message
        .contains("must be finite"));
    let mut missing = base_request();
    missing.model_kind = Some(ModelKind::VariableKindDomainVerify);
    assert_eq!(engine.submit(missing).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        let payload = serde_json::to_string(&document).unwrap();
        std::fs::write(directory.join("variable-kind-domain.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn critical_assembly_entry_shows_prerequisites_and_sticky_notes() {
    use ospf_rust_bridge::{AssemblyCase, CriticalAssemblyReport, CriticalAssemblyRequest};

    let case = |name: &str| AssemblyCase { name: name.to_owned(), with_baseline: true,
        proven_optimal: true, optimal_value: Some(1.0),
        baseline_status: "reachable".to_owned(), perturbation_count: 0,
        perturbation_status: "reachable".to_owned(), note_unavailable: Vec::new() };
    let engine = NativeEngine::default();
    let verify = |document: CriticalAssemblyRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::CriticalAssemblyVerify);
        request.critical_assembly = Some(document);
        engine.submit(request)
    };
    let run = |document: CriticalAssemblyRequest| {
        let view = verify(document).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: CriticalAssemblyReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        engine.release(&view.id).unwrap();
        report
    };
    let mut no_baseline = case("no-baseline");
    no_baseline.with_baseline = false;
    let mut noted = case("noted-then-proven");
    noted.note_unavailable = vec!["manual note".to_owned(), "manual note".to_owned()];
    let mut no_objective = case("perturbation-no-objective");
    no_objective.proven_optimal = false;
    no_objective.optimal_value = None;
    no_objective.perturbation_count = 1;
    let mut with_objective = case("with-objective");
    with_objective.perturbation_count = 2;
    let document = CriticalAssemblyRequest { cases: vec![no_baseline, case("baseline-only"),
        noted, no_objective, with_objective] };

    let report = run(document.clone());
    let row = |name: &str| report.rows.iter().find(|row| row.name == name).unwrap();
    // baseline 是唯一硬前置。 / The baseline is the only hard prerequisite.
    assert!(report.baseline_is_the_only_prerequisite);
    assert!(!row("no-baseline").built);
    assert!(row("no-baseline").error.as_deref().unwrap().contains("requires a baseline summary"));
    // 已证明最优 + 缺 activity ≠ Reachable。 / Proven plus a missing activity is not Reachable.
    assert!(report.proven_optimal_is_not_enough);
    assert!(report.missing_activity_degrades_to_unsupported);
    assert_eq!(row("baseline-only").status.as_deref(), Some("Unsupported"));
    assert!(row("baseline-only").unavailable_reasons.iter()
        .any(|reason| reason.contains("activity stage was not run")));
    assert_eq!(row("baseline-only").schema_version.as_deref(), Some("1.0"));
    // 粘性且原样重复。 / Sticky and duplicated verbatim.
    assert!(report.note_unavailable_is_sticky);
    assert_eq!(row("noted-then-proven").unavailable_reasons.iter()
        .filter(|reason| reason.as_str() == "manual note").count(), 2);
    // 没有已证明目标值就建不出有效性排名。 / No proven objective, no effectiveness ranking.
    assert!(report.effectiveness_needs_proven_objective);
    assert!(!row("perturbation-no-objective").built);
    assert!(row("perturbation-no-objective").error.as_deref().unwrap()
        .contains("requires a proven baseline objective value"));
    // 有目标值时能建出排名，但 activity 仍缺。 / With an objective the ranking builds; activity is still missing.
    assert!(row("with-objective").built);
    assert!(row("with-objective").has_effectiveness);
    assert!(!row("with-objective").has_funnel);
    assert_eq!(row("with-objective").status.as_deref(), Some("Unsupported"));

    // 拒绝：未知状态 / 空白理由 / 缺请求。 / Refusals.
    let mut bad = case("bad");
    bad.baseline_status = "maybe".to_owned();
    assert!(verify(CriticalAssemblyRequest { cases: vec![bad] }).unwrap_err().message
        .contains("unknown analysis status"));
    let mut blank = case("blank");
    blank.note_unavailable = vec!["  ".to_owned()];
    assert!(verify(CriticalAssemblyRequest { cases: vec![blank] }).unwrap_err().message
        .contains("must not be blank"));
    let mut missing = base_request();
    missing.model_kind = Some(ModelKind::CriticalAssemblyVerify);
    assert_eq!(engine.submit(missing).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        let payload = serde_json::to_string(&document).unwrap();
        std::fs::write(directory.join("critical-assembly.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn flatten_identity_entry_shows_that_keys_are_addresses() {
    use ospf_rust_bridge::{FlattenIdentityReport, FlattenIdentityRequest};

    let engine = NativeEngine::default();
    let verify = |document: FlattenIdentityRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::FlattenIdentityVerify);
        request.flatten_identity = Some(document);
        engine.submit(request)
    };
    let run = |document: FlattenIdentityRequest| {
        let view = verify(document).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: FlattenIdentityReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        engine.release(&view.id).unwrap();
        report
    };
    let document = FlattenIdentityRequest { coefficient: 2.5, var_index: 3, other_var: 4,
        forged_key: 0xdead_beef };

    let report = run(document.clone());
    // 内容相同也拿不到相同的键。 / Equal content never shares a key.
    assert!(report.equal_content_gives_distinct_keys);
    assert!(report.clone_gets_its_own_key);
    assert_ne!(report.first_key, report.clone_key);
    assert_ne!(report.first_key, report.twin_key);
    assert_ne!(report.clone_key, report.twin_key);
    assert_ne!(report.first_key, 0);
    // 键不能当等值判据。 / Keys cannot serve as an equality test.
    assert!(report.different_content_gives_distinct_keys);
    assert!(report.hashset_does_not_dedupe);
    // 伪造键可行；重建必换键。 / Keys can be forged; a rebuild always gets a fresh one.
    assert!(report.from_raw_forges_collisions);
    assert!(report.rebuilt_polynomial_gets_a_fresh_key);
    assert!(report.monomial_is_linear);
    assert!((report.coefficient - 2.5).abs() < f64::EPSILON);

    // 拒绝：非有限系数 / 变量序号越界 / 缺请求。 / Refusals.
    let mut bad = document.clone();
    bad.coefficient = f64::NAN;
    assert!(verify(bad).unwrap_err().message.contains("must be finite"));
    let mut bad = document.clone();
    bad.other_var = u32::MAX as u64 + 1;
    assert!(verify(bad).unwrap_err().message.contains("must be below"));
    let mut missing = base_request();
    missing.model_kind = Some(ModelKind::FlattenIdentityVerify);
    assert_eq!(engine.submit(missing).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        let payload = serde_json::to_string(&document).unwrap();
        std::fs::write(directory.join("flatten-identity.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn a_continuation_entry_refuses_foreign_payload_fields() {
    use ospf_rust_bridge::{FlattenIdentityRequest, SortByRequest, VariableKindDomainRequest};

    let engine = NativeEngine::default();
    // 1) 自己的字段不在场、却夹带别人的字段 → 拒绝。 / Own field absent while carrying another's → refused.
    let mut request = base_request();
    request.model_kind = Some(ModelKind::SortByVerify);
    request.flatten_identity = Some(FlattenIdentityRequest { coefficient: 1.0, var_index: 1,
        other_var: 2, forged_key: 7 });
    let error = engine.submit(request).unwrap_err();
    assert_eq!(error.code, "InvalidInput");
    assert!(error.message.contains("only its own payload field"), "{}", error.message);

    // 2) 自己的字段在场、同时夹带别人的字段 → 同样拒绝。 / Own present plus another's → also refused.
    let mut request = base_request();
    request.model_kind = Some(ModelKind::FlattenIdentityVerify);
    request.flatten_identity = Some(FlattenIdentityRequest { coefficient: 1.0, var_index: 1,
        other_var: 2, forged_key: 7 });
    request.sort_by = Some(SortByRequest { left: Vec::new(), right: Vec::new(),
        append_asc: Vec::new(), append_desc: Vec::new(), support_probe: String::new() });
    let error = engine.submit(request).unwrap_err();
    assert_eq!(error.code, "InvalidInput");
    assert!(error.message.contains("only its own payload field"), "{}", error.message);

    // 3) 跨两代入口的夹带同样被拦住（变量定义域 + 排序）。 / Smuggling across generations is caught too.
    let mut request = base_request();
    request.model_kind = Some(ModelKind::VariableKindDomainVerify);
    request.sort_by = Some(SortByRequest { left: Vec::new(), right: Vec::new(),
        append_asc: Vec::new(), append_desc: Vec::new(), support_probe: String::new() });
    let error = engine.submit(request).unwrap_err();
    assert!(error.message.contains("only its own payload field"), "{}", error.message);

    // 4) 只带自己的字段仍然照常工作——守卫没有过宽。 / Own-only still works: the guard is not over-broad.
    let mut request = base_request();
    request.model_kind = Some(ModelKind::VariableKindDomainVerify);
    request.variable_kind_domain = Some(VariableKindDomainRequest { probes: vec![0.0] });
    let view = engine.submit(request).unwrap();
    let finished = settle(&engine, &view);
    assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
    engine.release(&view.id).unwrap();
}

#[test]
fn master_cut_entry_shows_the_construct_normalize_split() {
    use ospf_rust_bridge::{MasterCutReport, MasterCutRequest, MasterCutTerm};

    let term = |variable: &str, coefficient: f64| MasterCutTerm {
        variable: variable.to_owned(), coefficient };
    let engine = NativeEngine::default();
    let verify = |document: MasterCutRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::MasterCutVerify);
        request.master_cut = Some(document);
        engine.submit(request)
    };
    let run = |document: MasterCutRequest| {
        let view = verify(document).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: MasterCutReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        engine.release(&view.id).unwrap();
        report
    };
    let document = MasterCutRequest { id: "cut-1".to_owned(),
        terms: vec![term("x", 1.0), term("x", 2.5), term("y", 0.0)],
        sense: "le".to_owned(), rhs: 4.0,
        proof_reference: Some("  proof-1  ".to_owned()), origin: "capacity".to_owned(),
        reason: "cutoff".to_owned(), verified_global: true, normalize: true,
        normalize_twice: false };

    let report = run(document.clone());
    assert!(report.normalized);
    assert_eq!(report.error, None);
    // 规范化：合并同变量、丢零系数、trim 证明引用。
    // Normalized: duplicates merged, zeros dropped, the proof reference trimmed.
    assert_eq!(report.term_count, 1);
    assert_eq!(report.terms[0].variable, "x");
    assert!((report.terms[0].coefficient - 3.5).abs() < f64::EPSILON);
    assert_eq!(report.proof_status, "Verified");
    assert_eq!(report.proof_reference.as_deref(), Some("proof-1"));
    assert_eq!(report.encoding, "Linear");
    assert_eq!(report.sense, "le");
    assert!(!report.expression_key.is_empty());
    // 九条规则属性与请求无关。 / The nine rule properties are independent of the request.
    assert!(report.new_merges_duplicates);
    assert!(report.new_keeps_zeros);
    assert!(report.normalize_drops_zeros);
    assert!(report.normalize_refuses_blank_id);
    assert!(report.normalize_refuses_non_finite_rhs);
    assert!(report.verified_needs_reference);
    assert!(report.blank_reference_is_dropped);
    assert!(report.expression_key_ignores_identity);
    assert!(report.normalize_is_idempotent);

    // 不规范化时零系数留下。 / Without normalize the zero coefficient survives.
    let mut plain = document.clone();
    plain.normalize = false;
    plain.verified_global = false;
    let plain = run(plain);
    assert!(!plain.normalized);
    assert_eq!(plain.term_count, 2);
    assert!(plain.terms.iter().any(|row| row.variable == "y" && row.coefficient == 0.0));

    // 规范化两次仍然一次的结果。 / Normalizing twice equals once.
    let mut twice = document.clone();
    twice.normalize_twice = true;
    assert_eq!(run(twice).term_count, 1);

    // 拒绝：空白 ID / 非有限 rhs / `Verified` 无引用 / 未知方向 / 缺请求。 / Refusals.
    let mut blank = document.clone();
    blank.id = "  ".to_owned();
    assert!(verify(blank).unwrap_err().message.contains("must not be empty"));
    let mut infinite = document.clone();
    infinite.rhs = f64::INFINITY;
    assert!(verify(infinite).unwrap_err().message.contains("non-finite rhs"));
    let mut no_reference = document.clone();
    no_reference.proof_reference = Some("   ".to_owned());
    assert!(verify(no_reference).unwrap_err().message.contains("must carry a proof reference"));
    let mut wrong_sense = document.clone();
    wrong_sense.sense = "lt".to_owned();
    assert!(verify(wrong_sense).unwrap_err().message.contains("unknown sense"));
    let mut missing = base_request();
    missing.model_kind = Some(ModelKind::MasterCutVerify);
    assert_eq!(engine.submit(missing).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        let payload = serde_json::to_string(&document).unwrap();
        std::fs::write(directory.join("master-cut.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn boolean_fold_entry_shows_the_three_folding_rules() {
    use ospf_rust_bridge::{BooleanFoldReport, BooleanFoldRequest};

    let engine = NativeEngine::default();
    let verify = |document: BooleanFoldRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::BooleanFoldVerify);
        request.boolean_fold = Some(document);
        engine.submit(request)
    };
    let run = |document: BooleanFoldRequest| {
        let view = verify(document).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: BooleanFoldReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        engine.release(&view.id).unwrap();
        report
    };
    let text = |values: &[&str]| values.iter().map(|value| (*value).to_owned()).collect::<Vec<_>>();
    let document = BooleanFoldRequest { collected: text(&["a", "b"]),
        combined: text(&["a", "b"]), mode: "and".to_owned() };

    let report = run(document.clone());
    // 尾部后缀折叠成一层：and(a, b)。 / The trailing suffix folds to one level.
    assert_eq!(report.shape, "and(2)");
    assert_eq!(report.node_count, 3);
    assert!(report.trailing_suffix_folds);
    // 空作用域给常量。 / An empty scope yields constants.
    assert_eq!(report.empty_and_shape, "constant(true)");
    assert_eq!(report.empty_or_shape, "constant(false)");
    assert!(report.empty_scope_is_constant);
    assert!(report.single_element_loses_wrapper);
    assert!(report.single_argument_unwraps);
    assert!(report.wrong_order_does_not_fold);
    assert!(report.interleaved_item_survives);

    // 乱序不折叠：多一层。 / A reversed order does not fold: one extra level.
    let mut reordered = document.clone();
    reordered.combined = text(&["b", "a"]);
    let reordered = run(reordered);
    assert_eq!(reordered.shape, "and(3)");
    assert_eq!(reordered.node_count, 6);

    // 单元素拆壳。 / A single element loses its wrapper.
    let mut single = document.clone();
    single.collected = text(&["a"]);
    single.combined = Vec::new();
    let single = run(single);
    assert!(single.shape.starts_with("comparison"), "{}", single.shape);
    assert_eq!(single.node_count, 1);

    // `or` 同样折叠。 / `or` folds the same way.
    let mut either = document.clone();
    either.mode = "or".to_owned();
    assert_eq!(run(either).shape, "or(2)");

    // 拒绝：空白字段名 / 未知模式 / 缺请求。 / Refusals.
    let mut blank = document.clone();
    blank.collected = text(&["  "]);
    assert!(verify(blank).unwrap_err().message.contains("every field needs a name"));
    let mut wrong = document.clone();
    wrong.mode = "xor".to_owned();
    assert!(verify(wrong).unwrap_err().message.contains("unknown mode"));
    let mut missing = base_request();
    missing.model_kind = Some(ModelKind::BooleanFoldVerify);
    assert_eq!(engine.submit(missing).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        let payload = serde_json::to_string(&document).unwrap();
        std::fs::write(directory.join("boolean-fold.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn field_resolution_entry_shows_the_four_states_and_the_lossy_bridge() {
    use ospf_rust_bridge::{FieldResolutionReport, FieldResolutionRequest};

    let engine = NativeEngine::default();
    let verify = |document: FieldResolutionRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::FieldResolutionVerify);
        request.field_resolution = Some(document);
        engine.submit(request)
    };
    let run = |document: FieldResolutionRequest| {
        let view = verify(document).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: FieldResolutionReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        engine.release(&view.id).unwrap();
        report
    };
    let request = |backend: &str, candidates: Vec<&str>, invalid: &str| FieldResolutionRequest {
        path: "orders.status".to_owned(), backend: backend.to_owned(),
        candidates: candidates.into_iter().map(|value| value.to_owned()).collect(),
        invalid_reason: invalid.to_owned() };

    // 四态各自可达。 / Each of the four states is reachable.
    let resolved = run(request("user_status", Vec::new(), ""));
    assert_eq!(resolved.state, "resolved");
    assert_eq!(resolved.resolved.as_deref(), Some("user_status"));
    assert_eq!(resolved.nullable.as_deref(), Some("user_status"));
    let missing = run(request("", Vec::new(), ""));
    assert_eq!(missing.state, "missing");
    let ambiguous = run(request("", vec!["a", "b"], ""));
    assert_eq!(ambiguous.state, "ambiguous");
    assert_eq!(ambiguous.candidates.len(), 2);
    let invalid = run(request("", Vec::new(), "unknown table"));
    assert_eq!(invalid.state, "invalidConfiguration");
    assert_eq!(invalid.reason.as_deref(), Some("unknown table"));

    // 三种失败在可空接口上都是 `None`，无法区分。 / All three failures are `None` and indistinguishable.
    assert_eq!(missing.nullable, None);
    assert_eq!(ambiguous.nullable, None);
    assert_eq!(invalid.nullable, None);
    assert!(missing.missing_collapses);
    assert!(ambiguous.ambiguous_collapses);
    assert!(invalid.invalid_configuration_collapses);
    assert!(missing.failures_are_indistinguishable);
    // 闭包只有两态，且看到的是路径本体。 / The closure is two-state and sees the path body.
    assert!(resolved.closure_is_two_state);
    assert!(resolved.closure_sees_path_value);
    // 闭包看到的就是路径本体（去前缀）。 / The closure sees the prefix-free path body.
    assert_eq!(resolved.closure_sees, "orders.status");
    assert_eq!(resolved.closure.as_deref(), Some("user_status"));
    assert_eq!(ambiguous.closure, None);
    assert!(resolved.four_states_exist);

    // 拒绝：空白路径 / 空白候选 / 缺请求。 / Refusals.
    let mut blank = request("", Vec::new(), "");
    blank.path = "  ".to_owned();
    assert!(verify(blank).unwrap_err().message.contains("path must not be blank"));
    assert!(verify(request("", vec!["  ", "x"], "")).unwrap_err().message
        .contains("candidate must not be blank"));
    let mut missing_request = base_request();
    missing_request.model_kind = Some(ModelKind::FieldResolutionVerify);
    assert_eq!(engine.submit(missing_request).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        let payload = serde_json::to_string(&request("", vec!["a", "b"], "")).unwrap();
        std::fs::write(directory.join("field-resolution.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn scalar_function_entry_shows_the_vocabulary_is_not_a_constraint() {
    use ospf_rust_bridge::{ScalarFunctionReport, ScalarFunctionRequest};

    let engine = NativeEngine::default();
    let verify = |document: ScalarFunctionRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::ScalarFunctionVerify);
        request.scalar_function = Some(document);
        engine.submit(request)
    };
    let run = |document: ScalarFunctionRequest| {
        let view = verify(document).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: ScalarFunctionReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        engine.release(&view.id).unwrap();
        report
    };
    let document = ScalarFunctionRequest { path: "user.name".to_owned(), coalesce_arity: 2,
        raw_name: "abs".to_owned(), raw_arity: 1 };

    let report = run(document.clone());
    // 六个常量与六个构造器一一对应，coalesce 是唯一变参。 
    // The six constants match their builders and `coalesce` is the only variadic one.
    assert_eq!(report.rows.len(), 6);
    assert!(report.constants_match_builders);
    assert!(report.coalesce_is_the_only_variadic);
    assert!(report.coalesce_accepts_no_arguments);
    assert!(report.names_are_distinct);
    let by = |constant: &str| report.rows.iter().find(|row| row.constant == constant).unwrap();
    assert_eq!(by("ABS").name, "abs");
    assert_eq!(by("ABS").arity, 1);
    assert_eq!(by("COALESCE").name, "coalesce");
    assert_eq!(by("COALESCE").arity, 2);
    // 裸构造器不做校验。 / The raw constructor validates nothing.
    assert!(report.raw_name_is_unvalidated);
    assert_eq!(report.raw_name, "abs");
    assert_eq!(report.raw_arity, 1);
    assert!(report.name_matching_is_case_sensitive);

    // 空白名字照收。 / A blank name is accepted.
    let mut blank = document.clone();
    blank.raw_name = "  ".to_owned();
    let blank = run(blank);
    assert_eq!(blank.raw_name, "  ");
    assert!(blank.raw_name_is_unvalidated);
    // 大写名不被归一化。 / An uppercase name is not normalised.
    let mut upper = document.clone();
    upper.raw_name = "ABS".to_owned();
    assert_eq!(run(upper).raw_name, "ABS");
    // 零参 coalesce 合法。 / A zero-argument coalesce is legal.
    let mut empty = document.clone();
    empty.coalesce_arity = 0;
    assert_eq!(run(empty).rows.iter().find(|row| row.constant == "COALESCE").unwrap().arity, 0);

    // 拒绝：空白路径 / 元数超限 / 缺请求。 / Refusals.
    let mut no_path = document.clone();
    no_path.path = "  ".to_owned();
    assert!(verify(no_path).unwrap_err().message.contains("path must not be blank"));
    let mut many = document.clone();
    many.raw_arity = 65;
    assert!(verify(many).unwrap_err().message.contains("at most"));
    let mut missing = base_request();
    missing.model_kind = Some(ModelKind::ScalarFunctionVerify);
    assert_eq!(engine.submit(missing).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        let payload = serde_json::to_string(&document).unwrap();
        std::fs::write(directory.join("scalar-function.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn polynomial_lift_entry_shows_the_asymmetric_degree_bridge() {
    use ospf_rust_bridge::{LinearTermSpec, PolynomialLiftReport, PolynomialLiftRequest, PowerSpec};

    let engine = NativeEngine::default();
    let verify = |document: PolynomialLiftRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::PolynomialLiftVerify);
        request.polynomial_lift = Some(document);
        engine.submit(request)
    };
    let run = |document: PolynomialLiftRequest| {
        let view = verify(document).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: PolynomialLiftReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        engine.release(&view.id).unwrap();
        report
    };
    let document = PolynomialLiftRequest { constant: 4.25,
        terms: vec![LinearTermSpec { variable: 0, coefficient: 1.5 },
            LinearTermSpec { variable: 3, coefficient: -2.0 }],
        powers: vec![PowerSpec { variable: 1, power: 2 }, PowerSpec { variable: 4, power: 1 }],
        missing_variable: 7 };

    let report = run(document.clone());
    // 线性 → 二次：保留常数与项数，全是线性 kind。 
    // Linear to quadratic: the constant and arity survive, every term is a linear kind.
    assert_eq!(report.linear_count, 2);
    assert_eq!(report.quadratic_count, 2);
    assert_eq!(report.quadratic_constant, 4.25);
    assert!(report.quadratic_all_linear_kind);
    assert!(report.from_linear_keeps_constant);
    assert!(report.from_linear_keeps_arity);
    // 规范化态：靠幂次表，缺项读 0。 / The canonical state lives on the power table; an absent variable reads 0.
    assert_eq!(report.canonical_count, 1);
    assert_eq!(report.canonical_total_power, 3);
    assert_eq!(report.missing_power_is_zero, 0);
    assert!(report.canonical_has_no_lift);
    assert!(report.zero_matches_constant);
    assert!(report.constant_only_is_empty);
    assert!(report.add_monomial_does_not_merge);

    // 只有常数：线性态为空。 / Constant only: the linear state is empty.
    let mut bare = document.clone();
    bare.terms = Vec::new();
    let bare = run(bare);
    assert!(bare.linear_is_constant_only);
    assert_eq!(bare.linear_count, 0);
    // 同一变量两次不合并。 / The same variable twice does not merge.
    let mut duplicated = document.clone();
    duplicated.terms = vec![LinearTermSpec { variable: 2, coefficient: 1.0 },
        LinearTermSpec { variable: 2, coefficient: 1.0 }];
    assert_eq!(run(duplicated).linear_count, 2);

    // 拒绝：非有限常数 / 非有限系数 / 负幂次 / 缺请求。 / Refusals.
    let mut infinite = document.clone();
    infinite.constant = f64::NAN;
    assert!(verify(infinite).unwrap_err().message.contains("constant must be finite"));
    let mut bad_coefficient = document.clone();
    bad_coefficient.terms = vec![LinearTermSpec { variable: 0, coefficient: f64::INFINITY }];
    assert!(verify(bad_coefficient).unwrap_err().message.contains("coefficient must be finite"));
    let mut negative = document.clone();
    negative.powers = vec![PowerSpec { variable: 1, power: -1 }];
    assert!(verify(negative).unwrap_err().message.contains("powers must be non-negative"));
    let mut missing = base_request();
    missing.model_kind = Some(ModelKind::PolynomialLiftVerify);
    assert_eq!(engine.submit(missing).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        let payload = serde_json::to_string(&document).unwrap();
        std::fs::write(directory.join("polynomial-lift.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn bounded_no_good_entry_shows_the_family_contract() {
    use ospf_rust_bridge::{BoundedNoGoodReport, BoundedNoGoodRequest, BoundedNoGoodValue,
        BoundedNoGoodVariableSpec};

    let engine = NativeEngine::default();
    let verify = |document: BoundedNoGoodRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::BoundedNoGoodVerify);
        request.bounded_no_good = Some(document);
        engine.submit(request)
    };
    let run = |document: BoundedNoGoodRequest| {
        let view = verify(document).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: BoundedNoGoodReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        engine.release(&view.id).unwrap();
        report
    };
    let document = BoundedNoGoodRequest {
        variables: vec![
            BoundedNoGoodVariableSpec { id: "m.a".to_owned(), lower: 0, upper: 4, binary: false },
            BoundedNoGoodVariableSpec { id: "m.b".to_owned(), lower: 0, upper: 4, binary: false },
            BoundedNoGoodVariableSpec { id: "m.c".to_owned(), lower: 0, upper: 1, binary: true },
        ],
        assignment: vec![
            BoundedNoGoodValue { id: "m.a".to_owned(), value: 2 },
            BoundedNoGoodValue { id: "m.b".to_owned(), value: 1 },
            BoundedNoGoodValue { id: "m.c".to_owned(), value: 1 },
        ],
        family_variables: Vec::new(),
        proof_reference: "probe/proof".to_owned(),
    };

    let report = run(document.clone());
    // 3 个变量 → 10 行；辅助变量 6 个且每行都声明全部。 
    // Three variables give ten rows; six auxiliaries are declared by every row.
    assert_eq!(report.variable_count, 3);
    assert_eq!(report.rows.len(), 10);
    assert_eq!(report.auxiliary_count, 6);
    assert!(report.rows_are_three_per_variable_plus_one);
    assert!(report.every_row_declares_every_auxiliary);
    assert!(report.require_difference_has_no_variable);
    assert!(report.family_id_is_uniform);
    assert!(report.assignment_is_uniform);
    assert!(report.rows_follow_variable_order);
    assert_eq!(report.rows.last().unwrap().row, "requireDifference");
    assert_eq!(report.rows.last().unwrap().variable, None);
    // 冲突成员去重。 / Conflict members deduplicate.
    assert_eq!(report.conflict.members.len(), 3);
    assert!(report.conflict_members_deduplicate);

    // 单变量家族 → 4 行。 / A one-variable family gives four rows.
    let mut single = document.clone();
    single.family_variables = vec!["m.a".to_owned()];
    let single = run(single);
    assert_eq!(single.rows.len(), 4);
    let kinds: Vec<&str> = single.rows.iter().map(|row| row.row.as_str()).collect();
    assert_eq!(kinds, vec!["lowerSide", "upperSide", "atMostOne", "requireDifference"]);

    // 拒绝：空变量 / 空白证明 / 二元界错 / 值域外 / 家族变量未赋值 / 缺请求。
    // Refusals: no variables, blank proof, wrong binary bounds, out-of-domain, unassigned family variable, missing request.
    let mut empty = document.clone();
    empty.variables = Vec::new();
    assert!(verify(empty).unwrap_err().message.contains("at least one variable"));
    let mut blank = document.clone();
    blank.proof_reference = "  ".to_owned();
    assert!(verify(blank).unwrap_err().message.contains("proof reference must not be blank"));
    let mut bad_binary = document.clone();
    bad_binary.variables[2].upper = 5;
    assert!(verify(bad_binary).unwrap_err().message.contains("bounded by 0 and 1"));
    let mut out_of_domain = document.clone();
    out_of_domain.assignment[0].value = 9;
    assert!(verify(out_of_domain).is_err());
    let mut foreign = document.clone();
    foreign.family_variables = vec!["not.assigned".to_owned()];
    assert!(verify(foreign).unwrap_err().message.contains("family variable is not assigned"));
    let mut missing = base_request();
    missing.model_kind = Some(ModelKind::BoundedNoGoodVerify);
    assert_eq!(engine.submit(missing).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        let payload = serde_json::to_string(&document).unwrap();
        std::fs::write(directory.join("bounded-no-good.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn effectiveness_aggregate_entry_shows_two_meanings_of_improvement() {
    use ospf_rust_bridge::{EffectivenessAggregateReport, EffectivenessAggregateRequest,
        FunnelCandidateSpec, ObservationSpec, PerturbationSpec};

    let engine = NativeEngine::default();
    let verify = |document: EffectivenessAggregateRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::EffectivenessAggregateVerify);
        request.effectiveness_aggregate = Some(document);
        engine.submit(request)
    };
    let run = |document: EffectivenessAggregateRequest| {
        let view = verify(document).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: EffectivenessAggregateReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        engine.release(&view.id).unwrap();
        report
    };
    let observed = |delta: f64, improvement: f64, improved: bool, proven: bool| ObservationSpec {
        delta, improvement: Some(improvement), improved: Some(improved), objective_proven: proven };
    let document = EffectivenessAggregateRequest { baseline_objective: 100.0,
        dual_threshold: 1e-9,
        funnel: vec![
            FunnelCandidateSpec { constraint_id: "c.a".to_owned(), group: Some("g".to_owned()),
                tier: "a".to_owned(), dual_value: Some(2.5), local_effective: Some(true) },
            FunnelCandidateSpec { constraint_id: "c.b".to_owned(), group: None,
                tier: "c".to_owned(), dual_value: Some(0.1), local_effective: None },
        ],
        reports: vec![
            PerturbationSpec { constraint_id: "c.a".to_owned(), removal_effective: None,
                observations: vec![observed(4.0, 8.0, true, true), observed(1.0, 2.0, true, true),
                    observed(2.0, 3.0, false, true)] },
            PerturbationSpec { constraint_id: "c.b".to_owned(), removal_effective: Some(true),
                observations: Vec::new() },
        ] };

    let report = run(document.clone());
    // 只有已证明的观察参与：最大改善 8.0、first delta 取最小 1.0、边际 8.0。 
    // Only proven observations participate: max 8.0, the smallest delta 1.0, marginal 8.0.
    assert_eq!(report.rows[0].constraint_id, "c.a");
    assert_eq!(report.rows[0].max_improvement, Some(8.0));
    assert_eq!(report.rows[0].first_effective_delta, Some(1.0));
    assert_eq!(report.rows[0].marginal_effect, Some(8.0));
    assert!(report.rows[0].globally_effective);
    assert_eq!(report.order, vec!["c.a".to_owned(), "c.b".to_owned()]);
    // 删除有效：报告说有改善，排序说没有 → 两份判据不一致。 
    // Removal effective: the report says improved, the ranking says no, so the predicates disagree.
    let removal = report.rows.iter().find(|row| row.constraint_id == "c.b").unwrap();
    assert_eq!(removal.max_improvement, None);
    assert!(!removal.globally_effective);
    assert!(removal.report_has_global_improvement);
    assert!(!removal.predicates_agree);
    assert!(report.unproven_observations_are_ignored);
    assert!(report.removal_improvement_is_not_a_gain);
    assert!(report.the_two_predicates_can_disagree);
    assert!(report.marginal_effect_needs_positive_delta);
    assert!(report.first_delta_is_the_smallest);
    assert!(report.none_improvement_sorts_last);
    assert!(report.ties_fall_back_to_absolute_dual);
    assert!(report.missing_candidate_is_unclassified);

    // 未证明的改善不进排序。 / An unproven improvement never enters the ranking.
    let mut unproven = document.clone();
    unproven.funnel = Vec::new();
    unproven.reports = vec![PerturbationSpec { constraint_id: "u".to_owned(),
        removal_effective: None, observations: vec![observed(1.0, 5.0, true, false)] }];
    assert_eq!(run(unproven).rows[0].max_improvement, None);

    // 拒绝：空报告 / 非有限基线 / 重复约束 / delta 非正 / 缺请求。 
    // Refusals: no reports, non-finite baseline, duplicate constraint, non-positive delta, missing request.
    let mut empty = document.clone();
    empty.reports = Vec::new();
    assert!(verify(empty).unwrap_err().message.contains("at least one report"));
    let mut infinite = document.clone();
    infinite.baseline_objective = f64::NAN;
    assert!(verify(infinite).unwrap_err().message.contains("baseline objective must be finite"));
    let mut duplicate = document.clone();
    duplicate.reports[1].constraint_id = "c.a".to_owned();
    assert!(verify(duplicate).unwrap_err().message.contains("repeats a constraint"));
    let mut zero = document.clone();
    zero.reports[0].observations[0].delta = 0.0;
    assert!(verify(zero).unwrap_err().message.contains("deltas must be finite and positive"));
    let mut missing = base_request();
    missing.model_kind = Some(ModelKind::EffectivenessAggregateVerify);
    assert_eq!(engine.submit(missing).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        let payload = serde_json::to_string(&document).unwrap();
        std::fs::write(directory.join("effectiveness-aggregate.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn assumption_binding_entry_shows_identity_is_a_length_prefixed_key() {
    use ospf_rust_bridge::{AssumptionBindingReport, AssumptionBindingRequest,
        AssumptionMappingSpec, AssumptionSpec};

    let engine = NativeEngine::default();
    let verify = |document: AssumptionBindingRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::AssumptionBindingVerify);
        request.assumption_binding = Some(document);
        engine.submit(request)
    };
    let run = |document: AssumptionBindingRequest| {
        let view = verify(document).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: AssumptionBindingReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        engine.release(&view.id).unwrap();
        report
    };
    let document = AssumptionBindingRequest {
        assumptions: vec![
            AssumptionSpec { variable: "cp.x".to_owned(), shape: "equal".to_owned(),
                negated: false, value: 5, values: Vec::new() },
            AssumptionSpec { variable: "cp.y".to_owned(), shape: "sparseDomain".to_owned(),
                negated: false, value: 0, values: vec![3, 1, 3] },
            AssumptionSpec { variable: "cp.z".to_owned(), shape: "literal".to_owned(),
                negated: true, value: 0, values: Vec::new() },
        ],
        mappings: vec![
            AssumptionMappingSpec { assumption: 0, master_variable: "m.x".to_owned() },
            AssumptionMappingSpec { assumption: 1, master_variable: "m.y".to_owned() },
            AssumptionMappingSpec { assumption: 2, master_variable: "m.z".to_owned() },
        ],
        conflict_members: vec![0, 2],
        proof_reference: "probe/proof".to_owned(),
    };

    let report = run(document.clone());
    // 身份是长度前缀拼接：`equal/4:cp.x/5`。 / The identity is length-prefixed: `equal/4:cp.x/5`.
    assert_eq!(report.rows[0].stable_id, "equal/4:cp.x/5");
    assert_eq!(report.rows[0].prefix, "equal");
    assert_eq!(report.rows[2].stable_id, "literal/4:cp.z/false");
    // `sparseDomain` 的身份丢掉值域内容。 / A `sparseDomain`'s identity drops its values.
    assert_eq!(report.rows[1].stable_id, "sparse-domain/4:cp.y");
    assert_eq!(report.rows[1].value_count, 2);
    assert!(report.identity_is_length_prefixed);
    assert!(report.sparse_domain_identity_ignores_values);
    assert!(report.different_domains_collide);
    assert!(report.different_numbers_do_not_collide);
    assert!(report.polarity_is_part_of_the_identity);
    assert!(report.domain_values_are_normalized);
    assert!(report.separators_are_unambiguous);
    assert!(report.mapping_must_cover_exactly_once);
    // 冲突子集经映射解析。 / The conflict subset resolves through the mapping.
    assert_eq!(report.conflict_members, vec!["m.x".to_owned(), "m.z".to_owned()]);
    assert_eq!(report.proof_reference, "probe/proof");

    // 拒绝：未知形态 / 空白变量 / 映射不全 / 映射重复 / 空白 master / 越界 / 空冲突 / 缺请求。
    // Refusals: unknown shape, blank variable, incomplete mapping, duplicate mapping, blank master, out of range,
    // empty conflict, missing request.
    let mut unknown = document.clone();
    unknown.assumptions[0].shape = "between".to_owned();
    assert!(verify(unknown).unwrap_err().message.contains("unknown shape"));
    let mut blank = document.clone();
    blank.assumptions[0].variable = "  ".to_owned();
    assert!(verify(blank).unwrap_err().message.contains("variable IDs must not be blank"));
    let mut incomplete = document.clone();
    incomplete.mappings.pop();
    assert!(verify(incomplete).unwrap_err().message.contains("cover every assumption exactly once"));
    let mut duplicate = document.clone();
    duplicate.mappings.push(AssumptionMappingSpec { assumption: 0,
        master_variable: "m.again".to_owned() });
    assert!(verify(duplicate).unwrap_err().message.contains("mappings must be unique"));
    let mut blank_master = document.clone();
    blank_master.mappings[0].master_variable = "  ".to_owned();
    assert!(verify(blank_master).unwrap_err().message.contains("master variables must not be blank"));
    let mut out_of_range = document.clone();
    out_of_range.mappings[0].assumption = 9;
    assert!(verify(out_of_range).unwrap_err().message.contains("mapping position is out of range"));
    let mut empty_conflict = document.clone();
    empty_conflict.conflict_members = Vec::new();
    assert!(verify(empty_conflict).is_err());
    let mut missing = base_request();
    missing.model_kind = Some(ModelKind::AssumptionBindingVerify);
    assert_eq!(engine.submit(missing).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        let payload = serde_json::to_string(&document).unwrap();
        std::fs::write(directory.join("assumption-binding.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn tier_classification_entry_shows_unknown_never_becomes_unimportant() {
    use ospf_rust_bridge::{CandidateSpec, TierClassificationReport, TierClassificationRequest};

    let engine = NativeEngine::default();
    let verify = |document: TierClassificationRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::TierClassificationVerify);
        request.tier_classification = Some(document);
        engine.submit(request)
    };
    let run = |document: TierClassificationRequest| {
        let view = verify(document).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: TierClassificationReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        engine.release(&view.id).unwrap();
        report
    };
    let document = TierClassificationRequest { dual_threshold: 1e-9, shortlist_limit: 2,
        candidates: vec![
            CandidateSpec { constraint_id: "c.hot".to_owned(), activity: "active".to_owned(),
                dual_value: Some(3.0) },
            CandidateSpec { constraint_id: "c.warm".to_owned(), activity: "active".to_owned(),
                dual_value: Some(0.0) },
            CandidateSpec { constraint_id: "c.blind".to_owned(),
                activity: "nearlyActive".to_owned(), dual_value: None },
            CandidateSpec { constraint_id: "c.cold".to_owned(), activity: "inactive".to_owned(),
                dual_value: None },
            CandidateSpec { constraint_id: "c.broken".to_owned(), activity: "violated".to_owned(),
                dual_value: None },
        ] };

    let report = run(document.clone());
    // 分层只由证据推导。 / The tiers follow the evidence only.
    assert_eq!(report.rows[0].tier, "A");
    assert_eq!(report.rows[1].tier, "B");
    assert_eq!(report.rows[2].tier, "UNCLASSIFIED");
    assert_eq!(report.rows[3].tier, "C");
    assert_eq!(report.rows[4].tier, "UNCLASSIFIED");
    assert_eq!(report.tier_a, 1);
    assert_eq!(report.tier_b, 1);
    assert_eq!(report.tier_c, 1);
    assert_eq!(report.unclassified, 2);
    // 线序名与枚举名不同。 / The wire name differs from the label.
    assert_eq!(report.rows[0].tier_wire, "TIER_A");
    assert_eq!(report.rows[2].tier_wire, "UNCLASSIFIED");
    // 短名单降权但不丢弃。 / The shortlist deprioritises without dropping.
    assert_eq!(report.shortlist, vec!["c.hot".to_owned(), "c.warm".to_owned()]);
    assert!(report.shortlist_prefers_defaults);
    assert!(report.shortlist_returns_everything_when_wide);
    assert!(report.activity_without_dual_is_unclassified);
    assert!(report.violated_is_not_tier_c);
    assert!(report.unknown_is_unclassified);
    assert!(report.threshold_equality_is_tier_b);
    assert!(report.unclassified_is_not_a_shortlist_default);
    assert!(report.wire_name_differs_from_label);

    // 名额够 → 全部返回。 / Enough slots return everything.
    let mut wide = document.clone();
    wide.shortlist_limit = 99;
    assert_eq!(run(wide).shortlist.len(), 5);
    // 恰好等于阈值 → B。 / Exactly at the threshold gives B.
    let mut at = document.clone();
    at.dual_threshold = 5.0;
    at.shortlist_limit = 1;
    at.candidates = vec![CandidateSpec { constraint_id: "only".to_owned(),
        activity: "active".to_owned(), dual_value: Some(5.0) }];
    assert_eq!(run(at).rows[0].tier, "B");

    // 拒绝：空候选 / 空白 ID / 重复 / 未知活动性 / 非有限对偶 / 负阈值 / 缺请求。
    // Refusals: no candidates, blank ID, duplicate, unknown activity, non-finite dual, negative threshold,
    // missing request.
    let mut empty = document.clone();
    empty.candidates = Vec::new();
    assert!(verify(empty).unwrap_err().message.contains("at least one candidate"));
    let mut blank = document.clone();
    blank.candidates[0].constraint_id = "  ".to_owned();
    assert!(verify(blank).unwrap_err().message.contains("constraint IDs must not be blank"));
    let mut duplicate = document.clone();
    duplicate.candidates[1].constraint_id = "c.hot".to_owned();
    assert!(verify(duplicate).unwrap_err().message.contains("repeats a constraint"));
    let mut unknown = document.clone();
    unknown.candidates[0].activity = "resting".to_owned();
    assert!(verify(unknown).unwrap_err().message.contains("unknown activity"));
    let mut infinite = document.clone();
    infinite.candidates[0].dual_value = Some(f64::INFINITY);
    assert!(verify(infinite).unwrap_err().message.contains("dual values must be finite"));
    let mut negative = document.clone();
    negative.dual_threshold = -1.0;
    assert!(verify(negative).unwrap_err().message.contains("must be finite and non-negative"));
    let mut missing = base_request();
    missing.model_kind = Some(ModelKind::TierClassificationVerify);
    assert_eq!(engine.submit(missing).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        let payload = serde_json::to_string(&document).unwrap();
        std::fs::write(directory.join("tier-classification.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn polarity_encoding_entry_shows_the_negated_bits_and_the_bound_rhs() {
    use ospf_rust_bridge::{BinaryVariableSpec, IntegerVariableSpec, PolarityReport,
        PolarityRequest};

    let engine = NativeEngine::default();
    let verify = |document: PolarityRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::PolarityEncodingVerify);
        request.polarity_encoding = Some(document);
        engine.submit(request)
    };
    let run = |document: PolarityRequest| {
        let view = verify(document).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: PolarityReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        engine.release(&view.id).unwrap();
        report
    };
    let document = PolarityRequest {
        binary: vec![
            BinaryVariableSpec { id: "b.a".to_owned(), value: 1 },
            BinaryVariableSpec { id: "b.b".to_owned(), value: 0 },
            BinaryVariableSpec { id: "b.c".to_owned(), value: 1 },
        ],
        integers: vec![IntegerVariableSpec { id: "i.x".to_owned(), lower: 0, upper: 4,
            value: 2 }],
        proof_reference: "probe/proof".to_owned(),
    };

    let report = run(document.clone());
    // 系数是取反的位，rhs = 1 - 2 = -1。 / Coefficients are the negated bits; the rhs is 1 - 2.
    let by = |variable: &str| report.no_good.terms.iter()
        .find(|term| term.variable == variable).map(|term| term.coefficient).unwrap();
    assert_eq!(by("b.a"), -1.0);
    assert_eq!(by("b.b"), 1.0);
    assert_eq!(by("b.c"), -1.0);
    assert_eq!(report.no_good.rhs, -1.0);
    assert_eq!(report.no_good.sense, "greaterOrEqual");
    assert_eq!(report.ones, 2);
    assert_eq!(report.encoding_count, 3);
    // 边行右端项就是值域边界。 / A side row's rhs is the domain bound.
    assert_eq!(report.families[0].lower_side.rhs, 4.0);
    assert_eq!(report.families[0].upper_side.rhs, 0.0);
    assert_eq!(report.families[0].at_most_one.rhs, 1.0);
    assert_eq!(report.families[0].require_difference.rhs, 1.0);
    assert!(report.coefficients_are_the_negated_bits);
    assert!(report.rhs_is_one_minus_the_ones);
    assert!(report.all_ones_rhs_is_negative);
    assert!(report.id_is_a_pure_function_of_the_assignment);
    assert!(report.encoding_holds_every_variable);
    assert!(report.side_rows_carry_domain_bounds);
    assert!(report.at_most_one_is_a_pair);
    assert!(report.require_difference_spans_every_auxiliary);
    assert!(report.unit_rows_use_rhs_one);

    // 单变量、值为 0：系数 +1、rhs 1。 / One variable at 0: coefficient +1 and rhs 1.
    let mut plain = document.clone();
    plain.binary = vec![BinaryVariableSpec { id: "only".to_owned(), value: 0 }];
    plain.integers = Vec::new();
    let plain = run(plain);
    assert_eq!(plain.no_good.rhs, 1.0);
    assert_eq!(plain.no_good.terms[0].coefficient, 1.0);
    assert!(plain.families.is_empty());

    // 拒绝：空二进制 / 空白证明 / 重复 / 跨组重复 / 二元越界 / 域外 / 界颠倒 / 缺请求。
    // Refusals: no binary, blank proof, duplicate, cross-group duplicate, binary out of range, out of domain,
    // inverted bounds, missing request.
    let mut empty = document.clone();
    empty.binary = Vec::new();
    assert!(verify(empty).unwrap_err().message.contains("at least one binary variable"));
    let mut blank = document.clone();
    blank.proof_reference = "  ".to_owned();
    assert!(verify(blank).unwrap_err().message.contains("proof reference must not be blank"));
    let mut duplicate = document.clone();
    duplicate.binary[1].id = "b.a".to_owned();
    assert!(verify(duplicate).unwrap_err().message.contains("repeats a variable"));
    let mut across = document.clone();
    across.integers[0].id = "b.a".to_owned();
    assert!(verify(across).unwrap_err().message.contains("repeats a variable"));
    let mut bad_binary = document.clone();
    bad_binary.binary[0].value = 2;
    assert!(verify(bad_binary).is_err());
    let mut out_of_domain = document.clone();
    out_of_domain.integers[0].value = 9;
    assert!(verify(out_of_domain).is_err());
    let mut inverted = document.clone();
    inverted.integers[0].lower = 5;
    inverted.integers[0].upper = 1;
    assert!(verify(inverted).unwrap_err().message.contains("lower bound exceeds its upper bound"));
    let mut missing = base_request();
    missing.model_kind = Some(ModelKind::PolarityEncodingVerify);
    assert_eq!(engine.submit(missing).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        let payload = serde_json::to_string(&document).unwrap();
        std::fs::write(directory.join("polarity-encoding.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn quadratic_monomial_entry_shows_the_derived_kind_and_the_raw_order() {
    use ospf_rust_bridge::{MonomialReport, MonomialSpec};

    let engine = NativeEngine::default();
    let verify = |document: MonomialSpec| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::QuadraticMonomialVerify);
        request.quadratic_monomial = Some(document);
        engine.submit(request)
    };
    let run = |document: MonomialSpec| {
        let view = verify(document).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: MonomialReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        engine.release(&view.id).unwrap();
        report
    };
    let document = MonomialSpec { coefficient: 2.5, first: 5, second: Some(2) };

    let report = run(document.clone());
    // kind 是从 `varIndex2` 推导的。 / The kind is derived from `varIndex2`.
    assert_eq!(report.kind, "quadratic");
    assert_eq!(report.first, 5);
    assert_eq!(report.second, Some(2));
    assert_eq!(report.coefficient, 2.5);
    // 序号不给排序：`(5, 2)` 原样保留。 / Indices stay unsorted: `(5, 2)` survives.
    assert!(report.indices_are_not_normalised);
    assert!(report.kind_is_derived);
    assert!(report.linear_never_has_a_second_index);
    assert!(report.is_square_only_when_equal);
    assert!(report.squares_are_quadratic);
    assert!(report.from_linear_has_no_second_index);
    assert!(report.key_is_stable);
    assert!(report.clone_gets_a_fresh_key);

    // 线性项与平方项。 / A linear term and a square.
    let linear = run(MonomialSpec { coefficient: 1.0, first: 3, second: None });
    assert_eq!(linear.kind, "linear");
    assert_eq!(linear.second, None);
    let square = run(MonomialSpec { coefficient: 4.0, first: 7, second: Some(7) });
    assert_eq!(square.kind, "quadratic");
    assert_eq!(square.first, square.second.unwrap());

    // 拒绝：非有限系数 / 缺请求。 / Refusals: non-finite coefficient, missing request.
    assert!(verify(MonomialSpec { coefficient: f64::NAN, first: 1, second: None })
        .unwrap_err().message.contains("coefficient must be finite"));
    let mut missing = base_request();
    missing.model_kind = Some(ModelKind::QuadraticMonomialVerify);
    assert_eq!(engine.submit(missing).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        let payload = serde_json::to_string(&document).unwrap();
        std::fs::write(directory.join("quadratic-monomial.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn deferred_materialize_entry_shows_the_all_or_nothing_contract() {
    use ospf_rust_bridge::{MaterializeReport, MaterializeRequest, StructureSpec};

    let engine = NativeEngine::default();
    let verify = |document: MaterializeRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::DeferredMaterializeVerify);
        request.deferred_materialize = Some(document);
        engine.submit(request)
    };
    let run = |document: MaterializeRequest| {
        let view = verify(document).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: MaterializeReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        engine.release(&view.id).unwrap();
        report
    };
    let specs = |count: usize| (0..count).map(|index| StructureSpec {
        name: format!("s.{index}"), rows: 1, fail: false }).collect::<Vec<_>>();
    let document = MaterializeRequest { structures: specs(4), indices: vec![0, 2],
        repeat_after_success: false };

    let report = run(document.clone());
    // 只物化 [0, 2]，剩下的保持原顺序。 / Only [0, 2] is materialized; the rest keep their order.
    assert!(report.succeeded);
    assert_eq!(report.constraint_count, 2);
    assert_eq!(report.pending_count, 2);
    assert_eq!(report.remaining, vec!["s.1".to_owned(), "s.3".to_owned()]);
    assert!(report.untouched_structures_keep_their_order);
    assert!(report.traces.iter().all(|trace| trace.calls <= 1));
    assert!(report.one_structure_is_never_materialized_twice);
    assert!(report.failure_writes_nothing);
    assert!(report.repeat_is_a_noop);

    // 重复与乱序索引等价于去重后集合。 / Duplicate and unsorted indices equal the deduplicated set.
    let deduped = run(MaterializeRequest { structures: specs(4), indices: vec![2, 0, 2],
        repeat_after_success: false });
    assert_eq!(deduped.constraint_count, 2);
    assert!(deduped.indices_are_sorted_and_deduplicated);

    // 失败不写一行。 / A failure writes nothing.
    let mut failing = specs(4);
    failing[2].fail = true;
    let failed = run(MaterializeRequest { structures: failing, indices: vec![0, 2],
        repeat_after_success: false });
    assert!(!failed.succeeded);
    assert_eq!(failed.constraint_count, 0);
    assert_eq!(failed.pending_count, 4);
    assert!(failed.error.as_deref().unwrap_or_default().contains("refuses to materialize"));

    // 越界不产生副作用。 / Out of range has no side effects.
    let out_of_range = run(MaterializeRequest { structures: specs(4), indices: vec![9],
        repeat_after_success: false });
    assert!(!out_of_range.succeeded);
    assert_eq!(out_of_range.constraint_count, 0);
    assert!(out_of_range.error.as_deref().unwrap_or_default().contains("out of range"));
    assert!(out_of_range.out_of_range_is_refused);

    // 成功后全量再物化：剩下的也被展开。 / A full pass after success expands the rest too.
    let full = run(MaterializeRequest { structures: specs(4), indices: vec![0, 2],
        repeat_after_success: true });
    assert_eq!(full.constraint_count, 4);
    assert_eq!(full.pending_count, 0);
    assert!(full.repeat_is_a_noop);

    // 拒绝：空结构 / 空白名 / 重复名 / 缺请求。 / Refusals.
    assert!(verify(MaterializeRequest { structures: Vec::new(), indices: Vec::new(),
        repeat_after_success: false }).unwrap_err().message.contains("at least one structure"));
    assert!(verify(MaterializeRequest { structures: vec![StructureSpec { name: "  ".to_owned(),
        rows: 1, fail: false }], indices: vec![0], repeat_after_success: false })
        .unwrap_err().message.contains("structure names must not be blank"));
    assert!(verify(MaterializeRequest { structures: vec![StructureSpec { name: "x".to_owned(),
        rows: 1, fail: false }, StructureSpec { name: "x".to_owned(), rows: 1, fail: false }],
        indices: vec![0], repeat_after_success: false }).unwrap_err().message
        .contains("repeats a structure"));
    let mut missing = base_request();
    missing.model_kind = Some(ModelKind::DeferredMaterializeVerify);
    assert_eq!(engine.submit(missing).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        let payload = serde_json::to_string(&document).unwrap();
        std::fs::write(directory.join("deferred-materialize.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn backend_features_entry_shows_two_independent_layers() {
    use ospf_rust_bridge::{BackendFeaturesReport, BackendFeaturesRequest};

    let engine = NativeEngine::default();
    let verify = |enabled: &[&str]| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::BackendFeaturesVerify);
        request.backend_features = Some(BackendFeaturesRequest {
            enabled: enabled.iter().map(|name| (*name).to_owned()).collect() });
        engine.submit(request)
    };
    let run = |enabled: &[&str]| {
        let view = verify(enabled).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: BackendFeaturesReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        engine.release(&view.id).unwrap();
        report
    };

    // 只开方言：`anyEnabled()` 是 false——方言不启用后端。
    // Dialect only: `anyEnabled()` is false — a dialect never enables a backend.
    let orphans = run(&["sqlxPostgres", "dieselMysql"]);
    assert!(!orphans.any_enabled);
    assert!(orphans.enabled_families.is_empty());
    assert_eq!(orphans.orphans, vec!["sqlxPostgres".to_owned(), "dieselMysql".to_owned()]);
    assert!(orphans.dialect_does_not_enable_the_family);

    // 只开族：`anyEnabled()` 是 true，方言仍全关。 / Family only: `anyEnabled()` is true, dialects stay off.
    let families = run(&["sqlx"]);
    assert!(families.any_enabled);
    assert_eq!(families.enabled, vec!["sqlx".to_owned()]);
    assert!(families.family_does_not_imply_its_dialects);

    // 14 = 8 族 + 6 方言；`current()` 是编译期常量。 
    // Fourteen is eight families plus six dialects; `current()` is a compile-time constant.
    let empty = run(&[]);
    assert_eq!(empty.family_count, 8);
    assert_eq!(empty.dialect_count, 6);
    assert!(!empty.any_enabled);
    assert!(empty.some_families_have_no_dialects);
    assert!(empty.current_is_compile_time);
    // 默认构建下编译期快照全关。 / In the default build the compile-time snapshot is all off.
    assert!(empty.current.enabled.is_empty());
    assert!(!empty.current.any_enabled);

    // 规范顺序按字段顺序。 / Canonical order follows the field order.
    let ordered = run(&["redis", "sqlxSqlite", "sqlx"]);
    assert_eq!(ordered.enabled, vec!["sqlx".to_owned(), "sqlxSqlite".to_owned(),
        "redis".to_owned()]);
    assert_eq!(ordered.enabled_families, vec!["sqlx".to_owned(), "redis".to_owned()]);

    // 拒绝：未知名 / 重复 / 大小写 / 缺请求。 / Refusals.
    assert!(verify(&["mysql"]).unwrap_err().message.contains("unknown flag"));
    assert!(verify(&["sqlx", "sqlx"]).unwrap_err().message.contains("repeats"));
    assert!(verify(&["SeaOrm"]).is_err());
    let mut missing = base_request();
    missing.model_kind = Some(ModelKind::BackendFeaturesVerify);
    assert_eq!(engine.submit(missing).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        let payload = serde_json::to_string(&BackendFeaturesRequest {
            enabled: vec!["sqlx".to_owned(), "sqlxSqlite".to_owned(), "redis".to_owned()],
        }).unwrap();
        std::fs::write(directory.join("backend-features.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn canonical_monomial_entry_shows_zero_power_is_representable() {
    use ospf_rust_bridge::{CanonicalMonomialReport, CanonicalMonomialRequest, PowerEntry};

    let engine = NativeEngine::default();
    let verify = |document: CanonicalMonomialRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::CanonicalMonomialVerify);
        request.canonical_monomial = Some(document);
        engine.submit(request)
    };
    let run = |document: CanonicalMonomialRequest| {
        let view = verify(document).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: CanonicalMonomialReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        engine.release(&view.id).unwrap();
        report
    };
    let document = CanonicalMonomialRequest { coefficient: 2.5,
        powers: vec![PowerEntry { variable: 3, power: 2 },
            PowerEntry { variable: 1, power: 1 }], append_again: false };

    let report = run(document.clone());
    assert_eq!(report.coefficient, 2.5);
    assert_eq!(report.table_len, 2);
    assert_eq!(report.total_power, 3);
    // 表按变量序号排序回。 / The table comes back sorted.
    assert_eq!(report.powers[0].variable, 1);
    assert_eq!(report.powers[1].variable, 3);

    // 幂次 0 与"变量不出现"：读回相同、表示不同。 
    // Zero power and absent: same read-back, different representation.
    let zero = run(CanonicalMonomialRequest { coefficient: 1.0,
        powers: vec![PowerEntry { variable: 4, power: 0 }], append_again: false });
    assert_eq!(zero.table_len, 1);
    assert_eq!(zero.zero_power, 0);
    assert_eq!(zero.total_power, 0);
    let empty = run(CanonicalMonomialRequest { coefficient: 1.0, powers: Vec::new(),
        append_again: false });
    assert_eq!(empty.table_len, 0);
    assert_eq!(empty.absent_power, 0);
    assert_eq!(empty.total_power, 0);
    assert!(empty.absent_reads_as_zero);
    assert!(empty.zero_power_is_representable);

    // 负幂次原样通过。 / Negative powers pass through.
    let negative = run(CanonicalMonomialRequest { coefficient: 1.0,
        powers: vec![PowerEntry { variable: 9, power: -2 }], append_again: false });
    assert_eq!(negative.powers[0].power, -2);
    assert_eq!(negative.total_power, -2);
    assert!(negative.powers_are_signed_and_unvalidated);

    // 追加不合并、不约简。 / Appending neither merges nor simplifies.
    let appended = run(CanonicalMonomialRequest { append_again: true, ..document.clone() });
    assert_eq!(appended.appended_count, 2);
    assert_eq!(appended.appended_coefficient_sum, 5.0);
    assert_eq!(appended.appended_total_power, 6);
    assert!(appended.add_monomial_does_not_merge);
    assert!(appended.new_variable_is_a_single_unit_power);
    assert!(appended.total_power_is_a_sum);

    // 拒绝：非有限系数 / 同一变量两次 / 幂次项过多 / 缺请求。 / Refusals.
    assert!(verify(CanonicalMonomialRequest { coefficient: f64::NAN,
        powers: Vec::new(), append_again: false }).unwrap_err().message
        .contains("coefficient must be finite"));
    assert!(verify(CanonicalMonomialRequest { coefficient: 1.0,
        powers: vec![PowerEntry { variable: 2, power: 1 }, PowerEntry { variable: 2, power: 3 }],
        append_again: false }).unwrap_err().message.contains("repeats variable 2"));
    assert!(verify(CanonicalMonomialRequest { coefficient: 1.0,
        powers: (0..65).map(|index| PowerEntry { variable: index, power: 1 }).collect(),
        append_again: false }).unwrap_err().message.contains("at most 64 power entries"));
    let mut missing = base_request();
    missing.model_kind = Some(ModelKind::CanonicalMonomialVerify);
    assert_eq!(engine.submit(missing).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        let payload = serde_json::to_string(&document).unwrap();
        std::fs::write(directory.join("canonical-monomial.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn expression_key_entry_shows_the_bit_identity() {
    use ospf_rust_bridge::{ExpressionKeyReport, ExpressionKeyRequest, KeyTerm};

    let engine = NativeEngine::default();
    let verify = |document: ExpressionKeyRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::ExpressionKeyVerify);
        request.expression_key = Some(document);
        engine.submit(request)
    };
    let run = |document: ExpressionKeyRequest| {
        let view = verify(document).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: ExpressionKeyReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        engine.release(&view.id).unwrap();
        report
    };
    let document = ExpressionKeyRequest { sense: "le".to_owned(), rhs: 7.0,
        terms: vec![KeyTerm { variable: "x".to_owned(), coefficient: 1.5 },
            KeyTerm { variable: "y".to_owned(), coefficient: -2.0 }], compare_rhs: None };

    let report = run(document.clone());
    // 定宽十六进制。 / Fixed-width hex.
    assert_eq!(report.rhs_bits.len(), 16);
    assert_eq!(report.rhs_bits, format!("{:016x}", 7.0f64.to_bits()));
    assert!(report.bit_pattern_is_fixed_width);
    // 项顺序不影响 key。 / The term order does not matter.
    assert_eq!(report.key, report.reordered_key);
    assert!(report.order_does_not_matter);
    assert!(report.id_is_length_prefixed);
    // `0.0` 与 `-0.0` 的 key 不同。 / `0.0` and `-0.0` key differently.
    assert_ne!(report.positive_zero_key, report.negative_zero_key);
    assert!(report.zeros_differ_by_sign);
    assert!(report.nan_payloads_differ);

    // `compareRhs` 给 `-0.0` 时会换 key。 / A `-0.0` compare rhs changes the key.
    let compared = run(ExpressionKeyRequest { compare_rhs: Some(-0.0), ..document.clone() });
    assert_ne!(compared.compare_key, compared.key);
    // `+0.0` 与 `-0.0` 的 compare key 不同，这正是"位身份"。 
    // The `+0.0` and `-0.0` compare keys differ, which is what "bit identity" means.
    let same = run(ExpressionKeyRequest { compare_rhs: Some(0.0), ..document.clone() });
    assert_ne!(same.compare_key, compared.compare_key);
    assert!(same.compare_key.contains("rhs=0000000000000000;"));
    assert!(compared.compare_key.contains("rhs=8000000000000000;"));

    // 拒绝：未知方向 / 空白 ID / 重复 ID / 项过多 / 缺请求。 / Refusals.
    assert!(verify(ExpressionKeyRequest { sense: "lt".to_owned(), ..document.clone() })
        .unwrap_err().message.contains("unknown sense"));
    assert!(verify(ExpressionKeyRequest { terms: vec![KeyTerm { variable: "  ".to_owned(),
        coefficient: 1.0 }], ..document.clone() }).unwrap_err().message
        .contains("variable IDs must not be blank"));
    assert!(verify(ExpressionKeyRequest { terms: vec![KeyTerm { variable: "x".to_owned(),
        coefficient: 1.0 }, KeyTerm { variable: "x".to_owned(), coefficient: 2.0 }],
        ..document.clone() }).unwrap_err().message.contains("repeats variable"));
    assert!(verify(ExpressionKeyRequest { terms: (0..257).map(|index| KeyTerm {
        variable: format!("v{index}"), coefficient: 1.0 }).collect(), ..document.clone() })
        .unwrap_err().message.contains("at most 256 terms"));
    let mut missing = base_request();
    missing.model_kind = Some(ModelKind::ExpressionKeyVerify);
    assert_eq!(engine.submit(missing).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        let payload = serde_json::to_string(&document).unwrap();
        std::fs::write(directory.join("expression-key.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn encoding_label_entry_shows_the_label_is_metadata() {
    use ospf_rust_bridge::{EncodingAssignmentRow, EncodingLabelReport, EncodingLabelRequest,
        EncodingTerm};

    let engine = NativeEngine::default();
    let verify = |document: EncodingLabelRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::EncodingLabelVerify);
        request.encoding_label = Some(document);
        engine.submit(request)
    };
    let run = |document: EncodingLabelRequest| {
        let view = verify(document).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: EncodingLabelReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        engine.release(&view.id).unwrap();
        report
    };
    let terms = vec![EncodingTerm { variable: "x".to_owned(), coefficient: 1.0 }];
    let document = EncodingLabelRequest { encoding: Some("binaryNoGood".to_owned()),
        assignment: vec![EncodingAssignmentRow { variable: "x".to_owned(), value: 1 }],
        family_id: None, terms: terms.clone(), rhs: 2.0 };

    let report = run(document.clone());
    // `new` 恒给线性，声明才改标签。 / `new` always claims linear; only the claim changes the label.
    assert_eq!(report.new_label, "linear");
    assert_eq!(report.claimed_label, "binaryNoGood");
    assert!(report.new_always_claims_linear);
    // key 不读编码。 / The key ignores the encoding.
    assert!(report.keys_agree);
    // `==` 读编码（其它字段全同）。 / `==` sees the encoding (every other field matches).
    assert!(!report.cuts_are_equal);
    assert!(report.key_ignores_the_encoding);
    assert!(report.equality_sees_the_encoding);
    assert!(report.normalize_accepted);
    assert_eq!(report.normalized_label, "binaryNoGood");

    // 不自洽的有界声明照样过 `normalize()`，族 ID 原样保留。 
    // An inconsistent bounded claim still normalizes, and the family id is kept verbatim.
    let inconsistent = run(EncodingLabelRequest { encoding: Some("boundedIntegerNoGood".to_owned()),
        assignment: Vec::new(), family_id: Some("   ".to_owned()), terms: terms.clone(),
        rhs: 2.0 });
    assert!(inconsistent.normalize_accepted);
    assert_eq!(inconsistent.normalized_label, "boundedIntegerNoGood");
    assert_eq!(inconsistent.normalized_family_id.as_deref(), Some("   "));
    assert!(inconsistent.normalize_ignores_the_encoding);

    // 线性声明 → 两个 cut 相等。 / A linear claim makes the two cuts equal.
    let linear = run(EncodingLabelRequest { encoding: Some("linear".to_owned()),
        assignment: Vec::new(), family_id: None, terms: terms.clone(), rhs: 2.0 });
    assert_eq!(linear.claimed_label, "linear");
    assert!(linear.cuts_are_equal);

    // 拒绝：未知编码 / 空白变量 / 缺请求。 / Refusals.
    assert!(verify(EncodingLabelRequest { encoding: Some("quadraticNoGood".to_owned()),
        assignment: Vec::new(), family_id: None, terms: terms.clone(), rhs: 0.0 })
        .unwrap_err().message.contains("unknown encoding"));
    assert!(verify(EncodingLabelRequest { encoding: None, assignment: Vec::new(),
        family_id: None, terms: vec![EncodingTerm { variable: "  ".to_owned(),
            coefficient: 1.0 }], rhs: 0.0 }).unwrap_err().message
        .contains("variable IDs must not be blank"));
    let mut missing = base_request();
    missing.model_kind = Some(ModelKind::EncodingLabelVerify);
    assert_eq!(engine.submit(missing).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        let payload = serde_json::to_string(&document).unwrap();
        std::fs::write(directory.join("encoding-label.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn activity_bands_entry_shows_the_four_bands() {
    use ospf_rust_bridge::{ActivityBandReport, ActivityBandRequest};

    let engine = NativeEngine::default();
    let verify = |document: ActivityBandRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::ActivityBandsVerify);
        request.activity_bands = Some(document);
        engine.submit(request)
    };
    let run = |document: ActivityBandRequest| {
        let view = verify(document).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: ActivityBandReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        engine.release(&view.id).unwrap();
        report
    };
    let document = ActivityBandRequest { tolerance: 1e-7, nearly_active_tolerance: 1e-5,
        relative_tolerance: 0.0, slacks: vec![0.0, -1e-7, -1.5e-7, 1.5e-7, 1.0],
        scales: vec![0.0; 5] };

    let report = run(document.clone());
    // 恰好等于 -active 是 Active；再往下才 Violated。 / Exactly -active is Active; only further is Violated.
    assert_eq!(report.rows[0].status, "ACTIVE");
    assert_eq!(report.rows[1].status, "ACTIVE");
    assert_eq!(report.rows[2].status, "VIOLATED");
    assert_eq!(report.rows[3].status, "NEARLY_ACTIVE");
    assert_eq!(report.rows[4].status, "INACTIVE");
    assert!(report.accepted);
    assert!(report.order_decides_the_boundaries);

    // 相对容差把零尺度也抬高。 / The relative tolerance lifts zero scale too.
    let relative = run(ActivityBandRequest { relative_tolerance: 1e-3, ..document.clone() });
    assert_eq!(relative.rows[0].active_tolerance, 1e-3);
    assert_eq!(relative.rows[0].nearly_active_tolerance, 1e-3);
    assert!(relative.relative_tolerance_lifts_zero_scale);
    assert!(relative.nearly_is_lifted_to_active);

    // **按实测纠正**：非有限输入**过不了 JSON 报告**（`serde_json` 把 NaN/∞ 写成 `null`），
    // 所以入口直接拒绝；"非有限 → Unknown"这条规则仍由固定探针的字段暴露。
    // Measured correction: non-finite input **cannot cross a JSON report** (`serde_json` writes NaN/∞ as `null`), so the
    // entry refuses it; the "non-finite → Unknown" rule is still exposed through the fixed-probe field.
    assert!(verify(ActivityBandRequest { slacks: vec![f64::NAN], scales: vec![0.0],
        ..document.clone() }).unwrap_err().message.contains("must be finite"));
    assert!(report.non_finite_is_unknown);

    // `withTolerance` 按十倍放大。 / `withTolerance` scales by ten.
    let scaled = run(ActivityBandRequest { tolerance: 2.5e-6, ..document.clone() });
    assert_eq!(scaled.scaled_nearly, 2.5e-5);
    assert!(scaled.with_tolerance_scales_by_ten);

    // 非法配置被 `validate()` 拒绝，但判定仍给出行。 
    // An invalid configuration is refused by `validate()`, yet the rows are still produced.
    let invalid = run(ActivityBandRequest { nearly_active_tolerance: 0.0, ..document.clone() });
    assert!(!invalid.accepted);
    assert_eq!(invalid.rows.len(), 5);

    // 拒绝：slack 与 scale 条数不符 / 条数过多 / 缺请求。 / Refusals.
    assert!(verify(ActivityBandRequest { scales: vec![0.0; 4], ..document.clone() })
        .unwrap_err().message.contains("every slack needs exactly one scale"));
    assert!(verify(ActivityBandRequest { slacks: vec![0.0; 257], scales: vec![0.0; 257],
        ..document.clone() }).unwrap_err().message.contains("at most 256 slacks"));
    let mut missing = base_request();
    missing.model_kind = Some(ModelKind::ActivityBandsVerify);
    assert_eq!(engine.submit(missing).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        let payload = serde_json::to_string(&document).unwrap();
        std::fs::write(directory.join("activity-bands.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn evidence_predicates_entry_shows_none_means_three_things() {
    use ospf_rust_bridge::{EvidenceReport, EvidenceSpec, EvidenceSpecRequest};

    let engine = NativeEngine::default();
    let verify = |document: EvidenceSpecRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::EvidencePredicatesVerify);
        request.evidence_predicates = Some(document);
        engine.submit(request)
    };
    let run = |document: EvidenceSpecRequest| {
        let view = verify(document).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: EvidenceReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        engine.release(&view.id).unwrap();
        report
    };
    let blank = |kind: &str| EvidenceSpec { kind: kind.to_owned(), lhs: None, rhs: None,
        variable: None, side: None, value: None, bound: None, integer_lhs: None,
        integer_rhs: None, relation: None, satisfied: None, margin: None, reason: None };
    let document = EvidenceSpecRequest { evidences: vec![
        EvidenceSpec { lhs: Some(1.0), rhs: Some(2.0), ..blank("linear") },
        EvidenceSpec { satisfied: Some(true), margin: Some(0.5), ..blank("semantic") },
        EvidenceSpec { reason: Some("boom".to_owned()), ..blank("unknown") },
        EvidenceSpec { integer_lhs: Some(1), integer_rhs: Some(2),
            relation: Some("le".to_owned()), ..blank("integerComparison") },
    ] };

    let report = run(document.clone());
    // 线性：有指标、但答不出 satisfied。 / Linear: has the metric, cannot answer.
    assert!(report.rows[0].has_slack_metric);
    assert_eq!(report.rows[0].satisfied, None);
    // 带 margin 的语义：两个都答。 / Semantic with a margin answers both.
    assert!(report.rows[1].has_slack_metric);
    assert_eq!(report.rows[1].satisfied, Some(true));
    // Unknown：两个都不答。 / Unknown answers neither.
    assert!(!report.rows[2].has_slack_metric);
    assert_eq!(report.rows[2].satisfied, None);
    // 整数比较：有指标、答不出。 / Integer comparison: has the metric, cannot answer.
    assert!(report.rows[3].has_slack_metric);
    assert_eq!(report.rows[3].satisfied, None);
    assert_eq!(report.with_slack_metric, 3);
    assert_eq!(report.with_satisfied, 1);
    assert!(report.same_variant_two_answers);
    assert!(report.satisfied_is_none_for_three_meanings);
    assert!(report.integer_comparison_cannot_answer);
    assert!(report.unknown_reason_is_unchecked);

    // 没有 margin 的语义证据就丢了指标。 / A semantic evidence without a margin loses the metric.
    let mut without = document.clone();
    without.evidences[1].margin = None;
    let without = run(without);
    assert!(!without.rows[1].has_slack_metric);
    assert_eq!(without.rows[1].satisfied, Some(true));

    // 变量边界按侧构造。 / A variable bound is built per side.
    let bound = run(EvidenceSpecRequest { evidences: vec![EvidenceSpec {
        variable: Some("x".to_owned()), side: Some("upper".to_owned()), value: Some(3.0),
        bound: Some(5.0), ..blank("variableBound") }] });
    assert!(bound.rows[0].has_slack_metric);
    assert_eq!(bound.rows[0].satisfied, None);

    // 拒绝：空清单 / 未知种类 / 缺 rhs / 非法侧 / 条数过多 / 缺请求。 / Refusals.
    assert!(verify(EvidenceSpecRequest { evidences: Vec::new() }).unwrap_err().message
        .contains("at least one evidence"));
    assert!(verify(EvidenceSpecRequest { evidences: vec![blank("quadratic")] })
        .unwrap_err().message.contains("unknown kind"));
    assert!(verify(EvidenceSpecRequest { evidences: vec![EvidenceSpec { lhs: Some(1.0),
        ..blank("linear") }] }).unwrap_err().message.contains("linear rhs must be a finite number"));
    assert!(verify(EvidenceSpecRequest { evidences: vec![EvidenceSpec {
        variable: Some("x".to_owned()), side: Some("middle".to_owned()), value: Some(1.0),
        bound: Some(2.0), ..blank("variableBound") }] }).unwrap_err().message
        .contains("side must be"));
    assert!(verify(EvidenceSpecRequest { evidences: (0..257).map(|_| blank("unknown"))
        .collect() }).unwrap_err().message.contains("at most 256 evidences"));
    let mut missing = base_request();
    missing.model_kind = Some(ModelKind::EvidencePredicatesVerify);
    assert_eq!(engine.submit(missing).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        let payload = serde_json::to_string(&document).unwrap();
        std::fs::write(directory.join("evidence-predicates.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn activity_report_entry_shows_where_validation_stops() {
    use ospf_rust_bridge::{ActivityReportRequest, ActivityReportVerdict, ConstraintRecordSpec,
        StatusCountSpec};

    let engine = NativeEngine::default();
    let verify = |document: ActivityReportRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::ActivityReportCheckVerify);
        request.activity_report_check = Some(document);
        engine.submit(request)
    };
    let run = |document: ActivityReportRequest| {
        let view = verify(document).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let verdict: ActivityReportVerdict =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        engine.release(&view.id).unwrap();
        verdict
    };
    let record = |id: &str, group: Option<&str>, status: &str, slack: f64| ConstraintRecordSpec {
        id: id.to_owned(), group: group.map(|group| group.to_owned()),
        status: status.to_owned(), slack: Some(slack),
        normalized_slack: Some(slack), tolerance: Some(1e-7), satisfied: Some(true) };
    let document = ActivityReportRequest { schema_version: Some("activity/v1".to_owned()),
        tolerance: Some(1e-7), nearly_active_tolerance: Some(1e-5),
        constraints: vec![record("c1", Some("g"), "INACTIVE", 1.0),
            record("c2", None, "ACTIVE", 0.0)],
        variable_bounds: Vec::new(), summary: None, variable_bound_summary: None, groups: None };

    let consistent = run(document.clone());
    assert!(consistent.accepted, "{:?}", consistent.error);
    assert!(consistent.aggregates_are_recomputed);
    assert!(consistent.duplicate_groups_are_refused);
    assert!(consistent.numbers_only_check_finiteness);
    assert!(consistent.status_is_not_cross_checked);
    assert!(consistent.satisfied_is_not_cross_checked);
    assert!(consistent.identities_are_checked);

    // 改错一位计数 → 拒绝。 / One wrong count is refused.
    let wrong = run(ActivityReportRequest { summary: Some(vec![
        StatusCountSpec { status: "INACTIVE".to_owned(), count: 2 }]),
        ..document.clone() });
    assert!(!wrong.accepted);
    assert!(wrong.error.as_deref().unwrap_or_default()
        .contains("summary does not match its records"));

    // 数值与状态怎么不一致都过。 / Numbers may disagree with the status in any way.
    let inconsistent = run(ActivityReportRequest { constraints: vec![
        ConstraintRecordSpec { slack: Some(5.0), normalized_slack: Some(-1234.5),
            tolerance: Some(0.0), satisfied: Some(false),
            ..record("c1", Some("g"), "INACTIVE", 5.0) },
        record("c2", None, "ACTIVE", 0.0)], ..document.clone() });
    assert!(inconsistent.accepted, "{:?}", inconsistent.error);

    // 重复 ID 被拒。 / A duplicate ID is refused.
    let duplicated = run(ActivityReportRequest { constraints: vec![
        record("c1", Some("g"), "INACTIVE", 1.0), record("c1", None, "ACTIVE", 0.0)],
        ..document.clone() });
    assert!(!duplicated.accepted);
    assert!(duplicated.error.as_deref().unwrap_or_default()
        .contains("duplicate constraint IDs"));

    // 空白组被拒。 / A blank group is refused.
    let blank_group = run(ActivityReportRequest { constraints: vec![
        record("c1", Some("   "), "ACTIVE", 0.0)], ..document.clone() });
    assert!(!blank_group.accepted);
    assert!(blank_group.error.as_deref().unwrap_or_default()
        .contains("group must not be blank"));

    // 空白 schema 被拒。 / A blank schema version is refused.
    let blank_schema = run(ActivityReportRequest {
        schema_version: Some("  ".to_owned()), ..document.clone() });
    assert!(!blank_schema.accepted);

    // 拒绝：未知状态 / 未知侧 / 缺请求。 / Refusals.
    assert!(verify(ActivityReportRequest { constraints: vec![
        record("c1", None, "active", 0.0)], ..document.clone() }).unwrap_err().message
        .contains("unknown status"));
    let mut missing = base_request();
    missing.model_kind = Some(ModelKind::ActivityReportCheckVerify);
    assert_eq!(engine.submit(missing).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        let payload = serde_json::to_string(&document).unwrap();
        std::fs::write(directory.join("activity-report.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn auxiliary_domain_entry_shows_the_two_to_the_53_ceiling() {
    use ospf_rust_bridge::{AuxiliaryDomainReport, AuxiliaryDomainRequest, AuxiliarySpec};

    let engine = NativeEngine::default();
    let verify = |document: AuxiliaryDomainRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::AuxiliaryDomainVerify);
        request.auxiliary_domain = Some(document);
        engine.submit(request)
    };
    let run = |document: AuxiliaryDomainRequest| {
        let view = verify(document).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: AuxiliaryDomainReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        engine.release(&view.id).unwrap();
        report
    };
    let limit: i64 = 1_i64 << 53;
    let integer = |id: &str, lower: i64, upper: i64| AuxiliarySpec { id: id.to_owned(),
        kind: Some("integer".to_owned()), lower: Some(lower), upper: Some(upper) };
    let binary = |id: &str| AuxiliarySpec { id: id.to_owned(),
        kind: Some("binary".to_owned()), lower: None, upper: None };
    let document = AuxiliaryDomainRequest { auxiliaries: vec![integer("ok", 0, 3),
        binary("bin")], together: false };

    let report = run(document.clone());
    assert!(report.outcomes.iter().all(|outcome| outcome.accepted));
    assert_eq!(report.kept_count, 2);
    assert_eq!(report.kept_ids, vec!["ok".to_owned(), "bin".to_owned()]);
    assert!(report.two_to_the_53_passes);
    assert!(report.beyond_the_exact_range_is_refused);
    assert!(report.inverted_bounds_are_refused);
    assert!(report.binary_is_unchecked);
    assert!(report.blank_ids_are_refused);
    assert!(report.duplicate_ids_are_refused);
    assert!(report.normalize_keeps_the_declarations);

    // 恰好 2^53 放行，2^53 + 1 被拒。 / Exactly 2^53 passes, 2^53 + 1 is refused.
    let at_limit = run(AuxiliaryDomainRequest { auxiliaries: vec![integer("a", 0, limit)],
        together: false });
    assert!(at_limit.outcomes[0].accepted);
    let beyond = run(AuxiliaryDomainRequest { auxiliaries: vec![integer("a", 0, limit + 1)],
        together: false });
    assert!(!beyond.outcomes[0].accepted);
    assert!(beyond.outcomes[0].error.as_deref().unwrap_or_default()
        .contains("exceeds the exact f64 integer range"));

    // 界颠倒被拒，消息带上下界原值。 / Inverted bounds are refused with both values quoted.
    let inverted = run(AuxiliaryDomainRequest { auxiliaries: vec![integer("a", 5, 1)],
        together: false });
    assert!(!inverted.outcomes[0].accepted);
    assert!(inverted.outcomes[0].error.as_deref().unwrap_or_default()
        .contains("lower 5 above upper 1"));

    // 空白 ID 与重复 ID 是两条不同报错。 / A blank ID and a duplicate ID are two different errors.
    let blank = run(AuxiliaryDomainRequest { auxiliaries: vec![binary("   ")], together: false });
    assert!(blank.outcomes[0].error.as_deref().unwrap_or_default()
        .contains("ID must not be empty"));
    let duplicated = run(AuxiliaryDomainRequest { auxiliaries: vec![binary("dup"), binary("dup")],
        together: true });
    assert!(duplicated.outcomes.iter().all(|outcome| outcome.accepted));
    assert_eq!(duplicated.combined_accepted, Some(false));
    assert!(duplicated.combined_error.as_deref().unwrap_or_default()
        .contains("duplicate auxiliary variable"));

    // 拒绝：缺上界 / 未知域 / 条数过多 / 缺请求。 / Refusals.
    assert!(verify(AuxiliaryDomainRequest { auxiliaries: vec![AuxiliarySpec {
        id: "a".to_owned(), kind: Some("integer".to_owned()), lower: Some(1), upper: None }],
        together: false }).unwrap_err().message.contains("needs an upper bound"));
    assert!(verify(AuxiliaryDomainRequest { auxiliaries: vec![AuxiliarySpec {
        id: "a".to_owned(), kind: Some("real".to_owned()), lower: None, upper: None }],
        together: false }).unwrap_err().message.contains("unknown domain"));
    assert!(verify(AuxiliaryDomainRequest { auxiliaries: (0..65)
        .map(|index| binary(&format!("a{index}"))).collect(), together: false })
        .unwrap_err().message.contains("at most 64 declarations"));
    let mut missing = base_request();
    missing.model_kind = Some(ModelKind::AuxiliaryDomainVerify);
    assert_eq!(engine.submit(missing).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        let payload = serde_json::to_string(&document).unwrap();
        std::fs::write(directory.join("auxiliary-domain.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn binding_assignment_entry_shows_the_exact_coverage_contract() {
    use ospf_rust_bridge::{BindingAssignmentReport, BindingAssignmentRequest, BindingLiteral};

    let engine = NativeEngine::default();
    let verify = |document: BindingAssignmentRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::BindingAssignmentVerify);
        request.binding_assignment = Some(document);
        engine.submit(request)
    };
    let run = |document: BindingAssignmentRequest| {
        let view = verify(document).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: BindingAssignmentReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        engine.release(&view.id).unwrap();
        report
    };
    let document = BindingAssignmentRequest { bindings: vec![
        BindingLiteral { id: "a".to_owned(), kind: Some("binary".to_owned()), lower: None,
            upper: None, value: Some(1) },
        BindingLiteral { id: "b".to_owned(), kind: Some("integer".to_owned()), lower: Some(0),
            upper: Some(3), value: Some(2) },
    ], extra: None, drop_one: false };

    let report = run(document.clone());
    assert!(report.binding_accepted, "{:?}", report.binding_error);
    assert_eq!(report.binding_count, 2);
    assert_eq!(report.binding_ids, vec!["a".to_owned(), "b".to_owned()]);
    assert_eq!(report.domains, vec!["binary".to_owned(), "integer:0..3".to_owned()]);
    assert!(report.assignment_accepted, "{:?}", report.assignment_error);
    assert!(report.empty_binding_is_refused);
    assert!(report.duplicate_is_refused_in_new);
    assert!(report.new_validates_the_domains);
    assert!(report.exact_coverage_is_required);
    assert!(report.out_of_domain_differs_from_extra);
    assert!(report.ids_are_lexicographic);

    // 少给与多给是同一个错误。 / Too few and too many are the same error.
    let short = run(BindingAssignmentRequest { drop_one: true, ..document.clone() });
    assert!(short.short_error.as_deref().unwrap_or_default()
        .contains("does not match the stable binding set"));
    let extra = run(BindingAssignmentRequest { extra: Some(BindingLiteral {
        id: "c".to_owned(), kind: Some("binary".to_owned()), lower: None, upper: None,
        value: Some(0) }), ..document.clone() });
    assert!(extra.extra_error.as_deref().unwrap_or_default()
        .contains("does not match the stable binding set"));
    assert_eq!(short.short_error, extra.extra_error);

    // 越界是另一条报错。 / Out of domain is a different error.
    let mut out_of_domain = document.clone();
    out_of_domain.bindings[0].value = Some(2);
    let report = run(out_of_domain);
    assert!(report.assignment_error.as_deref().unwrap_or_default()
        .contains("is outside its domain"));

    // 绑定集合建不起来时，赋值也建不起来。 
    // When the binding set cannot be built, neither can the assignment.
    let empty = run(BindingAssignmentRequest { bindings: Vec::new(), extra: None,
        drop_one: false });
    assert!(!empty.binding_accepted);
    assert!(empty.binding_error.as_deref().unwrap_or_default()
        .contains("at least one master binding"));
    assert_eq!(empty.binding_count, 0);
    let mut inverted = document.clone();
    inverted.bindings[1].lower = Some(5);
    inverted.bindings[1].upper = Some(1);
    assert!(run(inverted).binding_error.as_deref().unwrap_or_default()
        .contains("lower 5 above upper 1"));
    let mut duplicated = document.clone();
    duplicated.bindings[1].id = "a".to_owned();
    assert!(run(duplicated).binding_error.as_deref().unwrap_or_default()
        .contains("duplicate master binding ID"));

    // 拒绝：缺上界 / 未知域 / 条数过多 / 缺请求。 / Refusals.
    let mut missing = document.clone();
    missing.bindings[1].upper = None;
    assert!(verify(missing).unwrap_err().message.contains("needs an upper bound"));
    let mut unknown = document.clone();
    unknown.bindings[0].kind = Some("real".to_owned());
    assert!(verify(unknown).unwrap_err().message.contains("unknown domain"));
    let too_many = BindingAssignmentRequest { bindings: (0..65).map(|index| BindingLiteral {
        id: format!("v{index}"), kind: Some("binary".to_owned()), lower: None, upper: None,
        value: Some(0) }).collect(), extra: None, drop_one: false };
    assert!(verify(too_many).unwrap_err().message.contains("at most 64 bindings"));
    let mut missing_request = base_request();
    missing_request.model_kind = Some(ModelKind::BindingAssignmentVerify);
    assert_eq!(engine.submit(missing_request).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        let payload = serde_json::to_string(&document).unwrap();
        std::fs::write(directory.join("binding-assignment.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn source_identity_entry_shows_the_merged_bound_ids() {
    use ospf_rust_bridge::{SourceIdentityReport, SourceIdentityRequest, SourceSpec};

    let engine = NativeEngine::default();
    let verify = |document: SourceIdentityRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::SourceIdentityVerify);
        request.source_identity = Some(document);
        engine.submit(request)
    };
    let run = |document: SourceIdentityRequest| {
        let view = verify(document).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: SourceIdentityReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        engine.release(&view.id).unwrap();
        report
    };
    let spec = |kind: &str, id: Option<&str>| SourceSpec { kind: kind.to_owned(),
        id: id.map(|id| id.to_owned()), side: None, objective: None, relation: None,
        value: None };
    let document = SourceIdentityRequest { sources: vec![spec("constraint", Some("capacity")),
        spec("variableLowerBound", Some("x")), spec("variableUpperBound", Some("x")),
        spec("variableBound", Some("x")), spec("sparseDomain", Some("x"))] };

    let report = run(document.clone());
    assert_eq!(report.kinds, vec!["constraint".to_owned(),
        "variable-lower-bound".to_owned(), "variable-upper-bound".to_owned(),
        "variable-bound".to_owned(), "sparse-domain".to_owned(), "objective-target".to_owned()]);
    assert_eq!(report.sources[0].kind.as_deref(), Some("constraint"));
    assert_eq!(report.sources[0].stable_id.as_deref(), Some("constraint:capacity"));
    // 下界与带侧下界共享同一个 ID，但 `kind` 不同。 
    // The lower bound and the sided lower bound share an ID yet differ in `kind`.
    assert_eq!(report.sources[1].stable_id, report.sources[3].stable_id);
    assert_ne!(report.sources[1].kind, report.sources[3].kind);
    assert_eq!(report.sources[4].stable_id.as_deref(), Some("variable:x:domain"));
    assert!(report.sources.iter().all(|source| source.member == Some(true)));
    assert!(report.stable_id_merges_lower_and_bound);
    assert!(report.the_kind_completes_the_id);
    assert!(report.blank_ids_are_refused);
    assert!(report.target_ids_use_canonical_floats);
    assert!(report.the_target_is_a_separate_namespace);
    assert!(report.the_target_is_not_a_member);

    // 目标：独立命名空间、不是证据成员。 / A target has its own namespace and is not an evidence member.
    let target = run(SourceIdentityRequest { sources: vec![SourceSpec {
        kind: "objectiveTarget".to_owned(), id: None, side: None,
        objective: Some("cost".to_owned()), relation: Some("at-most".to_owned()),
        value: Some(12.5) }] });
    assert!(target.sources[0].stable_id.as_deref().unwrap_or_default()
        .starts_with("objective-target:cost:at-most:12.5"));
    assert_eq!(target.sources[0].member, Some(false));

    // 空白 ID 被记录，不打断整批。 / Blank IDs are recorded without aborting the batch.
    let blank = run(SourceIdentityRequest { sources: vec![spec("constraint", Some("  ")),
        spec("sparseDomain", Some("  ")), spec("sparseDomain", Some("ok"))] });
    assert!(blank.sources[0].error.as_deref().unwrap_or_default()
        .contains("constraint ID must not be blank"));
    assert!(blank.sources[1].error.as_deref().unwrap_or_default()
        .contains("variable ID must not be blank"));
    assert_eq!(blank.sources[1].kind, None);
    assert_eq!(blank.sources[2].stable_id.as_deref(), Some("variable:ok:domain"));

    // 拒绝：未知 kind / 未知侧 / 未知关系 / 缺值 / 条数过多 / 缺请求。 
    // Refusals: unknown kind, unknown side, unknown relation, missing value, too many, missing request.
    let cases = vec![spec("observation", Some("x")),
        SourceSpec { side: Some("middle".to_owned()), ..spec("variableBound", Some("x")) },
        SourceSpec { relation: Some("around".to_owned()), value: Some(1.0),
            objective: Some("obj".to_owned()), ..spec("objectiveTarget", None) },
        SourceSpec { objective: Some("obj".to_owned()), ..spec("objectiveTarget", None) }];
    for case in cases {
        let report = run(SourceIdentityRequest { sources: vec![case] });
        assert!(report.sources[0].error.is_some());
    }
    assert!(verify(SourceIdentityRequest { sources: (0..65)
        .map(|index| spec("sparseDomain", Some(&format!("v{index}")))).collect() })
        .unwrap_err().message.contains("at most 64 sources"));
    let mut missing = base_request();
    missing.model_kind = Some(ModelKind::SourceIdentityVerify);
    assert_eq!(engine.submit(missing).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        let payload = serde_json::to_string(&document).unwrap();
        std::fs::write(directory.join("source-identity.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn cut_validity_entry_shows_that_local_is_never_refused() {
    use ospf_rust_bridge::{CutValidityOutcome, CutValidityReport, CutValidityRequest,
        CutValiditySpec};

    let engine = NativeEngine::default();
    let verify = |document: CutValidityRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::CutValidityVerify);
        request.cut_validity = Some(document);
        engine.submit(request)
    };
    let run = |document: CutValidityRequest| {
        let view = verify(document).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: CutValidityReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        engine.release(&view.id).unwrap();
        report
    };
    let spec = |id: &str, validity: &str, status: &str, reference: Option<&str>| CutValiditySpec {
        id: id.to_owned(), validity: Some(validity.to_owned()),
        proof_status: Some(status.to_owned()),
        proof_reference: reference.map(|reference| reference.to_owned()), rhs: Some(1.0) };
    let document = CutValidityRequest { cuts: vec![spec("cut", "global", "verified",
        Some("proof-1"))] };

    let report = run(document.clone());
    assert!(report.cuts[0].accepted);
    assert_eq!(report.cuts[0].validity.as_deref(), Some("global"));
    assert_eq!(report.cuts[0].proof_status.as_deref(), Some("verified"));
    assert_eq!(report.cuts[0].proof_reference.as_deref(), Some("proof-1"));
    assert!(report.cuts[0].expression_key.is_some());
    assert!(report.local_is_never_refused);
    assert!(report.normalize_keeps_them);
    assert!(report.the_key_ignores_them);
    assert!(report.equality_sees_them);
    assert!(report.verified_global_pins_three_fields);
    assert!(report.verified_requires_a_reference);

    // `Local` 照样建成；只有效性与状态不同的两条：key 相同、`==` 不同。 
    // A `Local` cut still builds; two cuts differing only in validity and status share the key but not `==`.
    let pair = run(CutValidityRequest { cuts: vec![spec("cut", "global", "verified",
        Some("proof-1")), spec("cut", "local", "claimed", Some("proof-1"))] });
    assert_eq!(pair.cuts[1].validity.as_deref(), Some("local"));
    assert_eq!(pair.same_expression_key, Some(true));
    assert_eq!(pair.equal, Some(false));

    // `Verified` 必须有非空引用，内容不校验。 
    // `Verified` needs a non-blank reference; its content is unchecked.
    let missing = run(CutValidityRequest { cuts: vec![spec("cut", "global", "verified", None)] });
    assert!(!missing.cuts[0].accepted);
    assert!(missing.cuts[0].error.as_deref().unwrap_or_default()
        .contains("must carry a proof reference"));
    let trivial = run(CutValidityRequest { cuts: vec![spec("cut", "global", "verified",
        Some("x"))] });
    assert!(trivial.cuts[0].accepted);
    let blank = run(CutValidityRequest { cuts: vec![spec("cut", "global", "claimed",
        Some("   "))] });
    assert_eq!(blank.cuts[0].proof_reference, None);

    // 拒绝：未知有效性 / 未知证明状态 / 条数过多 / 缺请求。 
    // Refusals: unknown validity, unknown status, too many, missing request.
    assert!(verify(CutValidityRequest { cuts: vec![spec("cut", "branch", "none", None)] })
        .unwrap_err().message.contains("unknown validity"));
    assert!(verify(CutValidityRequest { cuts: vec![spec("cut", "global", "maybe", None)] })
        .unwrap_err().message.contains("unknown proof status"));
    assert!(verify(CutValidityRequest { cuts: (0..17)
        .map(|index| spec(&format!("c{index}"), "global", "none", None)).collect() })
        .unwrap_err().message.contains("at most 16 cuts"));
    let mut missing_request = base_request();
    missing_request.model_kind = Some(ModelKind::CutValidityVerify);
    assert_eq!(engine.submit(missing_request).unwrap_err().code, "InvalidInput");

    // 空白 ID 由 `normalize()` 拒，逐条记录。 / A blank ID is refused by `normalize()` and recorded per cut.
    let blank_id = run(CutValidityRequest { cuts: vec![spec("  ", "global", "none", None)] });
    assert!(!blank_id.cuts[0].accepted);
    assert!(blank_id.cuts[0].error.as_deref().unwrap_or_default()
        .contains("ID must not be empty"));

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        let payload = serde_json::to_string(&document).unwrap();
        std::fs::write(directory.join("cut-validity.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn assignment_composition_entry_shows_the_three_path_strengths() {
    use ospf_rust_bridge::{AssignLiteral, AssignStep, AssignmentCompositionReport,
        AssignmentCompositionRequest};

    let engine = NativeEngine::default();
    let verify = |document: AssignmentCompositionRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::AssignmentCompositionVerify);
        request.assignment_composition = Some(document);
        engine.submit(request)
    };
    let run = |document: AssignmentCompositionRequest| {
        let view = verify(document).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: AssignmentCompositionReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        engine.release(&view.id).unwrap();
        report
    };
    let step = |path: &str, kind: &str, value: Option<AssignLiteral>| AssignStep {
        path: path.to_owned(), kind: Some(kind.to_owned()), value };
    let document = AssignmentCompositionRequest {
        steps: vec![step("status", "value", Some(AssignLiteral::Text("inactive".to_owned()))),
            step("deletedAt", "null", None),
            step("score", "expression", Some(AssignLiteral::Number(1.0)))],
        paths: vec!["  na me  ".to_owned(), "  ok.name  ".to_owned(), "   ".to_owned()],
        segments: vec!["a".to_owned(), String::new(), "b".to_owned()] };

    let report = run(document.clone());
    assert_eq!(report.items.len(), 3);
    assert_eq!(report.items[0].variant, "set-value");
    assert_eq!(report.items[0].value_kind, "string");
    assert_eq!(report.items[1].variant, "set-null");
    assert_eq!(report.items[1].value_kind, "none");
    assert_eq!(report.items[2].variant, "set-from-expression");
    assert!(!report.empty);
    assert!(report.empty_after_then);
    // 三种路径校验强度。 / Three path validation strengths.
    assert_eq!(report.paths[0].from_new, "  na me  ");
    assert_eq!(report.paths[0].from_parse, "na me");
    assert!(!report.paths[0].parsed);
    assert!(report.paths[1].parsed);
    assert_eq!(report.paths[1].segments, vec!["ok".to_owned(), "name".to_owned()]);
    assert!(report.paths[2].segments.is_empty());
    assert_eq!(report.of_path, "a..b");
    assert_eq!(report.of_segments, vec!["a".to_owned(), String::new(), "b".to_owned()]);
    assert!(report.the_order_is_fifo_and_add_matches_then);
    assert!(report.the_same_path_is_not_merged);
    assert!(report.null_value_and_set_null_differ);
    assert!(report.three_validation_strengths);
    assert!(report.of_does_not_validate_segments);
    assert!(report.a_blank_path_has_no_segments);
    assert!(report.identifiers_are_unicode_aware);

    // 同一路径保留两条；`set-null` 与 `set(_, null)` 是不同变体。 
    // The same path stays twice; `set-null` and `set(_, null)` are different variants.
    let pair = run(AssignmentCompositionRequest {
        steps: vec![step("x", "null", None), step("x", "value", Some(AssignLiteral::Null))],
        paths: Vec::new(), segments: Vec::new() });
    assert_eq!(pair.items.len(), 2);
    assert_eq!(pair.items[0].variant, "set-null");
    assert_eq!(pair.items[1].variant, "set-value");
    assert_eq!(pair.items[1].value_kind, "null");

    // 空步骤集。 / An empty step list.
    let empty = run(AssignmentCompositionRequest { steps: Vec::new(), paths: Vec::new(),
        segments: Vec::new() });
    assert!(empty.empty);
    assert!(empty.items.is_empty());

    // 拒绝：未知步骤种类 / 条数过多 / 缺请求。 / Refusals: unknown step kind, too many, missing request.
    assert!(verify(AssignmentCompositionRequest { steps: vec![step("x", "merge", None)],
        paths: Vec::new(), segments: Vec::new() }).unwrap_err().message
        .contains("unknown step kind"));
    assert!(verify(AssignmentCompositionRequest {
        steps: (0..65).map(|index| step(&format!("p{index}"), "null", None)).collect(),
        paths: Vec::new(), segments: Vec::new() }).unwrap_err().message
        .contains("at most 64 entries"));
    let mut missing = base_request();
    missing.model_kind = Some(ModelKind::AssignmentCompositionVerify);
    assert_eq!(engine.submit(missing).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        let payload = serde_json::to_string(&document).unwrap();
        std::fs::write(directory.join("assignment-composition.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn dump_options_entry_shows_the_independent_bound_switches() {
    use ospf_rust_bridge::{DumpOptionsReport, DumpOptionsSpec, FixedVariableSpec};

    let engine = NativeEngine::default();
    let verify = |document: DumpOptionsSpec| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::DumpOptionsVerify);
        request.dump_options = Some(document);
        engine.submit(request)
    };
    let run = |document: DumpOptionsSpec| {
        let view = verify(document).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: DumpOptionsReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        engine.release(&view.id).unwrap();
        report
    };
    let fixed = |index: u32, value: f64| FixedVariableSpec { index, value };
    let document = DumpOptionsSpec { concurrent: Some(true), include_bounds: Some(false),
        force_bounds: Some(true), fixed_variables: vec![fixed(3, 1.5), fixed(3, 2.5)],
        replaced_variables: None, clear_variables: false };

    let report = run(document.clone());
    // 默认值：`concurrent: false`、`include_bounds: true`、`force_bounds: true`。 
    // The defaults: `concurrent: false`, `include_bounds: true`, `force_bounds: true`.
    assert!(!report.defaults.concurrent);
    assert!(report.defaults.include_bounds);
    assert!(report.defaults.force_bounds);
    assert_eq!(report.defaults.fixed_count, 0);
    // 矛盾组合被允许。 / The contradictory pair is allowed.
    assert!(!report.include_bounds);
    assert!(report.force_bounds);
    // 同 index 后来居上。 / The same index is last-write-wins.
    assert_eq!(report.fixed_count, 1);
    assert_eq!(report.fixed_indices, vec![3]);
    assert_eq!(report.fixed_values, vec![2.5]);
    assert_eq!(report.extensions, vec!["lp".to_owned(), "opm".to_owned()]);
    assert!(report.the_defaults_are_measured);
    assert!(report.the_two_bound_switches_are_independent);
    assert!(report.fixed_variable_inserts_are_last_write_wins);
    assert!(report.with_fixed_variables_replaces_the_whole_map);
    assert!(report.without_fixed_variables_clears_only_the_map);
    assert!(report.the_extensions_are_lowercase);
    assert!(report.new_equals_default);

    // 整表替换丢掉先前那条。 / Replacing the map drops the earlier entry.
    let replaced = run(DumpOptionsSpec { fixed_variables: vec![fixed(1, 1.0)],
        replaced_variables: Some(vec![fixed(2, 2.0), fixed(4, 4.0)]), ..document.clone() });
    assert_eq!(replaced.fixed_indices, vec![2, 4]);
    // 清空只清 map。 / Clearing only touches the map.
    let cleared = run(DumpOptionsSpec { clear_variables: true, ..document.clone() });
    assert_eq!(cleared.fixed_count, 0);
    assert!(cleared.concurrent);
    assert!(!cleared.include_bounds);
    assert!(cleared.force_bounds);

    // 拒绝：非有限固定值 / 条数过多 / 缺请求。 / Refusals: non-finite value, too many, missing request.
    assert!(verify(DumpOptionsSpec { fixed_variables: vec![fixed(0, f64::NAN)],
        ..document.clone() }).unwrap_err().message.contains("must be finite"));
    assert!(verify(DumpOptionsSpec {
        fixed_variables: (0..257).map(|index| fixed(index as u32, 0.0)).collect(),
        ..document.clone() }).unwrap_err().message.contains("at most 256 fixed variables"));
    let mut missing = base_request();
    missing.model_kind = Some(ModelKind::DumpOptionsVerify);
    assert_eq!(engine.submit(missing).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        let payload = serde_json::to_string(&document).unwrap();
        std::fs::write(directory.join("dump-options.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn native_write_scope_entry_shows_the_unchecked_counters() {
    use ospf_rust_bridge::{NativeStructureSpec, NativeWriteScopeReport, NativeWriteScopeRequest,
        OutcomeSpec, WriteRecordSpec};

    let engine = NativeEngine::default();
    let verify = |document: NativeWriteScopeRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::NativeWriteScopeVerify);
        request.native_write_scope = Some(document);
        engine.submit(request)
    };
    let run = |document: NativeWriteScopeRequest| {
        let view = verify(document).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: NativeWriteScopeReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        engine.release(&view.id).unwrap();
        report
    };
    let record = |fingerprint: Option<&str>, function: &str| WriteRecordSpec {
        writer: "abs-writer".to_owned(), schema: "functions-abs-1".to_owned(),
        fingerprint: fingerprint.map(|value| value.to_owned()),
        function: Some(function.to_owned()) };
    let native = OutcomeSpec { kind: "native".to_owned(), reason: None, message: None };
    let rejected = OutcomeSpec { kind: "fallback".to_owned(),
        reason: Some("rejected".to_owned()), message: Some("range".to_owned()) };
    let document = NativeWriteScopeRequest {
        records: vec![record(Some("fp-1"), "abs"), record(None, "abs")],
        structure: Some(NativeStructureSpec { fingerprint: Some("fp-1".to_owned()),
            function_name: Some("abs".to_owned()) }),
        outcomes: vec![native.clone(), rejected.clone()],
        native_writes: None, materialized_fallbacks: None };

    let report = run(document.clone());
    assert_eq!(report.matches, vec![true, false]);
    assert_eq!(report.reasons, vec![None, Some("rejected".to_owned())]);
    assert!(report.predicates[0].is_native && !report.predicates[0].requires_fallback);
    assert!(!report.predicates[1].is_native && report.predicates[1].requires_fallback);
    assert!(!report.fully_native);
    assert!(report.has_fallbacks);
    assert!(report.the_two_predicates_are_complementary);
    assert!(report.an_empty_report_says_neither);
    assert!(report.fully_native_ignores_the_outcomes);
    assert!(report.has_fallbacks_ignores_the_outcomes);
    assert!(report.a_fresh_record_never_matches);
    assert!(report.both_fingerprints_must_be_present);
    assert!(report.the_function_name_is_not_part_of_the_match);

    // 全是 fallback 但计数说 0 → 仍是"全原生"。 / All fallback while the counter says 0 → still "fully native".
    let lying = run(NativeWriteScopeRequest {
        outcomes: vec![OutcomeSpec { kind: "fallback".to_owned(),
            reason: Some("no-writer".to_owned()), message: None }],
        materialized_fallbacks: Some(0), native_writes: Some(1), ..document.clone() });
    assert!(lying.fully_native);
    assert!(!lying.has_fallbacks);
    // 空报告两边都是 false。 / An empty report answers neither.
    let empty = run(NativeWriteScopeRequest { outcomes: Vec::new(), ..document.clone() });
    assert!(!empty.fully_native);
    assert!(!empty.has_fallbacks);

    // 结构没指纹时记录都匹配不上。 / With no structure fingerprint, no record matches.
    let no_fingerprint = run(NativeWriteScopeRequest {
        structure: Some(NativeStructureSpec { fingerprint: None,
            function_name: Some("abs".to_owned()) }), ..document.clone() });
    assert!(no_fingerprint.matches.iter().all(|matched| !matched));

    // 拒绝：未知结果种类 / 未知回退原因 / 条数过多 / 缺请求。 
    // Refusals: unknown outcome kind, unknown fallback reason, too many, missing request.
    assert!(verify(NativeWriteScopeRequest { outcomes: vec![OutcomeSpec {
        kind: "cached".to_owned(), reason: None, message: None }], ..document.clone() })
        .unwrap_err().message.contains("unknown outcome kind"));
    assert!(verify(NativeWriteScopeRequest { outcomes: vec![OutcomeSpec {
        kind: "fallback".to_owned(), reason: Some("stale".to_owned()), message: None }],
        ..document.clone() }).unwrap_err().message.contains("unknown fallback reason"));
    assert!(verify(NativeWriteScopeRequest {
        records: (0..65).map(|index| WriteRecordSpec { writer: format!("w{index}"),
            schema: "s-1".to_owned(), fingerprint: None, function: None }).collect(),
        ..document.clone() }).unwrap_err().message.contains("at most 64 entries"));
    let mut missing = base_request();
    missing.model_kind = Some(ModelKind::NativeWriteScopeVerify);
    assert_eq!(engine.submit(missing).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        let payload = serde_json::to_string(&document).unwrap();
        std::fs::write(directory.join("native-write-scope.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn writer_registry_entry_shows_the_scheduling_contract() {
    use ospf_rust_bridge::{DestinationRow, WriterRegistryReport, WriterRegistryRequest, WriterSpec};

    let engine = NativeEngine::default();
    let verify = |document: WriterRegistryRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::WriterRegistryVerify);
        request.writer_registry = Some(document);
        engine.submit(request)
    };
    let run = |document: WriterRegistryRequest| {
        let view = verify(document).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: WriterRegistryReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        engine.release(&view.id).unwrap();
        report
    };
    let writer = |name: &str, claims: &[&str], mode: &str| WriterSpec { name: name.to_owned(),
        claims: claims.iter().map(|claim| (*claim).to_owned()).collect(),
        mode: Some(mode.to_owned()) };
    let document = WriterRegistryRequest {
        writers: vec![writer("abs-writer", &["abs"], "native"),
            writer("exp-writer", &["exp"], "native")],
        structures: vec!["abs".to_owned(), "exp".to_owned()] };

    let report = run(document.clone());
    assert!(!report.failed);
    assert_eq!(report.names, vec!["abs-writer".to_owned(), "exp-writer".to_owned()]);
    assert_eq!(report.calls, vec![1, 1]);
    assert_eq!(report.destinations[0].kind, "native");
    assert_eq!(report.destinations[0].writer.as_deref(), Some("abs-writer"));
    assert_eq!(report.destinations[1].writer.as_deref(), Some("exp-writer"));
    assert!(report.registration_order_decides);
    assert!(report.decided_structures_leave_the_batch);
    assert!(report.an_empty_batch_skips_the_writer);
    assert!(report.a_length_mismatch_fails_immediately);
    assert!(report.a_writer_error_propagates);
    assert!(report.a_skip_is_not_a_fallback);
    assert!(report.names_are_in_order_and_not_deduped);

    // 先注册先决定：后注册者一次都没被调用。 
    // The first registered decides: the later writer is never called.
    let pair = run(WriterRegistryRequest {
        writers: vec![writer("first", &["abs"], "native"),
            writer("second", &["abs"], "native")],
        structures: vec!["abs".to_owned()] });
    assert_eq!(pair.destinations[0].writer.as_deref(), Some("first"));
    assert_eq!(pair.calls, vec![1, 0]);

    // 跳过交给后续 writer；没人认领才 `no-writer`。 
    // A skip hands the batch on; only an unclaimed structure becomes `no-writer`.
    let skipped = run(WriterRegistryRequest {
        writers: vec![writer("skipper", &["abs"], "skip"),
            writer("taker", &["abs"], "native")],
        structures: vec!["abs".to_owned()] });
    assert_eq!(skipped.destinations[0].writer.as_deref(), Some("taker"));
    let unclaimed = run(WriterRegistryRequest {
        writers: vec![writer("skipper", &["abs"], "skip")],
        structures: vec!["abs".to_owned()] });
    assert_eq!(unclaimed.destinations[0].reason.as_deref(), Some("no-writer"));

    // 条数不符与 writer 错误都让整次调用失败。 
    // Both a length mismatch and a writer error fail the whole call.
    let short = run(WriterRegistryRequest {
        writers: vec![writer("shorty", &["abs", "exp"], "short")],
        structures: vec!["abs".to_owned(), "exp".to_owned()] });
    assert!(short.failed);
    assert!(short.error.as_deref().unwrap_or_default().contains("shorty"));
    assert!(short.destinations.is_empty());
    let boom = run(WriterRegistryRequest {
        writers: vec![writer("boom", &["abs"], "fail")],
        structures: vec!["abs".to_owned()] });
    assert!(boom.failed);
    assert_eq!(boom.calls, vec![1]);

    // 空注册表全 `no-writer`；同名可重复注册。 
    // An empty registry falls back everything; the same name may be registered twice.
    let empty = run(WriterRegistryRequest { writers: Vec::new(),
        structures: vec!["abs".to_owned()] });
    assert!(empty.empty);
    assert_eq!(empty.destinations[0].reason.as_deref(), Some("no-writer"));
    let repeated = run(WriterRegistryRequest {
        writers: vec![writer("alpha", &[], "native"), writer("beta", &[], "native"),
            writer("alpha", &[], "native")],
        structures: Vec::new() });
    assert_eq!(repeated.names, vec!["alpha".to_owned(), "beta".to_owned(),
        "alpha".to_owned()]);

    // 拒绝：未知模式 / 条数过多 / 缺请求。 / Refusals: unknown mode, too many, missing request.
    assert!(verify(WriterRegistryRequest { writers: vec![writer("w", &["abs"], "cache")],
        structures: vec!["abs".to_owned()] }).unwrap_err().message
        .contains("unknown writer mode"));
    assert!(verify(WriterRegistryRequest {
        writers: (0..17).map(|index| writer(&format!("w{index}"), &[], "native")).collect(),
        structures: Vec::new() }).unwrap_err().message.contains("at most 16 writers"));
    let mut missing = base_request();
    missing.model_kind = Some(ModelKind::WriterRegistryVerify);
    assert_eq!(engine.submit(missing).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        let payload = serde_json::to_string(&document).unwrap();
        std::fs::write(directory.join("writer-registry.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn candidate_shortlist_entry_shows_the_tier_partition() {
    use ospf_rust_bridge::{CandidateEntrySpec, ShortlistReport, ShortlistRequest};

    let engine = NativeEngine::default();
    let verify = |document: ShortlistRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::CandidateShortlistVerify);
        request.candidate_shortlist = Some(document);
        engine.submit(request)
    };
    let run = |document: ShortlistRequest| {
        let view = verify(document).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: ShortlistReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        engine.release(&view.id).unwrap();
        report
    };
    let candidate = |id: &str, tier: &str, activity: &str, dual: Option<f64>| CandidateEntrySpec {
        id: id.to_owned(), tier: Some(tier.to_owned()),
        activity: Some(activity.to_owned()), dual_value: dual, slack: None, group: None };
    let document = ShortlistRequest {
        candidates: vec![candidate("strong", "A", "ACTIVE", Some(1.0)),
            candidate("weak", "B", "ACTIVE", Some(0.0)),
            candidate("inactive", "C", "INACTIVE", None),
            candidate("unknown", "UNCLASSIFIED", "UNKNOWN", None)],
        limit: Some(2), validate: true, dual_threshold: None };

    let report = run(document.clone());
    assert_eq!(report.validated, Some(true));
    assert_eq!(report.error, None);
    assert_eq!(report.shortlist, vec!["strong".to_owned(), "weak".to_owned()]);
    assert_eq!(report.tiers, vec!["A".to_owned(), "B".to_owned()]);
    assert_eq!(report.priorities, vec![0, 1]);
    assert!(report.only_a_and_b_are_default);
    assert!(report.lower_tiers_fill_the_remaining_slots);
    assert!(report.a_zero_limit_returns_nothing);
    assert!(report.a_large_limit_returns_everything_but_regroups);
    assert!(report.the_shortlist_never_sorts);
    assert!(report.priorities_must_be_dense_and_positional);
    assert!(report.tiers_require_evidence);

    // 名额够时低层被追加。 / With enough slots the lower tiers are appended.
    let wide = run(ShortlistRequest { limit: Some(4), ..document.clone() });
    assert_eq!(wide.shortlist.len(), 4);
    assert_eq!(wide.tiers, vec!["A".to_owned(), "B".to_owned(), "C".to_owned(),
        "UNCLASSIFIED".to_owned()]);
    // 大名额返回全部，但 `C` 在前、`A` 在后的输入会被**分组**成 `A` 在前。 
    // A large limit returns everything, but `C` first with `A` later gets **regrouped** to `A` first.
    let regrouped = run(ShortlistRequest { candidates: vec![
        candidate("c", "C", "INACTIVE", None), candidate("a", "A", "ACTIVE", Some(1.0))],
        limit: Some(9), validate: true, dual_threshold: None });
    assert_eq!(regrouped.shortlist, vec!["a".to_owned(), "c".to_owned()]);
    // `shortlist(0)` 是空。 / `shortlist(0)` is empty.
    let zero = run(ShortlistRequest { limit: Some(0), ..document.clone() });
    assert!(zero.shortlist.is_empty());

    // 分层需要证据：A 恰好等于阈值被拒、B 超过阈值被拒、C 是 active 被拒。 
    // A tier needs evidence: A at the threshold, B above it and an active C are all refused.
    let at_threshold = run(ShortlistRequest { candidates: vec![candidate("a", "A", "ACTIVE",
        Some(1e-9))], limit: Some(1), validate: true, dual_threshold: None });
    assert_eq!(at_threshold.validated, Some(false));
    assert!(at_threshold.error.as_deref().unwrap_or_default()
        .contains("dual above the threshold"));
    let bad_b = run(ShortlistRequest { candidates: vec![candidate("b", "B", "ACTIVE", Some(1.0))],
        limit: Some(1), validate: true, dual_threshold: None });
    assert_eq!(bad_b.validated, Some(false));
    let c_active = run(ShortlistRequest { candidates: vec![candidate("c", "C", "ACTIVE", None)],
        limit: Some(1), validate: true, dual_threshold: None });
    assert_eq!(c_active.validated, Some(false));
    // `NearlyActive` 不是 active，所以 C 可以通过。 / `NearlyActive` is not active, so C passes.
    let c_nearly = run(ShortlistRequest { candidates: vec![candidate("c", "C", "NEARLY_ACTIVE",
        None)], limit: Some(1), validate: true, dual_threshold: None });
    assert_eq!(c_nearly.validated, Some(true));
    // 重复 ID 被拒。 / A duplicate ID is refused.
    let duplicated = run(ShortlistRequest { candidates: vec![
        candidate("a", "UNCLASSIFIED", "UNKNOWN", None),
        candidate("a", "UNCLASSIFIED", "UNKNOWN", None)],
        limit: Some(2), validate: true, dual_threshold: None });
    assert_eq!(duplicated.validated, Some(false));

    // 拒绝：未知层 / 未知活动性 / 负阈值 / 条数过多 / 缺请求。 
    // Refusals: unknown tier, unknown activity, negative threshold, too many, missing request.
    assert!(verify(ShortlistRequest { candidates: vec![candidate("a", "D", "ACTIVE", None)],
        limit: Some(1), validate: false, dual_threshold: None }).unwrap_err().message
        .contains("unknown tier"));
    assert!(verify(ShortlistRequest { candidates: vec![candidate("a", "A", "hot", None)],
        limit: Some(1), validate: false, dual_threshold: None }).unwrap_err().message
        .contains("unknown activity"));
    assert!(verify(ShortlistRequest { dual_threshold: Some(-1.0), ..document.clone() })
        .unwrap_err().message.contains("dual threshold"));
    assert!(verify(ShortlistRequest {
        candidates: (0..129).map(|index| candidate(&format!("c{index}"), "UNCLASSIFIED",
            "UNKNOWN", None)).collect(),
        limit: Some(1), validate: false, dual_threshold: None }).unwrap_err().message
        .contains("at most 128 candidates"));
    let mut missing = base_request();
    missing.model_kind = Some(ModelKind::CandidateShortlistVerify);
    assert_eq!(engine.submit(missing).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        let payload = serde_json::to_string(&document).unwrap();
        std::fs::write(directory.join("candidate-shortlist.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn lazy_flatten_init_entry_shows_the_once_semantics() {
    use ospf_rust_bridge::{LazyInitReport, LazyInitRequest};

    let engine = NativeEngine::default();
    let verify = |document: LazyInitRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::LazyFlattenInitVerify);
        request.lazy_flatten_init = Some(document);
        engine.submit(request)
    };
    let run = |document: LazyInitRequest| {
        let view = verify(document).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: LazyInitReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        engine.release(&view.id).unwrap();
        report
    };
    let document = LazyInitRequest { first: Some(11), second: Some(22), initialize: true };

    // 两次 `init` 只保留第一次。 / Two `init` calls keep only the first.
    let report = run(document.clone());
    assert!(report.initialized);
    assert_eq!(report.label, Some(11));
    assert!(!report.panicked);
    assert_eq!(report.cache_counts, vec![0, 0, 0]);
    assert!(report.init_is_first_wins);
    assert!(report.new_and_default_are_uninitialized);
    assert!(report.token_list_panics_before_init);
    assert!(report.the_three_contexts_share_it);
    assert!(report.only_the_linear_context_can_read_the_list_back);
    assert!(report.the_flag_agrees_with_the_reference);
    assert!(report.the_caches_start_empty);

    // 未初始化就读列表会 panic，消息点名 init。 
    // Reading before init panics, and the message names init.
    let uninitialized = run(LazyInitRequest { first: None, second: None, initialize: false });
    assert!(uninitialized.panicked);
    let message = uninitialized.panic_message.as_deref().unwrap_or_default();
    assert!(message.contains("not initialized"), "{message}");
    assert!(message.contains("init()"), "{message}");
    assert!(!uninitialized.initialized);
    assert_eq!(uninitialized.label, None);
    assert!(uninitialized.reference_is_none);

    // 只给 `second` 时它是唯一一次初始化。 
    // When only `second` is given it is the single initialization.
    let second_only = run(LazyInitRequest { first: Some(1), second: Some(9), initialize: false });
    assert_eq!(second_only.label, Some(9));
    // 只给第一次。 / Only the first.
    let single = run(LazyInitRequest { first: Some(4), second: None, initialize: true });
    assert_eq!(single.label, Some(4));

    // 拒绝：标签过大 / 缺请求。 / Refusals: an oversized label, a missing request.
    assert!(verify(LazyInitRequest { first: Some(1025), second: None, initialize: true })
        .unwrap_err().message.contains("at most 1024"));
    let mut missing = base_request();
    missing.model_kind = Some(ModelKind::LazyFlattenInitVerify);
    assert_eq!(engine.submit(missing).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        let payload = serde_json::to_string(&document).unwrap();
        std::fs::write(directory.join("lazy-flatten-init.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn dump_batch_entry_shows_the_dispatch_semantics() {
    use ospf_rust_bridge::{DumpBatchReport, DumpBatchRequest};

    let engine = NativeEngine::default();
    let verify = |document: DumpBatchRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::DumpBatchVerify);
        request.dump_batch = Some(document);
        engine.submit(request)
    };
    let run = |document: DumpBatchRequest| {
        let view = verify(document).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: DumpBatchReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        engine.release(&view.id).unwrap();
        report
    };
    let document = DumpBatchRequest { format: Some("LP".to_owned()), count: Some(3),
        concurrent: false, include_bounds: None, fail_second: false };

    let report = run(document.clone());
    assert!(!report.failed);
    assert_eq!(report.files.len(), 3);
    assert_eq!(report.files[0].content, "lp 0 bounds=true force=true");
    assert_eq!(report.leftovers.len(), 3);
    assert!(report.concurrent_matches_serial);
    assert!(report.a_single_entry_is_always_serial);
    assert!(report.opm_ignores_the_dump_options);
    assert!(report.errors_carry_the_path);
    assert!(report.a_failure_leaves_earlier_files_behind);
    assert!(report.concurrent_errors_propagate);
    assert!(report.the_format_ignores_the_extension);

    // 并发写出同样的字节。 / Concurrency writes the same bytes.
    let concurrent = run(DumpBatchRequest { concurrent: true, ..document.clone() });
    assert_eq!(concurrent.files, report.files);
    // 选项只到 LP，不到 OPM。 / The options reach LP but not OPM.
    let narrowed = run(DumpBatchRequest { include_bounds: Some(false), ..document.clone() });
    assert_eq!(narrowed.files[0].content, "lp 0 bounds=false force=true");
    let opm = run(DumpBatchRequest { include_bounds: Some(false),
        format: Some("OPM".to_owned()), ..document.clone() });
    assert_eq!(opm.files[0].content, "opm 0");
    // 失败保留先前文件，消息带路径。 / A failure keeps earlier files, and the message carries the path.
    let failing = run(DumpBatchRequest { fail_second: true, ..document.clone() });
    assert!(failing.failed);
    assert!(failing.error.as_deref().unwrap_or_default().contains("failed to dump"));
    assert_eq!(failing.files.len(), 1);
    assert_eq!(failing.leftovers, vec!["entry-0.out".to_owned()]);
    // 拒绝：条目数越界 / 未知格式 / 缺请求。 / Refusals.
    for count in [0, 9] {
        assert!(verify(DumpBatchRequest { count: Some(count), ..document.clone() })
            .unwrap_err().message.contains("between 1 and 8"));
    }
    assert!(verify(DumpBatchRequest { format: Some("MPS".to_owned()), ..document.clone() })
        .unwrap_err().message.contains("unknown format"));
    let mut missing = base_request();
    missing.model_kind = Some(ModelKind::DumpBatchVerify);
    assert_eq!(engine.submit(missing).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        let payload = serde_json::to_string(&document).unwrap();
        std::fs::write(directory.join("dump-batch.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn quadratic_elastic_entry_shows_the_switch_semantics() {
    use ospf_rust_bridge::{QuadraticElasticReport, QuadraticElasticRequest};

    let engine = NativeEngine::default();
    let verify = |document: QuadraticElasticRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::QuadraticElasticVerify);
        request.quadratic_elastic = Some(document);
        engine.submit(request)
    };
    let run = |document: QuadraticElasticRequest| {
        let view = verify(document).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: QuadraticElasticReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        engine.release(&view.id).unwrap();
        report
    };
    let document = QuadraticElasticRequest { penalty: None, linear_constraints: true,
        rows: Some(2) };

    let report = run(document.clone());
    assert_eq!(report.variables_before, 2);
    assert_eq!(report.variables_after, 4);
    assert_eq!([report.constraints_before, report.constraints_after], [2, 2]);
    assert_eq!(report.quadratic_constraints, 0);
    assert_eq!(report.objective, vec![3.0, 3.0, 1_000.0, 1_000.0]);
    assert_eq!(report.slack_coefficient, Some(-1.0));
    assert_eq!(report.objective_category, "Minimum");
    assert_eq!(report.source_objective_category, "Maximum");
    assert_eq!(report.slack_group_ids.len(), 2);
    assert!(report.one_slack_per_linear_row);
    assert!(report.disabling_adds_no_slack);
    assert!(report.disabling_keeps_the_objective_category);
    assert!(report.the_penalty_is_last_write_wins_and_enters_the_objective);
    assert!(report.slacks_only_have_the_negative_side);
    assert!(report.the_default_penalty_is_one_thousand);
    assert!(report.disabling_still_resizes_the_objective);

    // 关掉开关：不加松弛、方向保持原值。 
    // Disabled: no slack, and the direction keeps its original value.
    let disabled = run(QuadraticElasticRequest { linear_constraints: false,
        ..document.clone() });
    assert_eq!(disabled.variables_after, disabled.variables_before);
    assert_eq!(disabled.objective, vec![3.0, 3.0]);
    assert_eq!(disabled.objective_category, "Maximum");
    assert!(disabled.slack_group_ids.is_empty());
    assert_eq!(disabled.slack_coefficient, None);

    // 显式罚项进目标。 / An explicit penalty reaches the objective.
    let penalized = run(QuadraticElasticRequest { penalty: Some(4.5), ..document.clone() });
    assert_eq!(penalized.objective[2], 4.5);
    assert_eq!(penalized.objective[3], 4.5);

    // 拒绝：行数越界 / 负罚项 / 缺请求。 / Refusals: rows out of range, negative penalty, missing request.
    for rows in [0, 5] {
        assert!(verify(QuadraticElasticRequest { rows: Some(rows), ..document.clone() })
            .unwrap_err().message.contains("between 1 and 4"));
    }
    assert!(verify(QuadraticElasticRequest { penalty: Some(-1.0), ..document.clone() })
        .unwrap_err().message.contains("penalty"));
    let mut missing = base_request();
    missing.model_kind = Some(ModelKind::QuadraticElasticVerify);
    assert_eq!(engine.submit(missing).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        let payload = serde_json::to_string(&document).unwrap();
        std::fs::write(directory.join("quadratic-elastic.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn stable_value_entry_shows_the_lookup_contract() {
    use ospf_rust_bridge::{StableValueEntry, StableValueReport, StableValueRequest};

    let engine = NativeEngine::default();
    let verify = |document: StableValueRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::StableValueVerify);
        request.stable_value = Some(document);
        engine.submit(request)
    };
    let run = |document: StableValueRequest| {
        let view = verify(document).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: StableValueReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        engine.release(&view.id).unwrap();
        report
    };
    let document = StableValueRequest {
        stable_values: vec![StableValueEntry { id: "x".to_owned(), value: 3.0 }],
        has_solution: true, requested: Some("x".to_owned()), binding: vec!["x".to_owned()] };

    let report = run(document.clone());
    assert_eq!(report.value, Some(3.0));
    assert_eq!(report.error, None);
    assert_eq!(report.requested.as_deref(), Some("x"));
    assert!(report.validate_runs_before_the_lookup);
    assert!(report.a_missing_solution_and_a_missing_id_share_one_message);
    assert!(report.the_message_carries_the_id);
    assert!(report.the_value_is_returned_verbatim);
    assert!(report.a_zero_value_is_still_returned);
    assert!(report.ids_are_matched_exactly);
    assert!(report.it_ignores_the_binding);

    // 缺 ID 报出 ID 原文。 / A missing ID is reported verbatim.
    let missing = run(StableValueRequest { requested: Some("ghost".to_owned()),
        ..document.clone() });
    assert_eq!(missing.value, None);
    assert!(missing.error.as_deref().unwrap_or_default()
        .contains("missing stable variable ghost"));
    // 无解的报告落进同一条消息（不留 `validate` 的痕迹）。 
    // A solution-less report lands on the same message, leaving no trace of `validate`.
    let without = run(StableValueRequest { has_solution: false, ..document.clone() });
    assert_eq!(without.value, None);
    assert!(without.error.as_deref().unwrap_or_default()
        .contains("missing stable variable x"));
    // 绑定外的 ID 也能取到值。 / An ID outside the binding still returns a value.
    let unbound = run(StableValueRequest {
        stable_values: vec![StableValueEntry { id: "ghost".to_owned(), value: 7.0 }],
        requested: Some("ghost".to_owned()), ..document.clone() });
    assert_eq!(unbound.value, Some(7.0));
    assert_eq!(unbound.error, None);
    // `0.0` 与 `2.5` 都原样返回。 / Both `0.0` and `2.5` come back verbatim.
    let zero = run(StableValueRequest {
        stable_values: vec![StableValueEntry { id: "x".to_owned(), value: 0.0 }],
        ..document.clone() });
    assert_eq!(zero.value, Some(0.0));
    let fractional = run(StableValueRequest {
        stable_values: vec![StableValueEntry { id: "x".to_owned(), value: 2.5 }],
        ..document.clone() });
    assert_eq!(fractional.value, Some(2.5));
    // ID 精确匹配，不 trim、不看大小写。 / IDs match exactly: no trimming, no case folding.
    for requested in [" x", "X"] {
        let exact = run(StableValueRequest { requested: Some(requested.to_owned()),
            ..document.clone() });
        assert_eq!(exact.value, None, "{requested}");
        assert!(exact.error.is_some(), "{requested}");
    }
    // 拒绝：条目过多 / 缺请求。 / Refusals: too many entries, a missing request.
    let too_many = StableValueRequest {
        stable_values: (0..65).map(|index| StableValueEntry { id: format!("v{index}"),
            value: 0.0 }).collect(), ..document.clone() };
    assert!(verify(too_many).unwrap_err().message.contains("at most 64 stable values"));
    let mut missing_request = base_request();
    missing_request.model_kind = Some(ModelKind::StableValueVerify);
    assert_eq!(engine.submit(missing_request).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        let payload = serde_json::to_string(&document).unwrap();
        std::fs::write(directory.join("stable-value.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn master_assignment_entry_shows_the_tolerance_semantics() {
    use ospf_rust_bridge::{
        MasterAssignedValue, MasterAssignmentReport, MasterAssignmentRequest,
        MasterAssignedVariableSpec,
    };

    let engine = NativeEngine::default();
    let verify = |document: MasterAssignmentRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::MasterAssignmentVerify);
        request.master_assignment = Some(document);
        engine.submit(request)
    };
    let run = |document: MasterAssignmentRequest| {
        let view = verify(document).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: MasterAssignmentReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        engine.release(&view.id).unwrap();
        report
    };
    let variable = |id: &str, value: f64, lower: i64, upper: i64| MasterAssignedVariableSpec {
        id: id.to_owned(), value, domain: Some("integer".to_owned()),
        lower: Some(lower), upper: Some(upper) };
    let document = MasterAssignmentRequest {
        variables: vec![variable("x", 3.0, 0, 10)], status: None, tolerance: None };

    let report = run(document.clone());
    assert_eq!(report.assigned, vec![MasterAssignedValue { id: "x".to_owned(), value: 3.0 }]);
    assert_eq!(report.error, None);
    assert!(report.only_optimal_reports_are_accepted);
    assert!(report.the_tolerance_must_be_finite_and_non_negative);
    assert!(report.rounding_is_half_away_from_zero);
    assert!(report.the_tolerance_is_an_absolute_difference);
    assert!(report.there_are_two_range_gates);
    assert!(report.the_domain_check_sees_the_rounded_value);
    assert!(report.a_missing_id_says_optimal_master_report);

    // `Feasible` 但无最优证明 → 被拒。 / Feasible without the optimality proof is refused.
    let unproven = run(MasterAssignmentRequest { status: Some("feasible".to_owned()),
        ..document.clone() });
    assert!(unproven.assigned.is_empty());
    assert!(unproven.error.as_deref().unwrap_or_default().contains("verified optimal"));
    // 半数远离零：`2.5 → 3`、`-2.5 → -3`。 / Half away from zero: `2.5 → 3`, `-2.5 → -3`.
    for (value, expected) in [(2.5, 3.0), (3.5, 4.0), (-2.5, -3.0)] {
        let rounded = run(MasterAssignmentRequest { tolerance: Some(0.5),
            variables: vec![variable("x", value, -10, 10)], status: None });
        assert_eq!(rounded.assigned[0].value, expected, "{value}");
    }
    // 恰好等于容差通过，略超被拒。 / An exact match passes, a slight excess is refused.
    let at = run(MasterAssignmentRequest { tolerance: Some(0.25),
        variables: vec![variable("x", 1.25, 0, 10)], status: None });
    assert_eq!(at.assigned[0].value, 1.0);
    let over = run(MasterAssignmentRequest { tolerance: Some(0.25),
        variables: vec![variable("x", 1.26, 0, 10)], status: None });
    assert!(over.assigned.is_empty());
    assert!(over.error.as_deref().unwrap_or_default().contains("is not integral"));
    // 域检查看舍入后的值。 / The domain check sees the rounded value.
    let inside = run(MasterAssignmentRequest { tolerance: Some(0.5),
        variables: vec![variable("x", 10.4, 0, 10)], status: None });
    assert_eq!(inside.assigned[0].value, 10.0);
    let outside = run(MasterAssignmentRequest { tolerance: Some(0.5),
        variables: vec![variable("x", 10.6, 0, 10)], status: None });
    assert!(outside.assigned.is_empty());
    assert!(outside.error.as_deref().unwrap_or_default()
        .contains("outside its declared domain"));
    // 两道范围闸。 / The two range gates.
    const LIMIT: i64 = 9_007_199_254_740_991;
    let huge = run(MasterAssignmentRequest { tolerance: Some(0.0),
        variables: vec![variable("x", 1e19, -LIMIT, LIMIT)], status: None });
    assert!(huge.error.as_deref().unwrap_or_default().contains("outside the i64 range"));
    let exact = run(MasterAssignmentRequest { tolerance: Some(0.0),
        variables: vec![variable("x", 1e17, -LIMIT, LIMIT)], status: None });
    assert!(exact.error.as_deref().unwrap_or_default()
        .contains("outside the exact f64 integer range"));
    // 拒绝：未知状态 / 空表 / 空 ID / 非有限值 / 未知域 / 界反了 / 条数过多 / 缺请求。
    // Refusals.
    for document in [
        MasterAssignmentRequest { status: Some("infeasible".to_owned()), ..document.clone() },
        MasterAssignmentRequest { variables: Vec::new(), ..document.clone() },
        MasterAssignmentRequest { variables: vec![MasterAssignedVariableSpec {
            id: "  ".to_owned(), value: 1.0, domain: None, lower: None, upper: None }],
            ..document.clone() },
        MasterAssignmentRequest { variables: vec![MasterAssignedVariableSpec {
            id: "x".to_owned(), value: f64::NAN, domain: None, lower: None, upper: None }],
            ..document.clone() },
        MasterAssignmentRequest { variables: vec![MasterAssignedVariableSpec {
            id: "x".to_owned(), value: 1.0, domain: Some("real".to_owned()),
            lower: None, upper: None }], ..document.clone() },
        MasterAssignmentRequest { variables: vec![variable("x", 1.0, 5, 1)],
            ..document.clone() },
        MasterAssignmentRequest {
            variables: (0..33).map(|index| variable(&format!("v{index}"), 0.0, 0, 10))
                .collect(), ..document.clone() },
    ] {
        assert_eq!(verify(document).unwrap_err().code, "InvalidInput");
    }
    let mut missing = base_request();
    missing.model_kind = Some(ModelKind::MasterAssignmentVerify);
    assert_eq!(engine.submit(missing).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        let payload = serde_json::to_string(&document).unwrap();
        std::fs::write(directory.join("master-assignment.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn sparse_triad_entry_shows_the_hand_written_semantics() {
    use ospf_rust_bridge::{
        SparseBoundsRow, SparseBoundsSpec, SparseEntrySpec, SparseTriadReport, SparseTriadRequest,
    };

    let engine = NativeEngine::default();
    let verify = |document: SparseTriadRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::SparseTriadVerify);
        request.sparse_triad = Some(document);
        engine.submit(request)
    };
    let run = |document: SparseTriadRequest| {
        let view = verify(document).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: SparseTriadReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        engine.release(&view.id).unwrap();
        report
    };
    let document = SparseTriadRequest {
        entries: vec![SparseEntrySpec { index: 3, value: 1.0 },
            SparseEntrySpec { index: 3, value: 2.0 },
            SparseEntrySpec { index: 0, value: 0.0 }],
        duplicate_variable: false, constraint_rows: Some(2), set_bounds: None };

    let report = run(document.clone());
    assert_eq!(report.entry_count, 3);
    assert_eq!(report.entry_indices, vec![3, 3, 0]);
    assert_eq!(report.variables, 1);
    assert_eq!(report.constraint_names, vec!["c0".to_owned(), "c1".to_owned()]);
    assert_eq!(report.bounds, vec![SparseBoundsRow { lower: "-inf".to_owned(),
        upper: "inf".to_owned() }]);
    assert_eq!(report.first_index, Some(0));
    assert_eq!(report.missing_index, None);
    assert_eq!(report.out_of_range_bounds, None);
    assert!(report.add_never_merges_or_drops_zeros);
    assert!(report.a_token_without_bounds_becomes_infinite);
    assert!(report.explicit_bounds_override_the_token);
    assert!(report.a_duplicate_id_overwrites_the_index_only);
    assert!(report.constraint_names_default_to_c);
    assert!(report.set_bounds_accepts_an_empty_domain);
    assert!(report.set_bounds_out_of_range_is_silently_ignored);

    // 重复 ID：变量数 +1，索引指向后加的那个。 
    // A duplicate ID: the count grows and the index points at the later variable.
    let duplicate = run(SparseTriadRequest { duplicate_variable: true, ..document.clone() });
    assert_eq!(duplicate.variables, 2);
    assert_eq!(duplicate.first_index, Some(1));
    // 空域被原样存下。 / An empty domain is stored verbatim.
    let empty_domain = run(SparseTriadRequest {
        set_bounds: Some(SparseBoundsSpec { index: 0, lower: 5.0, upper: 1.0 }),
        ..document.clone() });
    assert_eq!(empty_domain.bounds, vec![SparseBoundsRow { lower: "5".to_owned(),
        upper: "1".to_owned() }]);
    // 越界 `setBounds` 什么都不改。 / An out-of-range `setBounds` changes nothing.
    let out_of_range = run(SparseTriadRequest {
        set_bounds: Some(SparseBoundsSpec { index: 9, lower: 8.0, upper: 9.0 }),
        ..document.clone() });
    assert_eq!(out_of_range.bounds, vec![SparseBoundsRow { lower: "-inf".to_owned(),
        upper: "inf".to_owned() }]);
    // 拒绝：条目过多 / 行数过多 / 非有限值 / 缺请求。 / Refusals.
    assert!(verify(SparseTriadRequest {
        entries: (0..65).map(|index| SparseEntrySpec { index, value: 1.0 }).collect(),
        ..document.clone() }).unwrap_err().message.contains("at most 64 entries"));
    assert!(verify(SparseTriadRequest { constraint_rows: Some(17), ..document.clone() })
        .unwrap_err().message.contains("at most 16 constraint rows"));
    assert!(verify(SparseTriadRequest {
        entries: vec![SparseEntrySpec { index: 0, value: f64::INFINITY }],
        ..document.clone() }).unwrap_err().message.contains("must be finite"));
    let mut missing = base_request();
    missing.model_kind = Some(ModelKind::SparseTriadVerify);
    assert_eq!(engine.submit(missing).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        let payload = serde_json::to_string(&document).unwrap();
        std::fs::write(directory.join("sparse-triad.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn matrix_rows_entry_shows_the_lockstep_semantics() {
    use ospf_rust_bridge::{
        MatrixEntrySpec, MatrixRowSpec, MatrixRowsReport, MatrixRowsRequest,
    };

    let engine = NativeEngine::default();
    let verify = |document: MatrixRowsRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::MatrixRowsVerify);
        request.matrix_rows = Some(document);
        engine.submit(request)
    };
    let run = |document: MatrixRowsRequest| {
        let view = verify(document).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: MatrixRowsReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        engine.release(&view.id).unwrap();
        report
    };
    let row = |entries: &[(u64, f64)]| MatrixRowSpec { entries: entries.iter()
        .map(|(index, value)| MatrixEntrySpec { index: *index, value: *value }).collect() };
    let document = MatrixRowsRequest {
        rows: vec![row(&[(5, 1.0), (1, 2.0), (3, 3.0)]), row(&[(0, 4.0), (0, 5.0)]), row(&[])],
        with_metadata: true };

    let report = run(document.clone());
    assert_eq!(report.matrix_rows, 3);
    assert_eq!(report.num_constraints, 3);
    assert_eq!(report.parallel_lengths, vec![3, 3, 3, 3, 3, 3, 3, 3]);
    assert_eq!(report.first_row_indices, vec![5, 1, 3]);
    assert!(report.get_row_out_of_range_is_none);
    assert!(report.add_row_validates_nothing);
    assert!(report.the_eight_vectors_move_together);
    assert!(report.num_constraints_reads_only_b);
    assert!(report.add_constraint_only_fills_the_name);
    assert!(report.the_row_keeps_its_entry_order);
    assert!(report.get_row_is_bounds_checked);
    assert!(report.clone_constraints_is_deep);

    // 直接 `A.add_row` 会让 `num_constraints()` 停在 0（只读 `b`）。 
    // A raw `A.add_row` leaves `num_constraints()` at 0, because it reads only `b`.
    let raw = run(MatrixRowsRequest { with_metadata: false, ..document.clone() });
    assert_eq!(raw.matrix_rows, 3);
    assert_eq!(raw.num_constraints, 0);
    assert_eq!(raw.parallel_lengths, vec![3, 0, 0, 0, 0, 0, 0, 0]);
    // 不同宽度与空行都被接受。 / Differing widths and empty rows are accepted.
    let ragged = run(MatrixRowsRequest { rows: vec![row(&[(0, 1.0)]),
        row(&[(0, 1.0), (1, 2.0), (2, 3.0)]), row(&[])], with_metadata: true });
    assert_eq!(ragged.matrix_rows, 3);
    // 一行都没有也行。 / No rows at all is fine.
    let none = run(MatrixRowsRequest { rows: Vec::new(), with_metadata: true });
    assert_eq!(none.matrix_rows, 0);
    assert!(none.first_row_indices.is_empty());
    // 拒绝：行数过多 / 单行条目过多 / 非有限值 / 缺请求。 / Refusals.
    assert!(verify(MatrixRowsRequest { rows: (0..17).map(|_| row(&[(0, 1.0)])).collect(),
        with_metadata: true }).unwrap_err().message.contains("at most 16 rows"));
    assert!(verify(MatrixRowsRequest { rows: vec![MatrixRowSpec { entries: (0..65)
        .map(|index| MatrixEntrySpec { index, value: 1.0 }).collect() }], with_metadata: true })
        .unwrap_err().message.contains("at most 64 entries"));
    assert!(verify(MatrixRowsRequest { rows: vec![row(&[(0, f64::NAN)])],
        with_metadata: true }).unwrap_err().message.contains("must be finite"));
    let mut missing = base_request();
    missing.model_kind = Some(ModelKind::MatrixRowsVerify);
    assert_eq!(engine.submit(missing).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        let payload = serde_json::to_string(&document).unwrap();
        std::fs::write(directory.join("matrix-rows.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn polynomial_probe_entry_shows_the_mutation_semantics() {
    use ospf_rust_bridge::{
        LinearTermRow, PolynomialProbeReport, PolynomialProbeRequest, PolynomialTermSpec,
    };

    let engine = NativeEngine::default();
    let verify = |document: PolynomialProbeRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::PolynomialProbeVerify);
        request.polynomial_probe = Some(document);
        engine.submit(request)
    };
    let run = |document: PolynomialProbeRequest| {
        let view = verify(document).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: PolynomialProbeReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        engine.release(&view.id).unwrap();
        report
    };
    let document = PolynomialProbeRequest { terms: vec![
        PolynomialTermSpec { coefficient: 1.5, var_index: 9 },
        PolynomialTermSpec { coefficient: -2.5, var_index: 2 },
        PolynomialTermSpec { coefficient: 0.0, var_index: 5 }],
        constant: Some(3.5), overwrite_constant: false, rewrite_first_coefficient: None };

    let report = run(document.clone());
    assert_eq!(report.monomial_count, 3);
    assert_eq!(report.monomials, vec![
        LinearTermRow { coefficient: "1.5".to_owned(), var_index: 9 },
        LinearTermRow { coefficient: "-2.5".to_owned(), var_index: 2 },
        LinearTermRow { coefficient: "0".to_owned(), var_index: 5 }]);
    assert_eq!(report.constant_term, "3.5");
    assert_eq!(report.zero_constant, "3.5");
    assert_eq!(report.constant_polynomial_constant, "3.5");
    assert_eq!(report.lifted_kinds, vec!["Linear".to_owned(); 3]);
    assert!(!report.is_constant_only);
    assert!(report.add_monomial_is_pure_append);
    assert!(report.zero_and_constant_are_the_same_shape);
    assert!(report.is_constant_only_equals_is_empty);
    assert!(report.set_constant_term_overwrites);
    assert!(report.set_coefficient_mutates_in_place);
    assert!(report.from_linear_preserves_order_and_coefficients);
    assert!(report.from_linear_yields_linear_kind);

    // 空多项式只有常数项。 / An empty polynomial is constant only.
    let empty = run(PolynomialProbeRequest { terms: Vec::new(), constant: Some(7.0),
        ..document.clone() });
    assert_eq!(empty.monomial_count, 0);
    assert!(empty.is_constant_only);
    assert_eq!(empty.constant_term, "7");
    assert!(empty.lifted_kinds.is_empty());
    // 覆盖常数项与改第一个系数。 / Overwrite the constant and rewrite the first coefficient.
    let overwritten = run(PolynomialProbeRequest { overwrite_constant: true,
        ..document.clone() });
    assert_eq!(overwritten.constant_term, "4.5");
    let rewritten = run(PolynomialProbeRequest { rewrite_first_coefficient: Some(9.5),
        ..document.clone() });
    assert_eq!(rewritten.monomials[0].coefficient, "9.5");
    assert_eq!(rewritten.monomials[0].var_index, 9);
    // 拒绝：条目过多 / 非有限系数 / 索引越界 / 缺请求。 / Refusals.
    assert!(verify(PolynomialProbeRequest { terms: (0..33).map(|index| PolynomialTermSpec {
        coefficient: 1.0, var_index: index }).collect(), ..document.clone() })
        .unwrap_err().message.contains("at most 32 terms"));
    assert!(verify(PolynomialProbeRequest { terms: vec![PolynomialTermSpec {
        coefficient: f64::NAN, var_index: 0 }], ..document.clone() })
        .unwrap_err().message.contains("coefficients must be finite"));
    assert!(verify(PolynomialProbeRequest { terms: vec![PolynomialTermSpec {
        coefficient: 1.0, var_index: 4097 }], ..document.clone() })
        .unwrap_err().message.contains("at most 4096"));
    let mut missing = base_request();
    missing.model_kind = Some(ModelKind::PolynomialProbeVerify);
    assert_eq!(engine.submit(missing).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        let payload = serde_json::to_string(&document).unwrap();
        std::fs::write(directory.join("polynomial-probe.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn model_view_entry_shows_the_read_only_conventions() {
    use ospf_rust_bridge::{ModelViewReport, ModelViewRequest};

    let engine = NativeEngine::default();
    let verify = |document: ModelViewRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::ModelViewVerify);
        request.model_view = Some(document);
        engine.submit(request)
    };
    let run = |document: ModelViewRequest| {
        let view = verify(document).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: ModelViewReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        engine.release(&view.id).unwrap();
        report
    };
    let document = ModelViewRequest { name: Some("probe".to_owned()), variables: Some(3),
        rows: Some(2), quadratic_rows: Some(1), objective_length: None,
        quadratic_objective_rows: None, direction: Some("Maximum".to_owned()) };

    let report = run(document.clone());
    assert_eq!(report.linear_name, "probe");
    assert_eq!(report.quadratic_name, "probe");
    assert_eq!(report.linear_variables, 3);
    assert_eq!(report.linear_constraints, 2);
    assert_eq!(report.quadratic_constraints, 2);
    assert_eq!(report.quadratic_quadratic_constraints, 1);
    assert_eq!(report.objective_length, 3);
    assert_eq!(report.linear_objective_length, 3);
    assert_eq!(report.quadratic_objective_rows, 3);
    assert_eq!(report.objective_category, "Maximum");
    assert!(report.both_views_read_the_linear_name);
    assert!(report.the_quadratic_view_counts_linear_rows_only);
    assert!(report.the_objective_is_never_reconciled);
    assert!(report.objective_is_the_linear_objective);
    assert!(report.the_quadratic_objective_is_unreconciled);
    assert!(report.the_views_do_not_expose_the_direction);
    assert!(report.the_views_are_read_only);

    // 目标向量与 `Q` 行数都不与变量数对账。 
    // Neither the objective nor `Q`'s rows are reconciled with the variable count.
    let short = run(ModelViewRequest { objective_length: Some(1),
        quadratic_objective_rows: Some(0), ..document.clone() });
    assert_eq!(short.linear_variables, 3);
    assert_eq!(short.objective_length, 1);
    assert_eq!(short.quadratic_objective_rows, 0);
    let long = run(ModelViewRequest { variables: Some(1), objective_length: Some(5),
        quadratic_objective_rows: Some(4), ..document.clone() });
    assert_eq!(long.linear_variables, 1);
    assert_eq!(long.objective_length, 5);
    assert_eq!(long.quadratic_objective_rows, 4);
    // 零二次约束也行。 / Zero quadratic constraints is fine.
    let none = run(ModelViewRequest { quadratic_rows: Some(0), ..document.clone() });
    assert_eq!(none.quadratic_quadratic_constraints, 0);
    assert_eq!(none.quadratic_constraints, 2);
    // 拒绝：变量/行数越界 / 目标过长 / 空名 / 未知方向 / 缺请求。 / Refusals.
    assert!(verify(ModelViewRequest { variables: Some(9), ..document.clone() })
        .unwrap_err().message.contains("at most 8 variables"));
    assert!(verify(ModelViewRequest { objective_length: Some(17), ..document.clone() })
        .unwrap_err().message.contains("at most 16 objective entries"));
    assert!(verify(ModelViewRequest { quadratic_objective_rows: Some(17),
        ..document.clone() }).unwrap_err().message.contains("at most 16 objective entries"));
    assert!(verify(ModelViewRequest { name: Some("   ".to_owned()), ..document.clone() })
        .unwrap_err().message.contains("name must not be blank"));
    assert!(verify(ModelViewRequest { direction: Some("Minimise".to_owned()),
        ..document.clone() }).unwrap_err().message.contains("unknown direction"));
    let mut missing = base_request();
    missing.model_kind = Some(ModelKind::ModelViewVerify);
    assert_eq!(engine.submit(missing).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        let payload = serde_json::to_string(&document).unwrap();
        std::fs::write(directory.join("model-view.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn analysis_options_entry_shows_the_four_stage_gates() {
    use ospf_rust_bridge::{AnalysisOptionsReport, AnalysisOptionsRequest};

    let engine = NativeEngine::default();
    let verify = |document: AnalysisOptionsRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::AnalysisOptionsVerify);
        request.analysis_options = Some(document);
        engine.submit(request)
    };
    let run = |document: AnalysisOptionsRequest| {
        let view = verify(document).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: AnalysisOptionsReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        engine.release(&view.id).unwrap();
        report
    };
    let document = AnalysisOptionsRequest { dual_threshold: None, funnel_default_limit: None,
        candidate_limit: None, run_fixed_integer: None, run_perturbation: None,
        run_target_analysis: None, run_conflict_analysis: None, enumeration_limit: None,
        request_conflict: None, shrink_conflict: None, max_conflict_resolves: None, deltas: None,
        adaptive: None, max_solves: None };

    let report = run(document.clone());
    assert_eq!(report.candidate_limit, 32);
    assert_eq!(report.funnel_default_limit, 32);
    assert!(report.run_fixed_integer && report.run_perturbation);
    assert!(report.run_target_analysis && report.run_conflict_analysis);
    assert_eq!(report.enumeration_limit, 10_000);
    assert_eq!(report.max_conflict_resolves, 128);
    assert_eq!(report.delta_count, 1);
    assert_eq!(report.max_solves, 32);
    assert_eq!(report.validate_message, None);
    assert!(report.all_four_stage_flags_default_true);
    assert!(report.the_candidate_limit_is_snapshotted_from_the_funnel);
    assert!(report.a_zero_candidate_limit_is_legal);
    assert!(report.the_four_run_flags_are_never_validated);
    assert!(report.validation_runs_in_stage_order);
    assert!(report.the_pipeline_and_the_conflict_stage_carry_separate_cp_options);
    assert!(report.the_perturbation_policy_requires_exactly_one_source);
    // 顺序探针报 funnel 的文案；两套 CP 参数各自报同一条枚举文案。 
    // The order probe reports the funnel's message; both CP sets report the same enumeration message.
    assert!(report.order_probe_message.contains("dual threshold"));
    assert!(report.conflict_only_probe_message.contains("enumeration limit"));
    assert_eq!(report.conflict_only_probe_message, report.pipeline_only_probe_message);

    // 全关掉也通过。 / Turning everything off still passes.
    let off = run(AnalysisOptionsRequest { run_fixed_integer: Some(false),
        run_perturbation: Some(false), run_target_analysis: Some(false),
        run_conflict_analysis: Some(false), candidate_limit: Some(0), ..document.clone() });
    assert!(!off.run_fixed_integer && !off.run_conflict_analysis);
    assert_eq!(off.candidate_limit, 0);
    assert_eq!(off.validate_message, None);
    // 扰动来源必须恰好一个。 / The perturbation source must be exactly one.
    let both = run(AnalysisOptionsRequest { deltas: Some(vec![1.0]), adaptive: Some(true),
        ..document.clone() });
    assert!(both.validate_message.as_deref().unwrap().contains("either explicit deltas"));
    let neither = run(AnalysisOptionsRequest { deltas: Some(Vec::new()), ..document.clone() });
    assert_eq!(neither.validate_message, both.validate_message);
    // 拒绝：扰动序列过长 / 缺请求。 / Refusals.
    assert!(verify(AnalysisOptionsRequest { deltas: Some(vec![1.0; 17]), ..document.clone() })
        .unwrap_err().message.contains("at most 16 deltas"));
    let mut missing = base_request();
    missing.model_kind = Some(ModelKind::AnalysisOptionsVerify);
    assert_eq!(engine.submit(missing).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        let payload = serde_json::to_string(&document).unwrap();
        std::fs::write(directory.join("analysis-options.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn capability_matrix_entry_shows_the_inference_lattice() {
    use ospf_rust_bridge::{CapabilityMatrixReport, CapabilityMatrixRequest};

    let engine = NativeEngine::default();
    let verify = |document: CapabilityMatrixRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::CapabilityMatrixVerify);
        request.capability_matrix = Some(document);
        engine.submit(request)
    };
    let run = |document: CapabilityMatrixRequest| {
        let view = verify(document).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: CapabilityMatrixReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        engine.release(&view.id).unwrap();
        report
    };
    let document = CapabilityMatrixRequest { model_types: vec!["Mip".to_owned()],
        dual: Some(true), warm_start: None, exact_cp_lowering: None,
        native_assumption_solving: None, native_unsat_core: None,
        fallback_satisfaction_only: None, overrides: Vec::new() };

    let report = run(document.clone());
    assert_eq!(report.rows.len(), 13);
    let support = |name: &str| report.rows.iter().find(|row| row.capability == name)
        .map(|row| (row.support.clone(), row.available, row.native)).unwrap();
    assert_eq!(support("activity"), ("Supported".to_owned(), true, false));
    assert_eq!(support("fixed_integer_lp_sensitivity"),
        ("Supported".to_owned(), true, false));
    assert_eq!(support("native_unsat_core"), ("Unsupported".to_owned(), false, false));
    // `Conflict` 不可用 → `Mus` 也一定不可用。 / `Conflict` unavailable, so `Mus` is too.
    assert_eq!(support("conflict"), ("Unsupported".to_owned(), false, false));
    assert_eq!(support("mus"), ("Unsupported".to_owned(), false, false));
    assert!(report.lp_dual);
    assert_eq!(report.empty_activity, "Supported");
    assert_eq!(report.empty_rhs_perturbation, "Conditional");
    assert_eq!(report.empty_native_unsat_core, "Unsupported");
    assert_eq!(report.empty_mus, "Unsupported");
    assert!(report.an_empty_matrix_claims_activity_support);
    assert!(report.the_fixed_integer_ladder_is_three_tiered);
    assert!(report.the_perturbation_trio_ignores_the_dual);
    assert!(report.adaptive_mirrors_rhs_perturbation_exactly);
    assert!(report.only_two_capabilities_accept_the_fallback);
    assert!(report.mus_is_gated_on_conflict);
    assert!(report.an_override_never_upgrades_the_native_flag);
    assert!(report.conditional_counts_as_available);

    // 三态阶梯与 fallback 层。 / The three-tier ladder and the fallback tier.
    let ladder = run(CapabilityMatrixRequest { dual: Some(true), model_types: Vec::new(),
        ..document.clone() });
    assert_eq!(ladder.rows.iter().find(|row| row.capability == "fixed_integer_lp_sensitivity")
        .unwrap().support, "Conditional");
    let fallback = run(CapabilityMatrixRequest { fallback_satisfaction_only: Some(true),
        ..document.clone() });
    assert_eq!(fallback.rows.iter().find(|row| row.capability == "assumption_solving")
        .unwrap().support, "Conditional");
    // 覆盖项只改查询。 / An override moves the query only.
    let overridden = run(CapabilityMatrixRequest {
        overrides: vec!["exact_cp_lowering=Supported".to_owned()], ..document.clone() });
    let row = overridden.rows.iter().find(|row| row.capability == "exact_cp_lowering").unwrap();
    assert_eq!(row.support, "Supported");
    assert!(!row.native);
    // 拒绝：未知模型种类 / 未知能力 / 未知级别 / 缺 `=` / 缺请求。 / Refusals.
    assert!(verify(CapabilityMatrixRequest { model_types: vec!["Unknown".to_owned()],
        ..document.clone() }).unwrap_err().message.contains("unknown model kind"));
    assert!(verify(CapabilityMatrixRequest { overrides: vec!["nope=Supported".to_owned()],
        ..document.clone() }).unwrap_err().message.contains("unknown capability"));
    assert!(verify(CapabilityMatrixRequest { overrides: vec!["activity=Maybe".to_owned()],
        ..document.clone() }).unwrap_err().message.contains("unknown support level"));
    assert!(verify(CapabilityMatrixRequest { overrides: vec!["activity".to_owned()],
        ..document.clone() }).unwrap_err().message.contains("must read `name=Level`"));
    let mut missing = base_request();
    missing.model_kind = Some(ModelKind::CapabilityMatrixVerify);
    assert_eq!(engine.submit(missing).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        let payload = serde_json::to_string(&document).unwrap();
        std::fs::write(directory.join("capability-matrix.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn cp_support_merge_entry_shows_the_derivation_rules() {
    use ospf_rust_bridge::{CpConstraintLevelSpec, CpSupportMergeReport, CpSupportMergeRequest};

    let engine = NativeEngine::default();
    let verify = |document: CpSupportMergeRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::CpSupportMergeVerify);
        request.cp_support_merge = Some(document);
        engine.submit(request)
    };
    let run = |document: CpSupportMergeRequest| {
        let view = verify(document).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: CpSupportMergeReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        engine.release(&view.id).unwrap();
        report
    };
    let document = CpSupportMergeRequest { constraint_levels: vec![CpConstraintLevelSpec {
        id: "c0".to_owned(), level: "ExactLowering".to_owned() }], satisfaction: Some(true),
        integer_objective: Some(true), sparse_domain: Some(true), incremental_session: None,
        assumptions: Some(true), solution_hint: Some(true), verified_conflict_seed: Some(true),
        descriptor_capabilities: vec!["constraint_programming".to_owned(),
            "native_assumption_solving".to_owned()] };

    let report = run(document.clone());
    assert_eq!(report.feature_vocabulary.len(), 17);
    assert_eq!(report.feature_vocabulary[16], "incremental_solve");
    assert_eq!(report.model_types, vec!["ConstraintProgramming".to_owned()]);
    assert!(report.native_assumption_solving && !report.native_unsat_core);
    assert!(report.fallback_satisfaction_only && report.exact_cp_lowering);
    assert_eq!(report.assumption_solving, "Supported");
    assert_eq!(report.conflict, "Conditional");
    // 无条件插入的 CP 模型种类让 `model_types` 非空 → `Mus` 直接 `Supported`。 
    // The unconditionally inserted CP model type keeps `model_types` non-empty, so `Mus` is `Supported`.
    assert_eq!(report.mus, "Supported");
    assert_eq!(report.exact_cp_lowering_capability, "Supported");
    assert!(report.native_assumption_solving_comes_from_the_descriptor_only);
    assert!(report.the_fallback_flag_is_the_satisfaction_flag);
    assert!(report.exact_lowering_is_vacuously_true);
    assert!(report.one_inexact_constraint_kills_the_whole_flag);
    assert!(report.five_features_are_never_native);
    assert!(report.per_constraint_levels_never_enter_the_matrix);
    assert!(report.the_cp_model_type_is_inserted_unconditionally);
    assert!(report.three_solver_capabilities_are_silently_ignored);

    // descriptor 决定原生标志，报告决定 fallback。 
    // The descriptor decides the native flags; the report decides the fallback.
    let blind = run(CpSupportMergeRequest { descriptor_capabilities: Vec::new(),
        ..document.clone() });
    assert!(!blind.native_assumption_solving);
    assert_eq!(blind.assumption_solving, "Conditional");
    let quiet = run(CpSupportMergeRequest { satisfaction: Some(false), ..document.clone() });
    assert_eq!(quiet.conflict, "Unsupported");
    assert_eq!(quiet.mus, "Unsupported");
    let core = run(CpSupportMergeRequest { descriptor_capabilities: vec!["unsat_core".to_owned()],
        ..document.clone() });
    assert_eq!(core.conflict, "Supported");
    // 一条不精确就整份假；空约束表空真。 
    // One inexact constraint kills the flag; an empty map is vacuously true.
    let inexact = run(CpSupportMergeRequest { constraint_levels: vec![
        CpConstraintLevelSpec { id: "a".to_owned(), level: "ExactLowering".to_owned() },
        CpConstraintLevelSpec { id: "b".to_owned(), level: "Conditional".to_owned() }],
        ..document.clone() });
    assert!(!inexact.exact_cp_lowering);
    assert_eq!(inexact.exact_cp_lowering_capability, "Unsupported");
    // 拒绝：重复 ID / 空 ID / 未知级别 / 空能力名 / 约束过多 / 缺请求。 / Refusals.
    assert!(verify(CpSupportMergeRequest { constraint_levels: vec![
        CpConstraintLevelSpec { id: "a".to_owned(), level: "Native".to_owned() },
        CpConstraintLevelSpec { id: "a".to_owned(), level: "Native".to_owned() }],
        ..document.clone() }).unwrap_err().message.contains("must be distinct"));
    assert!(verify(CpSupportMergeRequest { constraint_levels: vec![CpConstraintLevelSpec {
        id: " ".to_owned(), level: "Native".to_owned() }], ..document.clone() })
        .unwrap_err().message.contains("must not be blank"));
    assert!(verify(CpSupportMergeRequest { constraint_levels: vec![CpConstraintLevelSpec {
        id: "a".to_owned(), level: "Maybe".to_owned() }], ..document.clone() })
        .unwrap_err().message.contains("unknown level"));
    assert!(verify(CpSupportMergeRequest { descriptor_capabilities: vec![" ".to_owned()],
        ..document.clone() }).unwrap_err().message.contains("capability names must not be blank"));
    assert!(verify(CpSupportMergeRequest { constraint_levels: (0..17).map(|index|
        CpConstraintLevelSpec { id: format!("c{index}"), level: "Native".to_owned() }).collect(),
        ..document.clone() }).unwrap_err().message.contains("at most 16 constraints"));
    let mut missing = base_request();
    missing.model_kind = Some(ModelKind::CpSupportMergeVerify);
    assert_eq!(engine.submit(missing).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        let payload = serde_json::to_string(&document).unwrap();
        std::fs::write(directory.join("cp-support-merge.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn subproblem_result_entry_shows_the_four_state_gates() {
    use ospf_rust_bridge::{SubproblemResultReport, SubproblemResultRequest, SubproblemValueSpec};

    let engine = NativeEngine::default();
    let verify = |document: SubproblemResultRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::SubproblemResultVerify);
        request.subproblem_result = Some(document);
        engine.submit(request)
    };
    let run = |document: SubproblemResultRequest| {
        let view = verify(document).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: SubproblemResultReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        engine.release(&view.id).unwrap();
        report
    };
    let document = SubproblemResultRequest { variant: "feasible".to_owned(),
        status: Some("Feasible".to_owned()),
        assignment: vec![SubproblemValueSpec { id: "x".to_owned(), value: 3 }],
        with_solution: None, with_proof: None, proof_reference: None,
        conflict_members: Vec::new(), conflict_reference: None, incumbent_objective: None,
        issue_code: None, issue_message: None };

    let report = run(document.clone());
    assert!(report.accepted);
    assert_eq!(report.state.as_deref(), Some("Feasible"));
    assert_eq!(report.assignment_size, Some(1));
    assert!(!report.has_model_fingerprint);
    assert!(report.has_report);
    assert!(report.failed_never_validates_anything);
    assert!(report.only_the_snapshot_constructors_issue_proofs);
    assert!(report.feasible_demands_the_feasible_status);
    assert!(report.infeasible_demands_a_genuine_certificate);
    assert!(report.the_conflict_reference_must_match_the_report);
    assert!(report.incomplete_refuses_even_an_optimal_report);
    assert!(report.the_incomplete_incumbent_has_two_value_gates);
    assert!(report.the_feasible_assignment_is_a_copy_of_the_report);
    // snapshot 入口才发证明。 / Only the snapshot entry issues a proof.
    assert!(run(SubproblemResultRequest { variant: "feasibleFromSnapshot".to_owned(),
        ..document.clone() }).has_model_fingerprint);
    // `Unknown` 带完整解也被拒。 / `Unknown` with a full solution is still refused.
    let unknown = run(SubproblemResultRequest { status: Some("Unknown".to_owned()),
        ..document.clone() });
    assert!(!unknown.accepted);
    assert!(unknown.refusal.as_deref().unwrap().contains("feasible incumbent report"));
    // 不可行要证书，且引用必须逐字相等。 / Infeasible needs a certificate and a verbatim reference.
    let good = run(SubproblemResultRequest { variant: "infeasible".to_owned(),
        status: Some("Infeasible".to_owned()), with_solution: Some(false), with_proof: Some(true),
        ..document.clone() });
    assert!(good.accepted);
    let wrong = run(SubproblemResultRequest { variant: "infeasible".to_owned(),
        status: Some("Infeasible".to_owned()), with_solution: Some(false), with_proof: Some(true),
        proof_reference: Some("proof-1".to_owned()), conflict_members: vec!["x".to_owned()],
        conflict_reference: Some("proof-2".to_owned()), ..document.clone() });
    assert!(!wrong.accepted);
    // `incomplete` 拒终态、拒 `i64::MIN`、拒非有限 objective。 
    // `incomplete` refuses terminal states, `i64::MIN`, and a non-finite objective.
    let terminal = run(SubproblemResultRequest { variant: "incomplete".to_owned(),
        status: Some("Infeasible".to_owned()), with_solution: Some(false), ..document.clone() });
    assert!(!terminal.accepted);
    let floor = run(SubproblemResultRequest { variant: "incomplete".to_owned(),
        status: Some("Unknown".to_owned()),
        assignment: vec![SubproblemValueSpec { id: "x".to_owned(), value: i64::MIN }],
        ..document.clone() });
    assert!(!floor.accepted);
    // `failed` 什么都收。 / `failed` accepts anything.
    let failed = run(SubproblemResultRequest { variant: "failed".to_owned(),
        status: None, assignment: Vec::new(), issue_code: Some(String::new()),
        issue_message: Some(String::new()), ..document.clone() });
    assert!(failed.accepted);
    assert!(!failed.has_report);
    assert_eq!(failed.state.as_deref(), Some("Failed"));
    // 拒绝：未知变体 / 未知状态 / 条目过多 / 空 ID / 缺请求。 / Refusals.
    assert!(verify(SubproblemResultRequest { variant: "nope".to_owned(), ..document.clone() })
        .unwrap_err().message.contains("unknown variant"));
    assert!(verify(SubproblemResultRequest { status: Some("Maybe".to_owned()),
        ..document.clone() }).unwrap_err().message.contains("unknown status"));
    assert!(verify(SubproblemResultRequest { assignment: (0..17).map(|index|
        SubproblemValueSpec { id: format!("x{index}"), value: 0 }).collect(),
        ..document.clone() }).unwrap_err().message.contains("at most 16 entries"));
    assert!(verify(SubproblemResultRequest { assignment: vec![SubproblemValueSpec {
        id: "  ".to_owned(), value: 0 }], ..document.clone() }).unwrap_err().message
        .contains("stable IDs must not be blank"));
    let mut missing = base_request();
    missing.model_kind = Some(ModelKind::SubproblemResultVerify);
    assert_eq!(engine.submit(missing).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        let payload = serde_json::to_string(&document).unwrap();
        std::fs::write(directory.join("subproblem-result.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn no_good_oracle_entry_shows_the_emitted_cuts() {
    use ospf_rust_bridge::{NoGoodOracleReport, NoGoodOracleRequest, OracleAssignmentEntry,
        OracleBindingLiteral};

    let engine = NativeEngine::default();
    let verify = |document: NoGoodOracleRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::NoGoodOracleVerify);
        request.no_good_oracle = Some(document);
        engine.submit(request)
    };
    let run = |document: NoGoodOracleRequest| {
        let view = verify(document).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: NoGoodOracleReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        engine.release(&view.id).unwrap();
        report
    };
    let binary = NoGoodOracleRequest {
        bindings: vec![
            OracleBindingLiteral { id: "a".to_owned(), kind: Some("binary".to_owned()),
                lower: None, upper: None },
            OracleBindingLiteral { id: "b".to_owned(), kind: Some("binary".to_owned()),
                lower: None, upper: None }],
        assignment: vec![OracleAssignmentEntry { id: "a".to_owned(), value: 1 },
            OracleAssignmentEntry { id: "b".to_owned(), value: 0 }],
        conflict_members: Vec::new(), conflict_reference: None,
        report_reference: Some("proof-1".to_owned()), entry: None, context_reference: None,
        result_variant: None };

    // 二进制域一条 cut。 / One cut for binary domains.
    let report = run(binary.clone());
    assert!(report.accepted);
    assert_eq!(report.cut_count, 1);
    let cut = &report.cuts[0];
    assert_eq!(cut.encoding_kind, "binaryNoGood");
    assert_eq!(cut.sense, "GreaterOrEqual");
    assert_eq!(cut.rhs, 0.0);
    assert_eq!(cut.proof_status, "Verified");
    assert_eq!(cut.validity, "Global");
    assert_eq!(cut.proof_reference.as_deref(), Some("proof-1"));
    assert_eq!(report.proof_references, vec!["proof-1".to_owned()]);
    assert!(report.binary_domains_yield_a_single_no_good);
    assert!(report.integer_domains_yield_three_rows_per_variable_plus_one);
    assert!(report.every_row_declares_the_whole_auxiliary_set);
    assert!(report.an_empty_conflict_excludes_the_whole_assignment);
    assert!(report.a_conflict_outside_the_assignment_is_refused);
    assert!(report.a_missing_proof_reference_is_refused);
    assert!(report.the_context_entry_does_not_validate_its_reference);
    assert!(report.only_an_infeasible_result_reaches_the_oracle);

    // 有界整数域 3n+1 = 7 条，每行声明 4 个 auxiliary。 
    // Bounded integers yield 3n+1 = 7 rows, each declaring all 4 auxiliaries.
    let integer = run(NoGoodOracleRequest {
        bindings: vec![
            OracleBindingLiteral { id: "a".to_owned(), kind: Some("integer".to_owned()),
                lower: Some(0), upper: Some(4) },
            OracleBindingLiteral { id: "b".to_owned(), kind: Some("integer".to_owned()),
                lower: Some(0), upper: Some(4) }],
        ..binary.clone() });
    assert_eq!(integer.cut_count, 7);
    assert!(integer.cuts.iter().all(|cut| cut.auxiliary_count == 4));
    assert_eq!(integer.cuts.iter().filter(|cut| cut.row_kind.as_deref()
        == Some("requireDifference")).count(), 1);

    // 窄冲突缩小 no-good。 / A narrow conflict shrinks the no-good.
    let narrow = run(NoGoodOracleRequest { conflict_members: vec!["a".to_owned()],
        conflict_reference: Some("proof-1".to_owned()), ..binary.clone() });
    assert_eq!(narrow.cuts[0].coefficient_count, 1);

    // context 入口用自己的引用。 / The context entry uses its own reference.
    let context = run(NoGoodOracleRequest { entry: Some("withContext".to_owned()),
        context_reference: Some("attempt-1::anything".to_owned()), ..binary.clone() });
    assert_eq!(context.proof_references, vec!["attempt-1::anything".to_owned()]);
    // 空白引用仍被 `normalize` 拒。 / A blank reference is still refused by `normalize`.
    let blank = run(NoGoodOracleRequest { entry: Some("withContext".to_owned()),
        context_reference: Some(String::new()), ..binary.clone() });
    assert!(!blank.accepted);

    // 只有不可行结果能进 oracle。 / Only an infeasible result reaches the oracle.
    let feasible = run(NoGoodOracleRequest { result_variant: Some("feasible".to_owned()),
        ..binary.clone() });
    assert!(!feasible.accepted);
    assert!(feasible.refusal.as_deref().unwrap().contains("requires an infeasible subproblem"));
    let failed = run(NoGoodOracleRequest { result_variant: Some("failed".to_owned()),
        ..binary.clone() });
    assert!(!failed.accepted);

    // 拒绝：未知入口 / 未知结果态 / 未知绑定类型 / 重复 ID / 条目过多 / 缺请求。 / Refusals.
    assert!(verify(NoGoodOracleRequest { entry: Some("nope".to_owned()), ..binary.clone() })
        .unwrap_err().message.contains("unknown entry"));
    assert!(verify(NoGoodOracleRequest { result_variant: Some("nope".to_owned()),
        ..binary.clone() }).unwrap_err().message.contains("unknown result variant"));
    assert!(verify(NoGoodOracleRequest { bindings: vec![OracleBindingLiteral {
        id: "a".to_owned(), kind: Some("nope".to_owned()), lower: None, upper: None }],
        ..binary.clone() }).unwrap_err().message.contains("unknown binding kind"));
    assert!(verify(NoGoodOracleRequest { assignment: vec![
        OracleAssignmentEntry { id: "a".to_owned(), value: 0 },
        OracleAssignmentEntry { id: "a".to_owned(), value: 1 }], ..binary.clone() })
        .unwrap_err().message.contains("repeats a stable ID"));
    assert!(verify(NoGoodOracleRequest { assignment: (0..17).map(|index|
        OracleAssignmentEntry { id: format!("x{index}"), value: 0 }).collect(),
        ..binary.clone() }).unwrap_err().message.contains("at most 16 entries"));
    let mut missing = base_request();
    missing.model_kind = Some(ModelKind::NoGoodOracleVerify);
    assert_eq!(engine.submit(missing).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        let payload = serde_json::to_string(&binary).unwrap();
        std::fs::write(directory.join("no-good-oracle.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn cp_model_build_entry_shows_the_freeze_gates() {
    use ospf_rust_bridge::{CpConstraintSpec, CpGroupSpec, CpModelBuildReport, CpModelBuildRequest,
        CpVariableSpec};

    let engine = NativeEngine::default();
    let verify = |document: CpModelBuildRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::CpModelBuildVerify);
        request.cp_model_build = Some(document);
        engine.submit(request)
    };
    let run = |document: CpModelBuildRequest| {
        let view = verify(document).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: CpModelBuildReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        engine.release(&view.id).unwrap();
        report
    };
    let document = CpModelBuildRequest {
        name: Some("probe".to_owned()), namespace: None, schema_version: None,
        variables: vec![CpVariableSpec { id: "x".to_owned(), lower: Some(0), upper: Some(4),
            values: Vec::new(), share_local_id_with: None }],
        constraints: vec![CpConstraintSpec { id: "capacity".to_owned(),
            variable: Some("x".to_owned()), relation: Some("lessOrEqual".to_owned()),
            rhs: Some(4), group: Some("capacity".to_owned()), origin: None, name: None }],
        objective: Some("maximize".to_owned()), objective_id: Some("profit".to_owned()),
        objective_variable: Some("x".to_owned()), clear_objective: false,
        groups: vec![CpGroupSpec { id: 1, name: "capacity".to_owned() }] };

    // 最小模型：`new` 自带身份、name 由 id 预填、模型名不进指纹。 
    // The minimal model: `new` carries an identity, the name is prefilled, the model name stays out of the fingerprint.
    let report = run(document.clone());
    assert!(report.accepted, "{:?}", report.refusal);
    assert_eq!(report.name.as_deref(), Some("probe"));
    assert_eq!(report.namespace.as_deref(), Some("model-local"));
    assert_eq!(report.schema_version.as_deref(), Some("1.0"));
    assert_eq!(report.variable_ids, vec!["x".to_owned()]);
    assert_eq!(report.variable_local_ids, vec!["0:0".to_owned()]);
    assert_eq!(report.variable_domains, vec!["range[0,4]".to_owned()]);
    assert_eq!(report.constraint_names, vec!["capacity".to_owned()]);
    assert_eq!(report.objective_id, "profit");
    assert_eq!(report.objective_category, "maximum");
    assert_eq!(report.groups, vec!["1:capacity".to_owned()]);
    assert!(report.fingerprint.is_some());
    assert!(report.a_new_model_already_has_an_identity);
    assert!(report.register_variable_refuses_two_kinds_of_duplicate);
    assert!(report.a_new_constraint_names_itself_after_its_id);
    assert!(report.set_objective_silently_overwrites);
    assert!(report.freeze_rebinds_local_ids_to_canonical_indices);
    assert!(report.the_model_name_is_not_in_the_fingerprint);
    assert!(report.a_group_id_may_not_be_rebound);
    assert!(report.freeze_is_the_reference_integrity_gate);
    // 只差名字的两个模型指纹相同。 / Two models differing only in name share one fingerprint.
    let renamed = run(CpModelBuildRequest { name: Some("other".to_owned()), ..document.clone() });
    assert_eq!(renamed.fingerprint, report.fingerprint);
    assert_ne!(renamed.name, report.name);

    // 反序注册：本地 ID 按稳定 ID 重排。 / Reverse registration: local IDs follow stable-ID order.
    let rebound = run(CpModelBuildRequest {
        variables: vec![
            CpVariableSpec { id: "b".to_owned(), lower: Some(0), upper: Some(4),
                values: Vec::new(), share_local_id_with: None },
            CpVariableSpec { id: "a".to_owned(), lower: Some(0), upper: Some(4),
                values: Vec::new(), share_local_id_with: None }],
        constraints: Vec::new(), objective: None, objective_id: None,
        objective_variable: None, groups: Vec::new(), ..document.clone() });
    assert_eq!(rebound.variable_ids, vec!["a".to_owned(), "b".to_owned()]);
    assert_eq!(rebound.variable_local_ids, vec!["0:0".to_owned(), "1:0".to_owned()]);

    // 身份置空 / 目标引用未注册变量 → 冻结阶段拒。 
    // A blank identity and a dangling objective both die at freeze.
    let blank = run(CpModelBuildRequest { namespace: Some("  ".to_owned()), ..document.clone() });
    assert!(!blank.accepted);
    assert_eq!(blank.refusal_stage.as_deref(), Some("freeze"));
    let dangling = run(CpModelBuildRequest { objective: Some("maximize".to_owned()),
        objective_variable: Some("ghost".to_owned()), ..document.clone() });
    assert!(!dangling.accepted);
    assert_eq!(dangling.refusal_stage.as_deref(), Some("freeze"));

    // 重复本地 ID / 组改绑 → 更早的阶段拒。 / A duplicate local ID and a rebound group die earlier.
    let duplicate = run(CpModelBuildRequest {
        variables: vec![
            CpVariableSpec { id: "x".to_owned(), lower: Some(0), upper: Some(4),
                values: Vec::new(), share_local_id_with: None },
            CpVariableSpec { id: "y".to_owned(), lower: Some(0), upper: Some(4),
                values: Vec::new(), share_local_id_with: Some("x".to_owned()) }],
        constraints: Vec::new(), objective: None, groups: Vec::new(), ..document.clone() });
    assert!(!duplicate.accepted);
    assert_eq!(duplicate.refusal_stage.as_deref(), Some("registerVariable"));
    let group = run(CpModelBuildRequest { groups: vec![
        CpGroupSpec { id: 1, name: "capacity".to_owned() },
        CpGroupSpec { id: 1, name: "renamed".to_owned() }], ..document.clone() });
    assert!(!group.accepted);
    assert_eq!(group.refusal_stage.as_deref(), Some("group"));

    // 拒绝：空 ID / 未知关系 / 未知目标方向 / 共享不存在的变量 / 条目过多 / 缺请求。 / Refusals.
    assert!(verify(CpModelBuildRequest { variables: vec![CpVariableSpec { id: "  ".to_owned(),
        lower: None, upper: None, values: Vec::new(), share_local_id_with: None }],
        ..document.clone() }).unwrap_err().message.contains("stable variable IDs must not be blank"));
    assert!(verify(CpModelBuildRequest { constraints: vec![CpConstraintSpec {
        id: "c".to_owned(), variable: None, relation: Some("nope".to_owned()), rhs: None,
        group: None, origin: None, name: None }], ..document.clone() })
        .unwrap_err().message.contains("unknown relation"));
    assert!(verify(CpModelBuildRequest { objective: Some("nope".to_owned()),
        ..document.clone() }).unwrap_err().message.contains("unknown objective direction"));
    assert!(verify(CpModelBuildRequest { variables: vec![CpVariableSpec { id: "x".to_owned(),
        lower: None, upper: None, values: Vec::new(),
        share_local_id_with: Some("missing".to_owned()) }], ..document.clone() })
        .unwrap_err().message.contains("no variable `missing` to share"));
    assert!(verify(CpModelBuildRequest { variables: (0..17).map(|index| CpVariableSpec {
        id: format!("x{index}"), lower: Some(0), upper: Some(1), values: Vec::new(),
        share_local_id_with: None }).collect(), ..document.clone() }).unwrap_err().message
        .contains("at most 16 entries"));
    let mut missing = base_request();
    missing.model_kind = Some(ModelKind::CpModelBuildVerify);
    assert_eq!(engine.submit(missing).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        let payload = serde_json::to_string(&document).unwrap();
        std::fs::write(directory.join("cp-model-build.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn snapshot_gate_entry_shows_the_read_side_checks() {
    use ospf_rust_bridge::{SnapshotGateReport, SnapshotGateRequest, SnapshotValueSpec};

    let engine = NativeEngine::default();
    let verify = |document: SnapshotGateRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::SnapshotGateVerify);
        request.snapshot_gate = Some(document);
        engine.submit(request)
    };
    let run = |document: SnapshotGateRequest| {
        let view = verify(document).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: SnapshotGateReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        engine.release(&view.id).unwrap();
        report
    };
    let plain = SnapshotGateRequest { mutations: Vec::new(), hint: Vec::new(),
        evaluation: Vec::new() };

    // 原样快照：校验通过、形状可读、artifact 往返。 
    // The untouched snapshot: it validates, its shape is readable, and the artifact round-trips.
    let report = run(plain.clone());
    assert!(report.identity_accepted, "{:?}", report.identity_refusal);
    assert!(report.identity_is_idempotent);
    assert_eq!(report.model_name, "snapshot-gate");
    assert_eq!(report.variable_ids, vec!["x".to_owned(), "y".to_owned()]);
    assert_eq!(report.constraint_ids, vec!["capacity".to_owned(), "floor".to_owned()]);
    assert_eq!(report.groups, vec!["1:capacity".to_owned()]);
    assert_eq!(report.objective_variables, vec!["x".to_owned()]);
    assert!(report.artifact_encoded);
    assert!(report.artifact_round_trips);
    assert_eq!(report.artifact_digest.as_deref().map(str::len), Some(64));
    assert_eq!(report.hint_accepted, None);
    assert_eq!(report.objective_value, None);
    assert!(report.a_frozen_snapshot_always_validates);
    assert!(report.a_blank_identity_is_the_first_gate);
    assert!(report.the_collection_order_gate_shadows_the_sort_gate);
    assert!(report.the_model_name_is_outside_the_fingerprint_but_constraint_names_are_inside);
    assert!(report.a_blank_group_name_is_refused_before_the_fingerprint);
    assert!(report.a_hint_may_cover_less_than_every_variable);
    assert!(report.objective_variables_ignore_constraints);
    assert!(report.the_artifact_codec_checks_three_things);

    // 空白身份先报 namespace。 / A blank identity reports the namespace first.
    let blank = run(SnapshotGateRequest { mutations: vec!["blankNamespace".to_owned()],
        ..plain.clone() });
    assert!(!blank.identity_accepted);
    assert!(blank.identity_refusal.as_deref().unwrap().contains("namespace must not be blank"));
    assert!(!blank.artifact_encoded);

    // 打乱顺序只撞整结构闸门。 / Shuffling only ever hits the whole-structure gate.
    let shuffled = run(SnapshotGateRequest { mutations: vec!["reverseVariables".to_owned()],
        ..plain.clone() });
    let message = shuffled.identity_refusal.unwrap();
    assert!(message.contains("non-canonical collection order"), "{message}");
    assert!(!message.contains("not in canonical stable-ID order"), "{message}");
    assert_eq!(shuffled.variable_ids, vec!["y".to_owned(), "x".to_owned()]);

    // 改模型名合法，改约束名不合法。 / Renaming the model is legal; renaming a constraint is not.
    assert!(run(SnapshotGateRequest { mutations: vec!["renameModel".to_owned()],
        ..plain.clone() }).identity_accepted);
    let renamed = run(SnapshotGateRequest { mutations: vec!["renameConstraint".to_owned()],
        ..plain.clone() });
    assert!(renamed.identity_refusal.as_deref().unwrap()
        .contains("fingerprint does not match canonical content"));

    // 空白组名先于指纹。 / A blank group name precedes the fingerprint.
    let group = run(SnapshotGateRequest { mutations: vec!["blankGroupName".to_owned()],
        ..plain.clone() });
    assert!(group.identity_refusal.as_deref().unwrap()
        .contains("group name must not be blank"));

    // hint 是部分赋值闸门。 / A hint is a partial-assignment gate.
    let hint = run(SnapshotGateRequest { hint: vec![SnapshotValueSpec { id: "x".to_owned(),
        value: 2 }], ..plain.clone() });
    assert_eq!(hint.hint_accepted, Some(true));
    let outside = run(SnapshotGateRequest { hint: vec![SnapshotValueSpec { id: "x".to_owned(),
        value: 9 }], ..plain.clone() });
    assert_eq!(outside.hint_accepted, Some(false));
    assert!(outside.hint_refusal.as_deref().unwrap().contains("outside the variable domain"));

    // `objective_value` 只求目标表达式。 / `objective_value` only evaluates the objective.
    let value = run(SnapshotGateRequest { evaluation: vec![
        SnapshotValueSpec { id: "x".to_owned(), value: 3 },
        SnapshotValueSpec { id: "y".to_owned(), value: 1 }], ..plain.clone() });
    assert_eq!(value.objective_value, Some(3));
    let missing = run(SnapshotGateRequest { evaluation: vec![SnapshotValueSpec {
        id: "y".to_owned(), value: 1 }], ..plain.clone() });
    assert!(missing.objective_value_refusal.as_deref().unwrap().contains("missing assignment"));

    // 重复稳定 ID / 掉变量 / 空稳定 ID 都在校验层拒。 / Duplicates, drops and blanks die at validation.
    for mutation in ["duplicateVariableStableId", "dropVariable", "blankStableId"] {
        let refused = run(SnapshotGateRequest { mutations: vec![mutation.to_owned()],
            ..plain.clone() });
        assert!(!refused.identity_accepted, "{mutation}");
        assert!(!refused.artifact_encoded, "{mutation}");
    }

    // 拒绝：未知篡改 / 空赋值 ID / 重复 ID / 条目过多 / 缺请求。 / Refusals.
    assert!(verify(SnapshotGateRequest { mutations: vec!["nope".to_owned()], ..plain.clone() })
        .unwrap_err().message.contains("unknown mutation"));
    assert!(verify(SnapshotGateRequest { hint: vec![SnapshotValueSpec { id: "  ".to_owned(),
        value: 0 }], ..plain.clone() }).unwrap_err().message
        .contains("assignment IDs must not be blank"));
    assert!(verify(SnapshotGateRequest { hint: vec![
        SnapshotValueSpec { id: "x".to_owned(), value: 0 },
        SnapshotValueSpec { id: "x".to_owned(), value: 1 }], ..plain.clone() })
        .unwrap_err().message.contains("repeats a stable ID"));
    assert!(verify(SnapshotGateRequest { mutations: (0..17).map(|index| format!("m{index}"))
        .collect(), ..plain.clone() }).unwrap_err().message.contains("at most 16 entries"));
    let mut missing = base_request();
    missing.model_kind = Some(ModelKind::SnapshotGateVerify);
    assert_eq!(engine.submit(missing).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        let payload = serde_json::to_string(&SnapshotGateRequest {
            evaluation: vec![SnapshotValueSpec { id: "x".to_owned(), value: 3 },
                SnapshotValueSpec { id: "y".to_owned(), value: 1 }],
            hint: vec![SnapshotValueSpec { id: "x".to_owned(), value: 2 }],
            ..plain }).unwrap();
        std::fs::write(directory.join("snapshot-gate.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn constraint_canon_entry_shows_which_variants_get_sorted() {
    use ospf_rust_bridge::{ConstraintCanonReport, ConstraintCanonRequest, ConstraintShapeSpec};

    let engine = NativeEngine::default();
    let verify = |document: ConstraintCanonRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::ConstraintCanonVerify);
        request.constraint_canon = Some(document);
        engine.submit(request)
    };
    let run = |document: ConstraintCanonRequest| {
        let view = verify(document).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: ConstraintCanonReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        engine.release(&view.id).unwrap();
        report
    };
    let shape = |kind: &str, items: &[&str]| ConstraintShapeSpec { kind: kind.to_owned(),
        items: items.iter().map(|item| (*item).to_owned()).collect() };
    let document = ConstraintCanonRequest {
        constraints: vec![shape("and", &["b", "!a", "a"]), shape("circuit", &["n1", "n0"])],
        reverse_items: false };

    // 布尔组合子排序（键是 `(稳定 ID, 取反)`），`circuit` 保持原序。 
    // The boolean combinator sorts by `(stable ID, negated)`; `circuit` keeps its order.
    let report = run(document.clone());
    assert!(report.accepted, "{:?}", report.refusal);
    assert_eq!(report.constraints[0].kind, "and");
    assert_eq!(report.constraints[0].items, vec!["a".to_owned(), "!a".to_owned(), "b".to_owned()]);
    assert_eq!(report.constraints[1].kind, "circuit");
    assert_eq!(report.constraints[1].items, vec!["n1".to_owned(), "n0".to_owned()]);
    assert!(report.the_boolean_combinators_sort_their_literals);
    assert!(report.the_table_constraints_sort_tuples_not_expressions);
    assert!(report.cumulative_sorts_by_interval_and_its_demand_tiebreak_is_unreachable);
    assert!(report.the_automaton_sorts_its_transitions_only);
    assert!(report.the_reservoir_sorts_its_events);
    assert!(report.the_untouched_variants_keep_their_order);
    assert!(report.it_never_dedups_and_never_simplifies);
    assert!(report.the_fingerprint_is_order_insensitive_exactly_where_sorting_happens);
    let fingerprint = report.fingerprint.clone();

    // 反序输入：布尔那半给出同样指纹，`circuit` 那半给出不同指纹。 
    // A reversed input: the boolean half keeps its fingerprint, the circuit half changes it.
    let reversed = run(ConstraintCanonRequest { reverse_items: true, ..document.clone() });
    assert_eq!(reversed.constraints[0].items,
        vec!["a".to_owned(), "!a".to_owned(), "b".to_owned()]);
    assert_eq!(reversed.constraints[1].items, vec!["n0".to_owned(), "n1".to_owned()]);
    assert_ne!(reversed.fingerprint, fingerprint);

    // 只有布尔时指纹不变。 / With only the boolean constraint the fingerprint is stable.
    let boolean_only = run(ConstraintCanonRequest { constraints: vec![shape("and", &["b", "a"])],
        reverse_items: false });
    let boolean_reversed = run(ConstraintCanonRequest {
        constraints: vec![shape("and", &["b", "a"])], reverse_items: true });
    assert_eq!(boolean_only.fingerprint, boolean_reversed.fingerprint);

    // 各变体各一条，确认排序位置。 / One of each variant to confirm where sorting lands.
    for (spec, expected) in [
        (shape("allDifferent", &["b", "a"]), vec!["a".to_owned(), "b".to_owned()]),
        (shape("allowedAssignments", &["3", "1", "2"]),
            vec!["1".to_owned(), "2".to_owned(), "3".to_owned()]),
        (shape("noOverlap", &["ivB", "ivA"]), vec!["ivA".to_owned(), "ivB".to_owned()]),
        (shape("cumulative", &["ivC:5", "ivA:1", "ivB:3"]),
            vec!["ivA:1".to_owned(), "ivB:3".to_owned(), "ivC:5".to_owned()]),
        (shape("automaton", &["1:2:1", "0:1:1"]),
            vec!["0:1:1".to_owned(), "1:2:1".to_owned()]),
        (shape("reservoir", &["3:1", "1:2"]), vec!["1:2".to_owned(), "3:1".to_owned()]),
    ] {
        let single = run(ConstraintCanonRequest { constraints: vec![spec.clone()],
            reverse_items: false });
        assert!(single.accepted, "{:?}: {:?}", spec.kind, single.refusal);
        assert_eq!(single.constraints[0].items, expected, "{}", spec.kind);
    }

    // 重复 literal 留着、单 literal 的 `and` 仍是 `and`。 
    // Duplicate literals survive and a single-literal `and` stays `and`.
    let duplicates = run(ConstraintCanonRequest {
        constraints: vec![shape("and", &["a", "a", "!a"])], reverse_items: false });
    assert_eq!(duplicates.constraints[0].items, vec!["a".to_owned(), "a".to_owned(),
        "!a".to_owned()]);
    let single = run(ConstraintCanonRequest { constraints: vec![shape("and", &["a"])],
        reverse_items: false });
    assert_eq!(single.constraints[0].kind, "and");

    // 领域拒（空 automaton / 空 circuit / 同区间两次 cumulative）表现为被拒结论。 
    // Domain refusals (empty automaton / empty circuit / duplicate cumulative interval) are refused verdicts.
    for spec in [shape("automaton", &[]), shape("circuit", &[]),
        shape("cumulative", &["ivA:1", "ivA:3"])] {
        let refused = run(ConstraintCanonRequest { constraints: vec![spec.clone()],
            reverse_items: false });
        assert!(!refused.accepted, "{}", spec.kind);
        assert!(refused.fingerprint.is_none(), "{}", spec.kind);
    }
    // 形状错误同样是被拒结论。 / Shape errors are refused verdicts too.
    let unknown = run(ConstraintCanonRequest { constraints: vec![shape("nope", &[])],
        reverse_items: false });
    assert!(!unknown.accepted);
    assert!(unknown.refusal.as_deref().unwrap().contains("unknown kind"));

    // 只有条目上限是 `Err`。 / Only the entry cap is an `Err`.
    assert!(verify(ConstraintCanonRequest {
        constraints: (0..17).map(|index| shape("and", &[&format!("v{index}")])).collect(),
        reverse_items: false }).unwrap_err().message.contains("at most 16 constraints"));
    let mut missing = base_request();
    missing.model_kind = Some(ModelKind::ConstraintCanonVerify);
    assert_eq!(engine.submit(missing).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        let payload = serde_json::to_string(&document).unwrap();
        std::fs::write(directory.join("constraint-canon.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn assignment_eval_entry_shows_the_evaluation_order() {
    use ospf_rust_bridge::{AssignmentEvalReport, AssignmentEvalRequest};

    let engine = NativeEngine::default();
    let verify = |document: AssignmentEvalRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::AssignmentEvalVerify);
        request.assignment_eval = Some(document);
        engine.submit(request)
    };
    let run = |document: AssignmentEvalRequest| {
        let view = verify(document).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: AssignmentEvalReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        engine.release(&view.id).unwrap();
        report
    };
    let plain = AssignmentEvalRequest { s1: None, s2: None, duration: None, presence: None,
        capacity: None, extra: false, drop: false, break_end: false, presence_domain: None,
        mandatory: false };

    // 默认赋值被接受：两段不重叠、各 2 长。 / The default assignment is accepted: two disjoint intervals of length 2.
    let report = run(plain.clone());
    assert!(report.accepted, "{:?}", report.refusal);
    assert_eq!(report.refusal_layer, None);
    assert_eq!(report.intervals.len(), 2);
    assert_eq!(report.intervals[0].id, "ivA");
    assert_eq!(report.intervals[0].start, 0);
    assert_eq!(report.intervals[0].end, 2);
    assert_eq!(report.intervals[0].duration, 2);
    assert_eq!(report.intervals[1].id, "ivB");
    assert_eq!(report.intervals[1].start, 2);
    assert!(report.the_assignment_must_cover_exactly_every_variable);
    assert!(report.the_domain_check_runs_before_any_interval);
    assert!(report.an_interval_end_must_equal_start_plus_duration);
    assert!(report.an_absent_optional_interval_is_skipped_by_every_constraint);
    assert!(report.cumulative_measures_load_at_every_boundary);
    assert!(report.a_violation_names_the_first_constraint_in_canonical_id_order);
    assert!(report.the_return_value_is_the_interval_value_map);
    assert!(report.a_presence_variable_must_have_the_sparse_boolean_domain);

    // 少给与多给同文案。 / Too few and too many share one message.
    let missing = run(AssignmentEvalRequest { drop: true, ..plain.clone() });
    assert_eq!(missing.refusal_layer.as_deref(), Some("coverage"));
    let extra = run(AssignmentEvalRequest { extra: true, ..plain.clone() });
    assert_eq!(extra.refusal_layer.as_deref(), Some("coverage"));
    assert_eq!(missing.refusal, extra.refusal);
    assert!(missing.refusal.as_deref().unwrap()
        .contains("does not cover every registered variable"));

    // 域先于区间。 / Domain before interval.
    let domain = run(AssignmentEvalRequest { s1: Some(99), break_end: true, ..plain.clone() });
    assert_eq!(domain.refusal_layer.as_deref(), Some("domain"));
    assert!(domain.refusal.as_deref().unwrap().contains("outside its domain"));

    // 区间端点是错误而不是"不满足"。 / A broken end is an error, not "unsatisfied".
    let interval = run(AssignmentEvalRequest { break_end: true, ..plain.clone() });
    assert_eq!(interval.refusal_layer.as_deref(), Some("interval"));
    assert!(interval.refusal.as_deref().unwrap()
        .contains("end does not equal start plus duration"));
    assert!(interval.intervals.is_empty());

    // 缺席可选区间被跳过：presence=0 + 完全重叠 → 通过且只返回 ivA。 
    // An absent optional interval is skipped: presence=0 with full overlap passes and returns only ivA.
    let absent = run(AssignmentEvalRequest { presence: Some(0), s1: Some(0), s2: Some(0),
        ..plain.clone() });
    assert!(absent.accepted, "{:?}", absent.refusal);
    assert_eq!(absent.intervals.len(), 1);
    // presence=1 时同一份赋值被 `cCumulative` 点名（容量 5 < 6）。 
    // With presence=1 the same assignment is named by `cCumulative` (capacity 5 < 6).
    let present = run(AssignmentEvalRequest { presence: Some(1), s1: Some(0), s2: Some(0),
        ..plain.clone() });
    assert_eq!(present.refusal_layer.as_deref(), Some("constraint"));
    assert!(present.refusal.as_deref().unwrap().contains("violates constraint cCumulative"));

    // 容量 6 时负载够，被点名的是 `cNoOverlap`。 
    // At capacity 6 the load fits and `cNoOverlap` is the one named.
    let overlap = run(AssignmentEvalRequest { presence: Some(1), s1: Some(0), s2: Some(0),
        capacity: Some(6), ..plain.clone() });
    assert!(overlap.refusal.as_deref().unwrap().contains("cNoOverlap"));

    // 不重叠时容量 5 够。 / Without overlap capacity 5 suffices.
    let disjoint = run(AssignmentEvalRequest { s1: Some(0), s2: Some(5), capacity: Some(5),
        ..plain.clone() });
    assert!(disjoint.accepted, "{:?}", disjoint.refusal);

    // presence 用区间域 `0..=1` 会在冻结阶段被拒。 / A range presence domain dies at freeze.
    let range = run(AssignmentEvalRequest { presence_domain: Some("range".to_owned()),
        ..plain.clone() });
    assert_eq!(range.refusal_layer.as_deref(), Some("freeze"));
    assert!(range.refusal.as_deref().unwrap().contains("is not Boolean"));

    // mandatory 时 presence 不被看。 / With a mandatory interval the presence is ignored.
    let mandatory = run(AssignmentEvalRequest { mandatory: true, presence: Some(0), s1: Some(0),
        s2: Some(0), ..plain.clone() });
    assert_eq!(mandatory.refusal_layer.as_deref(), Some("constraint"));

    // 拒绝：负时长 / 缺请求。 / Refusals: a negative duration and a missing request.
    assert!(verify(AssignmentEvalRequest { duration: Some(-1), ..plain.clone() }).unwrap_err()
        .message.contains("must not be negative"));
    let mut missing_request = base_request();
    missing_request.model_kind = Some(ModelKind::AssignmentEvalVerify);
    assert_eq!(engine.submit(missing_request).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        let payload = serde_json::to_string(&plain).unwrap();
        std::fs::write(directory.join("assignment-eval.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn expression_bounds_entry_shows_a_hull_not_a_value_set() {
    use ospf_rust_bridge::{AssignmentValueSpec, ExpressionBoundsReport, ExpressionBoundsRequest,
        ExpressionDomainSpec, ExpressionTermSpec};

    let engine = NativeEngine::default();
    let verify = |document: ExpressionBoundsRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::ExpressionBoundsVerify);
        request.expression_bounds = Some(document);
        engine.submit(request)
    };
    let run = |document: ExpressionBoundsRequest| {
        let view = verify(document).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: ExpressionBoundsReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        engine.release(&view.id).unwrap();
        report
    };
    let term = |id: &str, coefficient: i64| ExpressionTermSpec { id: id.to_owned(), coefficient };
    let range = |id: &str, lower: i64, upper: i64| ExpressionDomainSpec { id: id.to_owned(),
        lower: Some(lower), upper: Some(upper), values: Vec::new() };
    let sparse = |id: &str, values: &[i64]| ExpressionDomainSpec { id: id.to_owned(), lower: None,
        upper: None, values: values.to_vec() };
    let document = ExpressionBoundsRequest { constant: 3,
        terms: vec![term("x", -2), term("y", 1)],
        domains: vec![sparse("x", &[0, 4, 9]), range("y", -3, 2)], assignment: Vec::new() };

    // 稀疏域 `{0,4,9}`：界是包络 `[-18,5]`，中间值不可达。 
    // The sparse `{0,4,9}`: the bounds are the hull `[-18,5]` and the gaps are unreachable.
    let report = run(document.clone());
    assert!(report.accepted, "{:?}", report.refusal);
    assert_eq!(report.constant, 3);
    assert_eq!(report.terms.len(), 2);
    assert_eq!(report.terms[0].id, "x");
    assert_eq!(report.terms[0].coefficient, -2);
    assert_eq!((report.lower, report.upper), (Some(-18), Some(5)));
    assert_eq!(report.evaluated, None);
    assert!(report.terms_are_merged_sorted_and_zero_free);
    assert!(report.a_negative_coefficient_flips_the_interval_ends);
    assert!(report.bounds_read_only_the_domain_hull);
    assert!(report.a_constant_only_expression_never_touches_a_domain);
    assert!(report.a_missing_domain_is_an_error_not_a_bound);
    assert!(report.the_multiplication_gate_is_unreachable_while_the_other_two_are_not);
    assert!(report.evaluate_is_exact_and_never_consults_a_domain);
    assert!(report.the_bounds_always_contain_every_reachable_value);

    // 域外赋值照样能算（精确求值不看域）：`x = 5` 不在 `{0,4,9}` 里，`3 - 10 + 0 = -7`。 
    // An out-of-domain value still evaluates: `x = 5` is not in `{0,4,9}` and `3 - 10 + 0 = -7`.
    let evaluated = run(ExpressionBoundsRequest { assignment: vec![
        AssignmentValueSpec { id: "x".to_owned(), value: 5 },
        AssignmentValueSpec { id: "y".to_owned(), value: 0 }], ..document.clone() });
    assert_eq!(evaluated.evaluated, Some(-7));
    assert_eq!((evaluated.lower, evaluated.upper), (Some(-18), Some(5)));

    // 合并、排序、丢零和。 / Merge, sort, drop zero sums.
    let merged = run(ExpressionBoundsRequest { constant: 1,
        terms: vec![term("b", 2), term("a", 3), term("b", -2)],
        domains: vec![range("a", 0, 1), range("b", 0, 1)], assignment: Vec::new() });
    assert_eq!(merged.terms.len(), 1);
    assert_eq!(merged.terms[0].id, "a");
    assert_eq!(merged.terms[0].coefficient, 3);
    assert_eq!((merged.lower, merged.upper), (Some(1), Some(4)));

    // 负系数翻转两端。 / A negative coefficient flips the ends.
    let negative = run(ExpressionBoundsRequest { constant: 0, terms: vec![term("x", -2)],
        domains: vec![range("x", 1, 5)], assignment: Vec::new() });
    assert_eq!((negative.lower, negative.upper), (Some(-10), Some(-2)));

    // 纯常量从不碰域。 / A constant-only expression never touches a domain.
    let constant = run(ExpressionBoundsRequest { constant: 7, terms: Vec::new(),
        domains: Vec::new(), assignment: Vec::new() });
    assert!(constant.accepted);
    assert_eq!((constant.lower, constant.upper), (Some(7), Some(7)));

    // 缺域是错误而不是界。 / A missing domain is an error, not a bound.
    let missing = run(ExpressionBoundsRequest { constant: 0, terms: vec![term("ghost", 1)],
        domains: Vec::new(), assignment: Vec::new() });
    assert!(!missing.accepted);
    assert!(missing.refusal.as_deref().unwrap()
        .contains("missing domain for variable ghost"));
    assert_eq!((missing.lower, missing.upper), (None, None));

    // 导出闸门与累加闸门可达，乘法闸门不可达。 
    // The export and accumulation gates are reachable, the multiplication gate is not.
    let export = run(ExpressionBoundsRequest { constant: 0, terms: vec![term("x", i64::MAX)],
        domains: vec![range("x", 0, i64::MAX)], assignment: Vec::new() });
    assert!(export.refusal.as_deref().unwrap().contains("upper bound exceeds i64"));
    let accumulate = run(ExpressionBoundsRequest { constant: 0,
        terms: vec![term("a", i64::MAX), term("b", i64::MAX), term("c", i64::MAX)],
        domains: vec![range("a", 0, i64::MAX), range("b", 0, i64::MAX),
            range("c", 0, i64::MAX)], assignment: Vec::new() });
    assert!(accumulate.refusal.as_deref().unwrap().contains("upper bound overflow"));
    for message in [export.refusal.unwrap(), accumulate.refusal.unwrap()] {
        assert!(!message.contains("multiplication"), "{message}");
    }

    // 拒绝：空 ID、缺界、倒置区间、项数上限、缺赋值变量、缺请求。 
    // Refusals: blank IDs, a missing bound, an inverted range, the term cap, a missing assignment variable and a
    // missing request.
    assert!(verify(ExpressionBoundsRequest { constant: 0, terms: vec![term(" ", 1)],
        domains: Vec::new(), assignment: Vec::new() }).unwrap_err().message
        .contains("term IDs must not be blank"));
    assert!(verify(ExpressionBoundsRequest { constant: 0, terms: Vec::new(),
        domains: vec![ExpressionDomainSpec { id: "x".to_owned(), lower: None, upper: Some(1),
            values: Vec::new() }], assignment: Vec::new() }).unwrap_err().message
        .contains("needs a lower bound"));
    assert!(verify(ExpressionBoundsRequest { constant: 0, terms: Vec::new(),
        domains: vec![range("x", 5, 1)], assignment: Vec::new() }).unwrap_err().message
        .contains("lower bound exceeds upper bound"));
    assert!(verify(ExpressionBoundsRequest { constant: 0,
        terms: (0..65).map(|index| term(&format!("v{index}"), 1)).collect(), domains: Vec::new(),
        assignment: Vec::new() }).unwrap_err().message.contains("at most 64 terms"));
    assert!(verify(ExpressionBoundsRequest { constant: 0,
        terms: vec![term("x", 1), term("y", 1)], domains: vec![range("x", 0, 1)],
        assignment: vec![AssignmentValueSpec { id: "x".to_owned(), value: 1 }] }).unwrap_err()
        .message.contains("missing assignment for variable y"));
    let mut missing_request = base_request();
    missing_request.model_kind = Some(ModelKind::ExpressionBoundsVerify);
    assert_eq!(engine.submit(missing_request).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        let payload = serde_json::to_string(&document).unwrap();
        std::fs::write(directory.join("expression-bounds.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn domain_surface_entry_shows_cardinality_without_materializing() {
    use ospf_rust_bridge::{DomainProbeReport, DomainProbeRequest, DomainProbeSpec};

    let engine = NativeEngine::default();
    let verify = |document: DomainProbeRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::DomainSurfaceVerify);
        request.domain_surface = Some(document);
        engine.submit(request)
    };
    let run = |document: DomainProbeRequest| {
        let view = verify(document).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: DomainProbeReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        engine.release(&view.id).unwrap();
        report
    };
    let range = |id: &str, lower: i64, upper: i64| DomainProbeSpec { id: id.to_owned(),
        lower: Some(lower), upper: Some(upper), values: Vec::new(), raw: false };
    let sparse = |id: &str, values: &[i64]| DomainProbeSpec { id: id.to_owned(), lower: None,
        upper: None, values: values.to_vec(), raw: false };
    // **按实测**：`i64::MIN` 过不了 JSON 往返（JS 侧 `JSON.stringify` 会写成 `-9.22e18`），所以
    // "整个 `i64` 区间"这一项不进 fixture，只在原生侧单独跑一次。
    // **Measured**: `i64::MIN` cannot survive a JSON round-trip (JS `JSON.stringify` writes `-9.22e18`), so the full
    // `i64` range stays out of the fixture and runs natively only.
    let document = DomainProbeRequest {
        domains: vec![sparse("s", &[3, 1, 3, -2, 1]), range("w", 0, 1_000_000),
            range("r", 0, 1), sparse("b", &[0, 1])],
        maximum_amount: Some(8), probes: vec![-2, 0, 1, 5] };

    // 稀疏域排序去重；宽区间只报基数不物化；整个 `i64` 区间的基数是 `2^64`。 
    // The sparse domain sorts and dedups; a wide range reports only its cardinality; the full `i64` range is `2^64`.
    let report = run(document.clone());
    assert_eq!(report.domains.len(), 4);
    assert_eq!(report.domains[0].enumerated, vec![-2, 1, 3]);
    assert_eq!(report.domains[0].cardinality.as_deref(), Some("3"));
    assert_eq!(report.domains[1].cardinality.as_deref(), Some("1000001"));
    assert!(report.domains[1].enumerated.is_empty());
    assert!(report.domains[1].enumeration_refusal.as_deref().unwrap()
        .contains("exceeds enumeration limit"));
    // `Range(0,1)` 不是布尔，`Values([0,1])` 是。 / `Range(0,1)` is not Boolean, `Values([0,1])` is.
    assert_eq!(report.domains[2].is_boolean, Some(false));
    assert_eq!(report.domains[3].is_boolean, Some(true));
    assert_eq!(report.domains[2].enumerated, report.domains[3].enumerated);
    assert_ne!(report.domains[2].kind, report.domains[3].kind);
    // 成员判定看空洞：稀疏 `{-2,1,3}` 不含 0 与 5，区间 `[0,10^6]` 含 0。 
    // Membership sees the gaps: the sparse `{-2,1,3}` holds neither 0 nor 5 while the range holds 0.
    assert_eq!(report.domains[0].contains, vec![true, false, true, false]);
    assert_eq!(report.domains[1].contains, vec![false, true, true, true]);

    // 整个 `i64` 区间的基数是 `2^64`——只在原生侧量（JSON 往返会毁掉 `i64::MIN`）。 
    // The full `i64` range is `2^64` — measured natively only, since a JSON round-trip destroys `i64::MIN`.
    let full = run(DomainProbeRequest { domains: vec![range("full", i64::MIN, i64::MAX)],
        maximum_amount: Some(8), probes: vec![i64::MIN, 0, i64::MAX] });
    assert_eq!(full.domains[0].cardinality.as_deref(), Some("18446744073709551616"));
    assert_eq!(full.domains[0].contains, vec![true, true, true]);
    assert!(full.domains[0].enumeration_refusal.is_some());
    assert!(report.values_are_sorted_and_deduped_at_construction);
    assert!(report.a_range_is_never_materialized);
    assert!(report.the_full_i64_range_has_a_u128_cardinality);
    assert!(report.is_boolean_accepts_only_the_sparse_pair);
    assert!(report.the_enumeration_gate_is_exclusive_at_the_boundary);
    assert!(report.enumeration_walks_the_closed_interval);
    assert!(report.contains_is_membership_not_hull);
    assert!(report.the_same_value_set_has_two_canonical_encodings);

    // 枚举闸门在边界上"大于才拒"。 / The enumeration gate refuses only when strictly greater.
    let at = run(DomainProbeRequest { domains: vec![range("x", 0, 3)], maximum_amount: Some(4),
        probes: Vec::new() });
    assert_eq!(at.domains[0].enumerated, vec![0, 1, 2, 3]);
    assert_eq!(at.domains[0].enumeration_refusal, None);
    let under = run(DomainProbeRequest { domains: vec![range("x", 0, 3)], maximum_amount: Some(3),
        probes: Vec::new() });
    assert!(under.domains[0].enumerated.is_empty());
    let message = under.domains[0].enumeration_refusal.clone().unwrap();
    assert!(message.contains("cardinality 4") && message.contains("limit 3"), "{message}");

    // 枚举含两端，`i64::MAX` 处靠相等收尾。 
    // Enumeration includes both ends and terminates at `i64::MAX` by equality.
    let walked = run(DomainProbeRequest { domains: vec![range("x", -1, 2)],
        maximum_amount: Some(8), probes: Vec::new() });
    assert_eq!(walked.domains[0].enumerated, vec![-1, 0, 1, 2]);
    let edge = run(DomainProbeRequest { domains: vec![range("e", i64::MAX - 1, i64::MAX)],
        maximum_amount: Some(8), probes: Vec::new() });
    assert_eq!(edge.domains[0].enumerated, vec![i64::MAX - 1, i64::MAX]);

    // 绕过构造器的非规范形态被 `validate` 点名（读面全空）。 
    // A non-canonical shape that bypasses the constructor is named by `validate` (no readable face).
    let raw = run(DomainProbeRequest { domains: vec![DomainProbeSpec { id: "x".to_owned(),
        lower: None, upper: None, values: vec![3, 1], raw: true }], maximum_amount: Some(8),
        probes: vec![1] });
    assert!(!raw.domains[0].valid);
    assert!(raw.domains[0].invalid_reason.as_deref().unwrap()
        .contains("strictly increasing and deduplicated"));
    assert_eq!(raw.domains[0].kind, None);
    assert!(raw.domains[0].contains.is_empty());

    // 拒绝：空 ID、缺界、经构造器的倒置区间、域数上限、缺请求。 
    // Refusals: a blank ID, a missing bound, an inverted range through the constructor, the domain cap, a missing
    // request.
    assert!(verify(DomainProbeRequest { domains: vec![DomainProbeSpec { id: " ".to_owned(),
        lower: Some(0), upper: Some(1), values: Vec::new(), raw: false }],
        maximum_amount: Some(8), probes: Vec::new() }).unwrap_err().message
        .contains("domain IDs must not be blank"));
    assert!(verify(DomainProbeRequest { domains: vec![DomainProbeSpec { id: "x".to_owned(),
        lower: None, upper: Some(1), values: Vec::new(), raw: false }],
        maximum_amount: Some(8), probes: Vec::new() }).unwrap_err().message
        .contains("needs a lower bound"));
    assert!(verify(DomainProbeRequest { domains: vec![range("x", 2, 1)],
        maximum_amount: Some(8), probes: Vec::new() }).unwrap_err().message
        .contains("lower bound exceeds upper bound"));
    assert!(verify(DomainProbeRequest {
        domains: (0..17).map(|index| range(&format!("d{index}"), 0, 1)).collect(),
        maximum_amount: Some(8), probes: Vec::new() }).unwrap_err().message
        .contains("at most 16 domains"));
    let mut missing_request = base_request();
    missing_request.model_kind = Some(ModelKind::DomainSurfaceVerify);
    assert_eq!(engine.submit(missing_request).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        let payload = serde_json::to_string(&document).unwrap();
        std::fs::write(directory.join("domain-surface.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn identity_wrap_entry_shows_a_blank_only_gate() {
    use ospf_rust_bridge::{FingerprintSpec, IdentityWrapReport, IdentityWrapRequest};

    let engine = NativeEngine::default();
    let verify = |document: IdentityWrapRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::IdentityWrapVerify);
        request.identity_wrap = Some(document);
        engine.submit(request)
    };
    let run = |document: IdentityWrapRequest| {
        let view = verify(document).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: IdentityWrapReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        engine.release(&view.id).unwrap();
        report
    };
    let spec = |schema: &str, algorithm: &str, value: &str| FingerprintSpec {
        schema_version: Some(schema.to_owned()), algorithm: Some(algorithm.to_owned()),
        value: Some(value.to_owned()) };
    let good = spec("1.0", "sha256", &"a".repeat(64));
    let document = IdentityWrapRequest { master: good.clone(), solver: good.clone(),
        factory: good.clone() };

    // 三个包装器都构造成功，指纹原样读回。 
    // All three wrappers build and the fingerprints come back verbatim.
    let report = run(document.clone());
    assert_eq!(report.wrappers.len(), 3);
    for view in &report.wrappers {
        assert!(view.accepted, "{:?}", view.refusal);
        assert_eq!(view.schema_version.as_deref(), Some("1.0"));
        assert_eq!(view.algorithm.as_deref(), Some("sha256"));
        assert_eq!(view.value.as_deref(), Some("a".repeat(64).as_str()));
    }
    assert_eq!(report.wrappers[0].wrapped.as_deref(), Some("wrapped-marker"));
    assert_eq!(report.wrappers[1].wrapped.as_deref(), Some("bareSolver"));
    assert_eq!(report.wrappers[2].wrapped.as_deref(), Some("factoryClosure"));
    assert_eq!(report.wrappers[1].identity_visible, Some(true));
    assert!(report.the_gate_is_blank_only);
    assert!(report.the_gate_checks_neither_hex_nor_length_nor_vocabulary);
    assert!(report.the_master_and_subproblem_messages_differ);
    assert!(report.the_two_subproblem_wrappers_share_one_message);
    assert!(report.blank_means_trim_empty);
    assert!(report.the_fingerprint_is_stored_verbatim);
    assert!(report.into_inner_returns_the_wrapped_value);
    assert!(report.wrapping_turns_none_into_some);

    // 垃圾指纹照样过：闸门不查十六进制、不查长度、不查算法词表。 
    // A junk fingerprint passes: the gate checks neither hex, nor length, nor the vocabulary.
    let junk = run(IdentityWrapRequest { master: spec("x", "x", "x"), solver: spec("x", "x", "x"),
        factory: spec("x", "x", "x") });
    assert!(junk.wrappers.iter().all(|view| view.accepted));
    let algorithm = run(IdentityWrapRequest {
        master: spec("1.0", "not-a-real-algorithm", "z"), solver: good.clone(),
        factory: good.clone() });
    assert!(algorithm.wrappers[0].accepted);

    // 空白字段：master 一句、两个 subproblem 共用另一句。 
    // A blank field: the master has one message and the two subproblem wrappers share another.
    let master = run(IdentityWrapRequest { master: spec("", "sha256", "abc"),
        solver: good.clone(), factory: good.clone() });
    assert!(!master.wrappers[0].accepted);
    assert!(master.wrappers[0].refusal.as_deref().unwrap()
        .contains("master fingerprint cannot contain blank fields"));
    assert_eq!(master.wrappers[0].schema_version, None);
    assert!(master.wrappers[1].accepted && master.wrappers[2].accepted);
    let subproblem = run(IdentityWrapRequest { master: good.clone(),
        solver: spec("1.0", "sha256", " "), factory: spec("1.0", "sha256", " ") });
    assert!(!subproblem.wrappers[1].accepted && !subproblem.wrappers[2].accepted);
    assert!(subproblem.wrappers[1].refusal.as_deref().unwrap()
        .contains("subproblem factory fingerprint cannot contain blank fields"));
    assert_eq!(subproblem.wrappers[1].refusal, subproblem.wrappers[2].refusal);
    // 两句文案不同。 / The two messages differ.
    assert_ne!(master.wrappers[0].refusal, subproblem.wrappers[1].refusal);

    // 两端空格通过且原样存下来。 / Padded content passes and is stored verbatim.
    let padded = run(IdentityWrapRequest { master: spec(" 1.0 ", " sha256 ", " abc "),
        solver: good.clone(), factory: good.clone() });
    assert!(padded.wrappers[0].accepted);
    assert_eq!(padded.wrappers[0].schema_version.as_deref(), Some(" 1.0 "));
    assert_eq!(padded.wrappers[0].value.as_deref(), Some(" abc "));

    // 缺字段走默认：`1.0` / `sha256` / 空串 → 最后一项被拒。 
    // A missing field takes the default `1.0` / `sha256` / empty, so the last one is refused.
    let missing = run(IdentityWrapRequest {
        master: FingerprintSpec { schema_version: None, algorithm: None, value: None },
        solver: good.clone(), factory: good.clone() });
    assert!(!missing.wrappers[0].accepted);
    assert!(missing.wrappers[0].refusal.as_deref().unwrap()
        .contains("cannot contain blank fields"));
    let partial = run(IdentityWrapRequest {
        master: FingerprintSpec { schema_version: None, algorithm: None,
            value: Some("abc".to_owned()) },
        solver: good.clone(), factory: good.clone() });
    assert!(partial.wrappers[0].accepted);
    assert_eq!(partial.wrappers[0].schema_version.as_deref(), Some("1.0"));
    assert_eq!(partial.wrappers[0].algorithm.as_deref(), Some("sha256"));

    // 缺请求是 reject。 / A missing request rejects.
    let mut missing_request = base_request();
    missing_request.model_kind = Some(ModelKind::IdentityWrapVerify);
    assert_eq!(engine.submit(missing_request).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        let payload = serde_json::to_string(&document).unwrap();
        std::fs::write(directory.join("identity-wrap.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn container_surface_entry_shows_empty_is_not_zero() {
    use ospf_rust_bridge::{ContainerProbeReport, ContainerProbeRequest, ContainerTermSpec};

    let engine = NativeEngine::default();
    let verify = |document: ContainerProbeRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::ContainerSurfaceVerify);
        request.container_surface = Some(document);
        engine.submit(request)
    };
    let run = |document: ContainerProbeRequest| {
        let view = verify(document).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: ContainerProbeReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        engine.release(&view.id).unwrap();
        report
    };
    let term = |coefficient: f64, var_index: u32| ContainerTermSpec { coefficient, var_index };
    let document = ContainerProbeRequest { terms: vec![term(1.5, 0), term(2.5, 1)],
        constant: 9.0, mutate_constant: false, new_constant: None, append: false, appended: None };

    // 三个容器：线性三项两项、二次经 `from_linear` 全 `Linear` kind、标准同样。 
    // The three containers: linear holds the terms, quadratic goes through `from_linear` and is all `Linear` kind, and
    // canonical matches.
    let report = run(document.clone());
    assert_eq!(report.containers.len(), 3);
    assert_eq!(report.containers[0].container, "linear");
    assert_eq!(report.containers[0].len, 2);
    assert!(!report.containers[0].is_empty);
    assert!(!report.containers[0].is_constant_only);
    assert_eq!(report.containers[0].constant, 9.0);
    assert_eq!(report.containers[0].key_stable, None);
    assert_eq!(report.containers[1].container, "quadratic");
    assert_eq!(report.containers[1].debug_text, "linear,linear");
    assert_eq!(report.containers[1].constant, 9.0);
    assert_eq!(report.containers[2].container, "canonical");
    assert_eq!(report.containers[2].len, 2);
    assert!(report.is_empty_and_is_constant_only_are_one_predicate);
    assert!(report.len_counts_monomials_only);
    assert!(report.zero_and_constant_are_the_same_shape);
    assert!(report.mutation_keeps_the_cache_key);
    assert!(report.add_monomial_appends_without_merging);
    assert!(report.no_value_equality_anywhere);
    assert!(report.from_linear_preserves_the_constant_and_lifts_the_kind);
    assert!(report.clone_changes_the_key_but_not_the_debug);

    // 常数项 5、无单项式：`len == 0`、`is_empty`、`isConstantOnly` 都是真，但常数项是 5。 
    // Constant 5 with no monomials: `len == 0` and both predicates are true while the constant is 5.
    let constant_only = run(ContainerProbeRequest { terms: Vec::new(), constant: 5.0,
        mutate_constant: false, new_constant: None, append: false, appended: None });
    let linear = &constant_only.containers[0];
    assert_eq!(linear.len, 0);
    assert!(linear.is_empty && linear.is_constant_only);
    assert_eq!(linear.constant, 5.0);
    assert_eq!(linear.monomial_count, 0);

    // 改值不改键：改常数与追加项都不动 `Box`。 
    // Mutating the value keeps the key: neither the constant change nor the append moves the `Box`.
    let mutated = run(ContainerProbeRequest { terms: Vec::new(), constant: 1.0,
        mutate_constant: true, new_constant: Some(99.0), append: false, appended: None });
    assert_eq!(mutated.containers[0].key_stable, Some(true));
    assert_eq!(mutated.containers[0].constant, 99.0);
    let appended = run(ContainerProbeRequest { terms: vec![term(1.0, 0)], constant: 1.0,
        mutate_constant: false, new_constant: None, append: true, appended: Some(term(2.0, 0)) });
    assert_eq!(appended.containers[0].key_stable, Some(true));
    // 同一变量加两次 → 两项。 / The same variable twice gives two monomials.
    assert_eq!(appended.containers[0].len, 2);

    // 拒绝：非有限常数、项数上限、缺请求。 / Refusals: a non-finite constant, the term cap, a missing request.
    assert!(verify(ContainerProbeRequest { terms: Vec::new(), constant: f64::NAN,
        mutate_constant: false, new_constant: None, append: false, appended: None }).unwrap_err()
        .message.contains("the constant must be finite"));
    assert!(verify(ContainerProbeRequest {
        terms: (0..33).map(|index| term(1.0, index)).collect(), constant: 0.0,
        mutate_constant: false, new_constant: None, append: false, appended: None }).unwrap_err()
        .message.contains("at most 32 terms"));
    let mut missing_request = base_request();
    missing_request.model_kind = Some(ModelKind::ContainerSurfaceVerify);
    assert_eq!(engine.submit(missing_request).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        let payload = serde_json::to_string(&document).unwrap();
        std::fs::write(directory.join("container-surface.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn activation_set_entry_shows_absence_means_inactive() {
    use ospf_rust_bridge::{ActivationOpSpec, ActivationSetReport, ActivationSetRequest,
        ActivationSourceSpec, ActivationVariableSpec};

    let engine = NativeEngine::default();
    let verify = |document: ActivationSetRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::ActivationSetVerify);
        request.activation_set = Some(document);
        engine.submit(request)
    };
    let run = |document: ActivationSetRequest| {
        let view = verify(document).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: ActivationSetReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        engine.release(&view.id).unwrap();
        report
    };
    let variable = |id: &str| ActivationVariableSpec { id: id.to_owned(), values: Vec::new(),
        upper: None };
    let source = |kind: &str, id: &str| ActivationSourceSpec { kind: kind.to_owned(),
        id: id.to_owned(), side: None };
    let op = |source: ActivationSourceSpec, op: &str| ActivationOpSpec { source,
        op: op.to_owned() };
    let document = ActivationSetRequest {
        variables: vec![variable("x"), ActivationVariableSpec { id: "y".to_owned(),
            values: vec![1, 4, 9], upper: None }],
        with_constraint: Some(true), from_snapshot: Some(true), ops: Vec::new(),
        probes: Vec::new() };

    // 从快照起手：一条约束 + 每个变量两条边界 + 稀疏域额外一条 = 6 条。 
    // Starting from the snapshot: one constraint plus two bounds per variable plus one sparse extra = six records.
    let report = run(document.clone());
    assert_eq!(report.len, 6);
    assert!(!report.is_empty);
    assert_eq!(report.records.iter().map(|record| record.kind.clone()).collect::<Vec<_>>(),
        vec!["constraint", "variable-lower-bound", "variable-upper-bound",
            "variable-lower-bound", "variable-upper-bound", "sparse-domain"]);
    assert!(report.records.iter().all(|record| record.state == "ACTIVATED"));
    assert_eq!(report.active_constraints, vec!["c0".to_owned()]);
    assert!(report.assumptions.iter().all(|id| !id.starts_with("constraint/")));
    assert!(report.assumptions.iter().any(|id| id.starts_with("sparse-domain/")));
    assert!(report.insert_overrides_and_reorders);
    assert!(report.absence_means_inactive);
    assert!(report.the_typed_entry_points_validate_but_insert_does_not);
    assert!(report.the_state_enters_the_stable_identity);
    assert!(report.two_variants_share_one_stable_identity);
    assert!(report.constraints_never_become_assumptions);
    assert!(report.active_constraints_only_reads_constraints);
    assert!(report.from_snapshot_activates_by_domain_shape);

    // 空集合：什么都没有。 / An empty set: nothing at all.
    let empty = run(ActivationSetRequest { variables: vec![variable("x")],
        with_constraint: Some(true), from_snapshot: Some(false), ops: Vec::new(),
        probes: Vec::new() });
    assert_eq!(empty.len, 0);
    assert!(empty.is_empty);
    assert!(empty.records.is_empty());
    assert!(empty.active_constraints.is_empty());

    // `insert` 覆盖并移位。 / `insert` overrides and moves the record.
    let moved = run(ActivationSetRequest { variables: vec![variable("x")],
        with_constraint: Some(true), from_snapshot: Some(false),
        ops: vec![op(source("lowerBound", "a"), "insertActivate"),
            op(source("lowerBound", "b"), "insertActivate"),
            op(source("lowerBound", "a"), "insertDeactivate")], probes: Vec::new() });
    assert_eq!(moved.len, 2);
    assert_eq!(moved.records[0].source_stable_id, "variable:b:lower");
    assert_eq!(moved.records[1].source_stable_id, "variable:a:lower");
    assert_eq!(moved.records[1].state, "DEACTIVATED");

    // 缺席即未激活；停用记录也算出现过。 
    // Absence means inactive; a deactivated record still counts as present.
    let absent = run(ActivationSetRequest { variables: vec![variable("x")],
        with_constraint: Some(true), from_snapshot: Some(false),
        ops: vec![op(source("constraint", "c0"), "insertDeactivate")],
        probes: vec![source("constraint", "c0"), source("constraint", "c9")] });
    assert_eq!(absent.probe_active, vec![false, false]);
    assert_eq!(absent.len, 1);

    // 两个变体共享一个稳定身份 → 两条记录、身份重复。 
    // Two variants share one stable identity, so two records carry a duplicate identity.
    let shared = run(ActivationSetRequest { variables: vec![variable("x")],
        with_constraint: Some(true), from_snapshot: Some(false),
        ops: vec![op(source("lowerBound", "x"), "insertActivate"),
            op(ActivationSourceSpec { kind: "variableBound".to_owned(), id: "x".to_owned(),
                side: Some("lower".to_owned()) }, "insertActivate")], probes: Vec::new() });
    assert_eq!(shared.len, 2);
    assert_eq!(shared.records[0].stable_id, shared.records[1].stable_id);
    assert_eq!(shared.duplicate_stable_ids, vec!["activated:variable:x:lower".to_owned()]);

    // 拒绝：空白 ID 经 `activate`、未知种类/方向/操作、变量数上限、缺请求。 
    // Refusals: a blank ID through `activate`, unknown kind/side/operation, the operation cap, a missing request.
    assert!(verify(ActivationSetRequest { variables: vec![variable("x")],
        with_constraint: Some(true), from_snapshot: Some(false),
        ops: vec![op(source("constraint", "  "), "activate")], probes: Vec::new() }).unwrap_err()
        .message.contains("constraint ID must not be blank"));
    assert!(verify(ActivationSetRequest { variables: vec![variable("x")],
        with_constraint: Some(true), from_snapshot: Some(false),
        ops: vec![op(source("nope", "x"), "activate")], probes: Vec::new() }).unwrap_err()
        .message.contains("unknown source kind"));
    assert!(verify(ActivationSetRequest { variables: vec![variable("x")],
        with_constraint: Some(true), from_snapshot: Some(false),
        ops: vec![op(ActivationSourceSpec { kind: "variableBound".to_owned(), id: "x".to_owned(),
            side: Some("middle".to_owned()) }, "activate")], probes: Vec::new() }).unwrap_err()
        .message.contains("unknown bound side"));
    assert!(verify(ActivationSetRequest { variables: vec![variable("x")],
        with_constraint: Some(true), from_snapshot: Some(false),
        ops: vec![op(source("constraint", "c0"), "nope")], probes: Vec::new() }).unwrap_err()
        .message.contains("unknown operation"));
    assert!(verify(ActivationSetRequest { variables: vec![variable("  ")],
        with_constraint: Some(true), from_snapshot: Some(false), ops: Vec::new(),
        probes: Vec::new() }).unwrap_err().message.contains("variable IDs must not be blank"));
    assert!(verify(ActivationSetRequest { variables: vec![variable("x")],
        with_constraint: Some(true), from_snapshot: Some(false),
        ops: (0..33).map(|index| op(source("constraint", &format!("c{index}")), "insertActivate"))
            .collect(), probes: Vec::new() }).unwrap_err().message
        .contains("at most 32 operations"));
    let mut missing_request = base_request();
    missing_request.model_kind = Some(ModelKind::ActivationSetVerify);
    assert_eq!(engine.submit(missing_request).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        let payload = serde_json::to_string(&document).unwrap();
        std::fs::write(directory.join("activation-set.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn tier_selection_entry_shows_the_native_flag_decoupling() {
    use ospf_rust_bridge::{TierOverrideSpec, TierSelectionReport, TierSelectionRequest};

    let engine = NativeEngine::default();
    let verify = |document: TierSelectionRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::TierSelectionVerify);
        request.tier_selection = Some(document);
        engine.submit(request)
    };
    let run = |document: TierSelectionRequest| {
        let view = verify(document).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: TierSelectionReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        engine.release(&view.id).unwrap();
        report
    };
    let override_of = |capability: &str, support: &str| TierOverrideSpec {
        capability: capability.to_owned(), support: support.to_owned() };
    let plain = TierSelectionRequest { model_types: Vec::new(), native_assumption_solving: false,
        native_unsat_core: false, fallback_satisfaction_only: false, dual: false,
        exact_cp_lowering: false, overrides: Vec::new() };

    // 空矩阵：什么都不选。 / An empty matrix selects nothing.
    let report = run(plain.clone());
    assert_eq!(report.selected, "unavailable");
    assert!(!report.selected_is_native);
    assert!(!report.selected_is_available);
    assert_eq!(report.native_unsat_core_support, "unsupported");
    assert_eq!(report.assumption_solving_support, "unsupported");
    assert_eq!(report.conflict_support, "unsupported");
    assert!(report.the_order_is_the_priority);
    assert!(report.only_supported_is_selected);
    assert!(report.a_fallback_only_backend_lands_on_repeated_solving);
    assert!(report.an_override_can_upgrade_the_selection);
    assert!(report.an_override_can_disable_the_fallback);
    assert!(report.selecting_the_native_tier_does_not_make_it_native);
    assert!(report.the_four_tiers_have_exact_predicates);
    assert!(report.the_stable_names_are_four_words);

    // 顺序即优先级：两个原生都在时选 unsat core。 
    // The order is the priority: with both natives present the unsat core wins.
    let both = run(TierSelectionRequest { native_unsat_core: true,
        native_assumption_solving: true, ..plain.clone() });
    assert_eq!(both.selected, "native-unsat-core");
    assert!(both.selected_is_native && both.native_unsat_core_flag);
    let assumption_only = run(TierSelectionRequest { native_assumption_solving: true,
        ..plain.clone() });
    assert_eq!(assumption_only.selected, "assumption-extraction");
    assert!(!assumption_only.selected_is_native);

    // 只有 fallback 的后端走 `RepeatedSolving`。 
    // A fallback-only backend lands on `RepeatedSolving`.
    let fallback = run(TierSelectionRequest { fallback_satisfaction_only: true, ..plain.clone() });
    assert_eq!(fallback.selected, "repeated-solving");
    assert_eq!(fallback.assumption_solving_support, "conditional");
    assert_eq!(fallback.conflict_support, "conditional");
    assert!(fallback.selected_is_available);

    // 覆盖抬升 / 覆盖关掉。 / An override upgrades; another disables.
    let upgraded = run(TierSelectionRequest { overrides: vec![override_of("assumption_solving",
        "supported")], ..plain.clone() });
    assert_eq!(upgraded.selected, "assumption-extraction");
    let disabled = run(TierSelectionRequest { fallback_satisfaction_only: true,
        overrides: vec![override_of("conflict", "unsupported")], ..plain.clone() });
    assert_eq!(disabled.selected, "unavailable");
    assert_eq!(disabled.conflict_support, "unsupported");

    // 选中原生层不等于原生标志为真。 
    // Selecting the native tier does not make the flag true.
    let decoupled = run(TierSelectionRequest { overrides: vec![override_of("native_unsat_core",
        "supported")], ..plain.clone() });
    assert_eq!(decoupled.selected, "native-unsat-core");
    assert!(decoupled.selected_is_native);
    assert!(!decoupled.native_unsat_core_flag);
    assert_eq!(decoupled.native_unsat_core_support, "supported");

    // 模型种类不影响层级选择。 / The model kinds never move the tier.
    for kind in ["linear", "mip", "quadratic", "miqp", "constraintProgramming"] {
        let by_kind = run(TierSelectionRequest { model_types: vec![kind.to_owned()],
            native_unsat_core: true, ..plain.clone() });
        assert_eq!(by_kind.selected, "native-unsat-core", "{kind}");
    }

    // 拒绝：未知模型种类 / 能力 / 支持级别、覆盖上限、缺请求。 
    // Refusals: an unknown model kind, capability or support level, the override cap, a missing request.
    assert!(verify(TierSelectionRequest { model_types: vec!["nope".to_owned()],
        ..plain.clone() }).unwrap_err().message.contains("unknown model kind"));
    assert!(verify(TierSelectionRequest { overrides: vec![override_of("nope", "supported")],
        ..plain.clone() }).unwrap_err().message.contains("unknown capability"));
    assert!(verify(TierSelectionRequest { overrides: vec![override_of("conflict", "maybe")],
        ..plain.clone() }).unwrap_err().message.contains("unknown support level"));
    assert!(verify(TierSelectionRequest { overrides: (0..17)
        .map(|_| override_of("conflict", "supported")).collect(), ..plain.clone() }).unwrap_err()
        .message.contains("at most 16 overrides"));
    let mut missing_request = base_request();
    missing_request.model_kind = Some(ModelKind::TierSelectionVerify);
    assert_eq!(engine.submit(missing_request).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        let payload = serde_json::to_string(&plain).unwrap();
        std::fs::write(directory.join("tier-selection.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn evidence_remap_entry_shows_the_two_unreachable_kinds() {
    use ospf_rust_bridge::{EvidenceMemberSpec, EvidenceRemapReport, EvidenceRemapRequest,
        EvidenceSourceSpec, EvidenceSourceVariableSpec};

    let engine = NativeEngine::default();
    let verify = |document: EvidenceRemapRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::EvidenceRemapVerify);
        request.evidence_remap = Some(document);
        engine.submit(request)
    };
    let run = |document: EvidenceRemapRequest| {
        let view = verify(document).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: EvidenceRemapReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        engine.release(&view.id).unwrap();
        report
    };
    let variable = |id: &str| EvidenceSourceVariableSpec { id: id.to_owned(), values: Vec::new(),
        upper: None };
    let sparse = |id: &str, values: Vec<i64>| EvidenceSourceVariableSpec { id: id.to_owned(),
        values, upper: None };
    let source = |kind: &str, id: &str| EvidenceSourceSpec { kind: kind.to_owned(),
        id: id.to_owned(), side: None };
    let bound = |id: &str, side: &str| EvidenceSourceSpec { kind: "variableBound".to_owned(),
        id: id.to_owned(), side: Some(side.to_owned()) };
    let member = |kind: &str, value: &str| EvidenceMemberSpec { kind: kind.to_owned(),
        value: value.to_owned() };
    let plain = EvidenceRemapRequest { variables: vec![variable("x")], with_constraint: Some(true),
        from_snapshot: Some(false), extra_sources: Vec::new(),
        deactivated_sources: Vec::new(), members: Vec::new() };

    // 真实载荷（native 测试写出）：空证据集不产生任何记录。 
    // The real payload: an empty evidence set produces no record.
    let empty = run(plain.clone());
    assert!(empty.sources.is_empty() && empty.remapped.is_empty() && empty.assumptions.is_empty());
    assert!(empty.objective_targets_have_no_evidence_member);
    assert!(empty.the_interval_member_can_never_match);
    assert!(empty.as_member_drops_the_sparse_domain_content);
    assert!(empty.direct_members_match_without_the_assumption_path);
    assert!(empty.the_assumption_path_reaches_bound_variants);
    assert!(empty.unknown_members_are_dropped);
    assert!(empty.deactivated_sources_are_never_remapped);
    assert!(empty.shared_identity_records_are_not_deduped);

    // 快照面上六型都有成员，只有 `objectiveTarget` 没有；稀疏域只留变量名。 
    // Every snapshot kind has a member except `objectiveTarget`; the sparse domain keeps only the variable name.
    let surface = run(EvidenceRemapRequest { variables: vec![variable("x"),
        sparse("y", vec![1, 4, 9])], with_constraint: Some(true), from_snapshot: Some(true),
        extra_sources: vec![source("objectiveTarget", "obj")], deactivated_sources: Vec::new(),
        members: Vec::new() });
    assert_eq!(surface.sources.len(), 7);
    assert_eq!(surface.sources.last().unwrap().kind, "objective-target");
    assert_eq!(surface.sources.last().unwrap().member, None);
    assert!(surface.sources[..6].iter().all(|view| view.member.is_some()));
    assert_eq!(surface.sources[5].member.as_deref(), Some("domain:y"));
    assert_eq!(surface.assumptions.len(), 5);
    assert!(surface.assumptions.iter().all(|id| !id.starts_with("constraint/")));

    // 直接成员。 / A direct member.
    let direct = run(EvidenceRemapRequest { from_snapshot: Some(true),
        members: vec![member("constraint", "c0")], ..plain.clone() });
    assert_eq!(direct.remapped, vec!["constraint:c0".to_owned()]);

    // assumption 路径：共享身份让 `variableBound` 命中（身份是三段式 `lower-bound/1:x/0`）。 
    // The assumption path: the shared identity lets a `variableBound` hit (a three-segment identity).
    let via_assumption = run(EvidenceRemapRequest { extra_sources: vec![bound("x", "lower")],
        members: vec![member("assumption", "lower-bound/1:x/0")], ..plain.clone() });
    assert_eq!(via_assumption.remapped, vec!["variable:x:lower".to_owned()]);
    // 同样两条共享身份的记录回映两次。 / Two shared-identity records remap twice.
    let twice = run(EvidenceRemapRequest {
        extra_sources: vec![source("lowerBound", "x"), bound("x", "lower")],
        members: vec![member("lowerBound", "x")], ..plain.clone() });
    assert_eq!(twice.sources.len(), 2);
    assert_eq!(twice.remapped.len(), 2);

    // `Interval` 与未知成员都命中不了。 / An `Interval` member and an unknown one both miss.
    let missed = run(EvidenceRemapRequest { extra_sources: vec![source("lowerBound", "ivA")],
        members: vec![member("interval", "ivA"), member("constraint", "ghost")],
        ..plain.clone() });
    assert!(missed.remapped.is_empty());

    // 停用记录不参与。 / A deactivated record never takes part.
    let deactivated = run(EvidenceRemapRequest {
        deactivated_sources: vec![source("constraint", "c0")],
        members: vec![member("constraint", "c0")], ..plain.clone() });
    assert!(deactivated.sources.is_empty() && deactivated.remapped.is_empty());

    // 拒绝：未知来源/成员种类、未知方向、空值、空变量 ID、成员上限、缺请求。 
    // Refusals: an unknown source or member kind, an unknown side, a blank value or variable ID, the member cap, a
    // missing request.
    assert!(verify(EvidenceRemapRequest { extra_sources: vec![source("nope", "x")],
        ..plain.clone() }).unwrap_err().message.contains("unknown source kind"));
    assert!(verify(EvidenceRemapRequest { extra_sources: vec![bound("x", "middle")],
        ..plain.clone() }).unwrap_err().message.contains("unknown bound side"));
    assert!(verify(EvidenceRemapRequest { members: vec![member("nope", "x")],
        ..plain.clone() }).unwrap_err().message.contains("unknown member kind"));
    assert!(verify(EvidenceRemapRequest { members: vec![member("constraint", "  ")],
        ..plain.clone() }).unwrap_err().message.contains("member values must not be blank"));
    assert!(verify(EvidenceRemapRequest { variables: vec![variable("  ")],
        ..plain.clone() }).unwrap_err().message.contains("variable IDs must not be blank"));
    assert!(verify(EvidenceRemapRequest { members: (0..33)
        .map(|index| member("constraint", &format!("c{index}"))).collect(), ..plain.clone() })
        .unwrap_err().message.contains("at most 32 members"));
    let mut missing_request = base_request();
    missing_request.model_kind = Some(ModelKind::EvidenceRemapVerify);
    assert_eq!(engine.submit(missing_request).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        let payload = serde_json::to_string(&plain).unwrap();
        std::fs::write(directory.join("evidence-remap.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn dominance_entry_shows_the_truncating_direction_list() {
    use ospf_rust_bridge::{DominancePairSpec, DominanceReport, DominanceRequest};

    let engine = NativeEngine::default();
    let verify = |document: DominanceRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::DominanceVerify);
        request.dominance = Some(document);
        engine.submit(request)
    };
    let run = |document: DominanceRequest| {
        let view = verify(document).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: DominanceReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        engine.release(&view.id).unwrap();
        report
    };
    let pair = |lhs: &[f64], rhs: &[f64]| DominancePairSpec { lhs: lhs.to_vec(),
        rhs: rhs.to_vec() };
    let directions = |values: Vec<bool>, pairs: Vec<DominancePairSpec>| DominanceRequest {
        mode: "directions".to_owned(), directions: values, objective_amount: None, pairs,
        with_reverse: Some(true) };
    let plain = directions(vec![true], Vec::new());

    // 空对：八条规则照旧可读。 / No pairs: the eight rules still read.
    let empty = run(plain.clone());
    assert!(empty.pairs.is_empty());
    assert_eq!(empty.compared_axes, 1);
    assert_eq!(empty.mode, "directions");
    assert!(empty.the_relation_is_a_strict_partial_order);
    assert!(empty.a_shorter_direction_list_truncates_the_comparison);
    assert!(empty.a_longer_direction_list_refuses_every_comparison);
    assert!(empty.nan_counts_as_equal_not_as_poison);
    assert!(empty.infinities_compare_normally);
    assert!(empty.an_empty_direction_list_never_dominates);
    assert!(empty.minimization_is_a_pure_delegation);
    assert!(empty.mixed_directions_leave_vectors_incomparable);

    // 方向表短了：第二维被忽略。 / A short direction list ignores the second axis.
    let short = run(directions(vec![true], vec![pair(&[1.0, 10.0], &[2.0, 0.0])]));
    assert!(short.pairs[0].forward && !short.pairs[0].reverse);
    assert_eq!(short.compared_axes, 1);
    let full = run(directions(vec![true, true], vec![pair(&[1.0, 10.0], &[2.0, 0.0])]));
    assert!(!full.pairs[0].forward);

    // 方向表长了：一律拒绝。 / A long direction list refuses everything.
    let long = run(directions(vec![true, true, true],
        vec![pair(&[0.0, 0.0], &[1.0, 1.0])]));
    assert!(!long.pairs[0].forward && !long.pairs[0].reverse);
    assert_eq!(long.compared_axes, 3);

    // 混合方向：互不可比。 / Mixed directions: incomparable both ways.
    let mixed = run(directions(vec![true, false],
        vec![pair(&[1.0, 1.0], &[2.0, 2.0]), pair(&[1.0, 2.0], &[2.0, 1.0])]));
    assert!(!mixed.pairs[0].forward && !mixed.pairs[0].reverse);
    assert!(mixed.pairs[1].forward && !mixed.pairs[1].reverse);

    // 空方向表：永不支配。 / An empty direction list never dominates.
    let nothing = run(directions(Vec::new(), vec![pair(&[1.0], &[2.0])]));
    assert!(!nothing.pairs[0].forward && !nothing.pairs[0].reverse);
    assert_eq!(nothing.compared_axes, 0);

    // `minimization` 模式与全最小化方向表逐项相同。 
    // The `minimization` mode matches an all-minimisation direction list entry by entry.
    let samples = vec![pair(&[1.0, 2.0], &[2.0, 3.0]), pair(&[1.0, 2.0], &[1.0, 2.0]),
        pair(&[1.0, 4.0], &[2.0, 3.0]), pair(&[2.0, 3.0], &[1.0, 2.0])];
    let by_minimization = run(DominanceRequest { mode: "minimization".to_owned(),
        directions: Vec::new(), objective_amount: Some(2), pairs: samples.clone(),
        with_reverse: Some(true) });
    let by_directions = run(directions(vec![true, true], samples));
    assert_eq!(by_minimization.pairs, by_directions.pairs);
    assert_eq!(by_minimization.compared_axes, 2);

    // 拒绝：未知模式、非有限目标值、两处上限、缺请求。 
    // Refusals: an unknown mode, a non-finite objective, both caps, a missing request.
    assert!(verify(DominanceRequest { mode: "nope".to_owned(), ..plain.clone() }).unwrap_err()
        .message.contains("unknown mode"));
    assert!(verify(DominanceRequest { pairs: vec![pair(&[f64::INFINITY], &[1.0])],
        ..plain.clone() }).unwrap_err().message.contains("non-finite objective values"));
    assert!(verify(DominanceRequest { pairs: vec![pair(&[f64::NAN], &[1.0])],
        ..plain.clone() }).unwrap_err().message.contains("non-finite objective values"));
    assert!(verify(DominanceRequest { pairs: (0..65).map(|_| pair(&[0.0], &[1.0])).collect(),
        ..plain.clone() }).unwrap_err().message.contains("at most 64 pairs"));
    assert!(verify(DominanceRequest { directions: vec![true; 65], ..plain.clone() })
        .unwrap_err().message.contains("at most 64 directions"));
    let mut missing_request = base_request();
    missing_request.model_kind = Some(ModelKind::DominanceVerify);
    assert_eq!(engine.submit(missing_request).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        let payload = serde_json::to_string(&DominanceRequest { directions: vec![true, true],
            mode: "directions".to_owned(),
            pairs: vec![pair(&[1.0, 2.0], &[2.0, 3.0]), pair(&[1.0, 2.0], &[1.0, 2.0]),
                pair(&[1.0, 4.0], &[2.0, 3.0])], ..plain }).unwrap();
        std::fs::write(directory.join("dominance.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn dispersion_entry_shows_the_constant_axis_boundary() {
    use ospf_rust_bridge::{DispersionReport, DispersionRequest};

    let engine = NativeEngine::default();
    let verify = |document: DispersionRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::DispersionVerify);
        request.dispersion = Some(document);
        engine.submit(request)
    };
    let run = |document: DispersionRequest| {
        let view = verify(document).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: DispersionReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        engine.release(&view.id).unwrap();
        report
    };
    let plain = DispersionRequest { mode: "directions".to_owned(), directions: vec![true, true],
        objective_amount: None, points: Vec::new() };

    // 空种群没有层，八条规则照旧可读。 / An empty population has no front and the eight rules still read.
    let empty = run(plain.clone());
    assert_eq!(empty.front_count, 0);
    assert_eq!(empty.total_members, 0);
    assert!(empty.fronts.is_empty());
    assert_eq!(empty.compared_axes, 2);
    assert!(empty.the_fronts_partition_the_candidates);
    assert!(empty.a_front_keeps_the_input_order);
    assert!(empty.front_zero_is_the_pareto_front);
    assert!(empty.duplicates_share_the_first_front);
    assert!(empty.a_truncated_direction_list_regroups_the_fronts);
    assert!(empty.an_empty_population_has_no_front);
    assert!(empty.the_crowding_boundary_is_infinite_even_on_a_constant_axis);
    assert!(empty.crowding_reads_the_first_point_for_the_axis_count);

    // 五点两层：第 0 层四个成员，拥挤距离首尾无穷、中间有限。 
    // Five points in two fronts: front 0 keeps four members with infinite ends and finite middles.
    let split = run(DispersionRequest { points: vec![vec![1.0, 4.0], vec![2.0, 3.0],
        vec![3.0, 2.0], vec![4.0, 1.0], vec![5.0, 5.0]], ..plain.clone() });
    assert_eq!(split.front_count, 2);
    assert_eq!(split.total_members, 5);
    assert_eq!(split.fronts[0].members, vec![0, 1, 2, 3]);
    assert_eq!(split.fronts[1].members, vec![4]);
    assert!(split.fronts[0].crowding[0].infinite);
    assert!(split.fronts[0].crowding[3].infinite);
    assert!(!split.fronts[0].crowding[1].infinite);
    assert!(split.fronts[0].crowding[1].distance.is_some());
    assert!(!split.all_crowding_infinite);

    // 重复点同层。 / Duplicate points share the first front.
    let duplicated = run(DispersionRequest { points: vec![vec![1.0, 1.0], vec![1.0, 1.0],
        vec![2.0, 2.0]], ..plain.clone() });
    assert_eq!(duplicated.front_count, 2);
    assert_eq!(duplicated.fronts[0].members, vec![0, 1]);
    assert_eq!(duplicated.fronts[1].members, vec![2]);

    // 方向表截断会重排分层。 / A truncated direction list re-layers the population.
    let both = run(DispersionRequest { points: vec![vec![1.0, 10.0], vec![2.0, 0.0]],
        ..plain.clone() });
    assert_eq!(both.front_count, 1);
    let truncated = run(DispersionRequest { directions: vec![true],
        points: vec![vec![1.0, 10.0], vec![2.0, 0.0]], ..plain.clone() });
    assert_eq!(truncated.front_count, 2);
    assert_eq!(truncated.compared_axes, 1);

    // 三维里第二列全相等、另两列交叉：三点仍在一层，边界无穷、中间有限。 
    // Three axes with an equal middle column and two criss-crossing ones: the three points stay in one front, the
    // boundaries are infinite and the middle is finite.
    let constant = run(DispersionRequest { directions: vec![true, true, true],
        points: vec![vec![0.0, 5.0, 2.0], vec![1.0, 5.0, 1.0], vec![2.0, 5.0, 0.0]],
        ..plain.clone() });
    assert_eq!(constant.front_count, 1);
    assert!(constant.fronts[0].crowding[0].infinite);
    assert!(constant.fronts[0].crowding[2].infinite);
    assert_eq!(constant.fronts[0].crowding[1].distance, Some(2.0));
    // 二维同数据（去掉常量轴）中间点仍是 2.0，边界仍是无穷——常量轴不改变中间点的有限性。 
    // The same data on two axes keeps the middle at 2.0 and the boundaries infinite, so the constant axis never changes
    // the middle's finiteness.
    let two_axes = run(DispersionRequest { points: vec![vec![0.0, 2.0], vec![1.0, 1.0],
        vec![2.0, 0.0]], ..plain.clone() });
    assert_eq!(two_axes.fronts[0].crowding[1].distance, Some(2.0));

    // 三点单调排列：中间点距离正好 2.0。 / Three monotone points: the middle distance is exactly 2.0.
    let monotone = run(DispersionRequest { points: vec![vec![0.0, 2.0], vec![1.0, 1.0],
        vec![2.0, 0.0]], ..plain.clone() });
    assert_eq!(monotone.fronts[0].crowding[1].distance, Some(2.0));

    // 两点互有支配 → 两层，每层单点 → 全无穷。 
    // Two points where one dominates the other form two single-member fronts, all infinite.
    let pair = run(DispersionRequest { points: vec![vec![0.0, 0.0], vec![1.0, 1.0]],
        ..plain.clone() });
    assert_eq!(pair.front_count, 2);
    assert!(pair.all_crowding_infinite);

    // `minimization` 模式与全最小化方向表逐层相同。 
    // The `minimization` mode matches an all-minimisation direction list front by front.
    let points = vec![vec![1.0, 4.0], vec![2.0, 3.0], vec![5.0, 5.0]];
    let by_minimization = run(DispersionRequest { mode: "minimization".to_owned(),
        directions: Vec::new(), objective_amount: Some(2), points: points.clone() });
    let by_directions = run(DispersionRequest { points, ..plain.clone() });
    assert_eq!(by_minimization.fronts, by_directions.fronts);
    assert_eq!(by_minimization.compared_axes, 2);

    // 拒绝：参差维度、非有限值、未知模式、三处上限、缺请求。 
    // Refusals: ragged axes, a non-finite value, an unknown mode, all three caps, a missing request.
    assert!(verify(DispersionRequest { points: vec![vec![0.0], vec![1.0, 2.0]],
        ..plain.clone() }).unwrap_err().message.contains("same axis count"));
    assert!(verify(DispersionRequest { points: vec![vec![f64::INFINITY]],
        ..plain.clone() }).unwrap_err().message.contains("non-finite objective values"));
    assert!(verify(DispersionRequest { points: vec![vec![f64::NAN]], ..plain.clone() })
        .unwrap_err().message.contains("non-finite objective values"));
    assert!(verify(DispersionRequest { mode: "nope".to_owned(), ..plain.clone() }).unwrap_err()
        .message.contains("unknown mode"));
    assert!(verify(DispersionRequest { points: (0..65).map(|_| vec![0.0]).collect(),
        ..plain.clone() }).unwrap_err().message.contains("at most 64 candidates"));
    assert!(verify(DispersionRequest { points: vec![vec![0.0; 65]], ..plain.clone() })
        .unwrap_err().message.contains("at most 64 axes"));
    assert!(verify(DispersionRequest { directions: vec![true; 65], ..plain.clone() })
        .unwrap_err().message.contains("at most 64 directions"));
    let mut missing_request = base_request();
    missing_request.model_kind = Some(ModelKind::DispersionVerify);
    assert_eq!(engine.submit(missing_request).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        let payload = serde_json::to_string(&DispersionRequest { points: vec![vec![1.0, 4.0],
            vec![2.0, 3.0], vec![3.0, 2.0], vec![4.0, 1.0], vec![5.0, 5.0]], ..plain }).unwrap();
        std::fs::write(directory.join("dispersion.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn reference_points_entry_shows_the_simplex_lattice() {
    use ospf_rust_bridge::{ReferencePairSpec, ReferencePointsReport, ReferencePointsRequest};

    let engine = NativeEngine::default();
    let verify = |document: ReferencePointsRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::ReferencePointsVerify);
        request.reference_points = Some(document);
        engine.submit(request)
    };
    let run = |document: ReferencePointsRequest| {
        let view = verify(document).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: ReferencePointsReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        engine.release(&view.id).unwrap();
        report
    };
    let plain = ReferencePointsRequest { objective_amount: 3, divisions: 4, pairs: Vec::new() };

    // 15 个点的格点：首尾是两个角。 / The 15-point lattice runs between two corners.
    let lattice = run(plain.clone());
    assert_eq!(lattice.point_count, 15);
    assert_eq!(lattice.axis_width, 4);
    assert!(lattice.every_point_sums_to_one);
    assert_eq!(lattice.points[0], vec![0.0, 0.0, 1.0]);
    assert_eq!(lattice.points[14], vec![1.0, 0.0, 0.0]);
    assert!(lattice.the_point_count_follows_the_simplex_lattice);
    assert!(lattice.an_illegal_dimension_yields_an_empty_list);
    assert!(lattice.one_objective_collapses_to_the_unit_point);
    assert!(lattice.every_point_sums_to_one_measured);
    assert!(lattice.the_order_is_lexicographic_in_the_first_coordinate);
    assert!(lattice.the_distance_ignores_objectives_beyond_the_reference);
    assert!(lattice.a_missing_objective_or_a_zero_reference_is_infinite);
    assert!(lattice.the_distance_is_invariant_to_reference_scaling);

    // 二维五点多。 / Two objectives over four divisions give five points.
    let two = run(ReferencePointsRequest { objective_amount: 2, divisions: 4, pairs: Vec::new() });
    assert_eq!(two.point_count, 5);
    assert_eq!(two.points[0], vec![0.0, 1.0]);
    assert_eq!(two.points[4], vec![1.0, 0.0]);

    // 一个目标必塌成单位点。 / One objective collapses to the unit point.
    for divisions in [1_u32, 2, 5, 16] {
        let single = run(ReferencePointsRequest { objective_amount: 1, divisions,
            pairs: Vec::new() });
        assert_eq!(single.point_count, 1, "{divisions}");
        assert_eq!(single.points, vec![vec![1.0]], "{divisions}");
    }

    // 非法维度给空表（不 panic）。 / An illegal dimension gives an empty list without panicking.
    for (amount, divisions) in [(0_u32, 4_u32), (3, 0), (0, 0), (1, 0)] {
        let empty = run(ReferencePointsRequest { objective_amount: amount, divisions,
            pairs: Vec::new() });
        assert_eq!(empty.point_count, 0, "({amount},{divisions})");
        assert!(empty.points.is_empty(), "({amount},{divisions})");
        assert!(empty.every_point_sums_to_one, "({amount},{divisions})");
    }

    // 距离：有限、无穷（零参考 / 目标偏短）、以及 0。 
    // Distances: finite, infinite (zero reference or a short objective vector), and zero.
    let distances = run(ReferencePointsRequest { objective_amount: 2, divisions: 4, pairs: vec![
        ReferencePairSpec { objectives: vec![1.0, 1.0], reference: vec![1.0, 0.0] },
        ReferencePairSpec { objectives: vec![1.0, 1.0], reference: vec![2.0, 0.0] },
        ReferencePairSpec { objectives: vec![1.0, 1.0], reference: vec![0.0, 0.0] },
        ReferencePairSpec { objectives: vec![1.0], reference: vec![1.0, 0.0] },
        ReferencePairSpec { objectives: vec![1.0, 900.0], reference: vec![1.0] },
    ] });
    assert_eq!(distances.pairs[0].distance, Some(1.0));
    // 尺度不变：`[1,0]` 与 `[2,0]` 同值。 / Scale invariant: `[1,0]` and `[2,0]` agree.
    assert_eq!(distances.pairs[0].distance, distances.pairs[1].distance);
    assert!(distances.pairs[2].infinite && distances.pairs[2].distance.is_none());
    assert!(distances.pairs[3].infinite);
    assert_eq!(distances.pairs[4].distance, Some(0.0));

    // 拒绝：维度/分割数上限、格点上限、配对上与逐轴上限、非有限值、缺请求。 
    // Refusals: the dimension and division caps, the lattice cap, the pair and per-axis caps, a non-finite value, a
    // missing request.
    assert!(verify(ReferencePointsRequest { objective_amount: 9, divisions: 4, pairs: Vec::new() })
        .unwrap_err().message.contains("at most 8 objectives"));
    assert!(verify(ReferencePointsRequest { objective_amount: 3, divisions: 17, pairs: Vec::new() })
        .unwrap_err().message.contains("at most 16 divisions"));
    assert!(verify(ReferencePointsRequest { objective_amount: 8, divisions: 16, pairs: Vec::new() })
        .unwrap_err().message.contains("exceed the 512-point cap"));
    assert!(verify(ReferencePointsRequest { pairs: (0..33).map(|_| ReferencePairSpec {
        objectives: vec![1.0], reference: vec![1.0] }).collect(), ..plain.clone() })
        .unwrap_err().message.contains("at most 32 pairs"));
    assert!(verify(ReferencePointsRequest { pairs: vec![ReferencePairSpec {
        objectives: vec![f64::INFINITY], reference: vec![1.0] }], ..plain.clone() })
        .unwrap_err().message.contains("non-finite values"));
    assert!(verify(ReferencePointsRequest { pairs: vec![ReferencePairSpec {
        objectives: vec![1.0; 9], reference: vec![1.0] }], ..plain.clone() })
        .unwrap_err().message.contains("at most 8 axes"));
    let mut missing_request = base_request();
    missing_request.model_kind = Some(ModelKind::ReferencePointsVerify);
    assert_eq!(engine.submit(missing_request).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        let payload = serde_json::to_string(&ReferencePointsRequest { objective_amount: 3,
            divisions: 4, pairs: vec![ReferencePairSpec { objectives: vec![1.0, 1.0],
                reference: vec![1.0, 0.0] }] }).unwrap();
        std::fs::write(directory.join("reference-points.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn pareto_archive_entry_shows_the_trim_reordering() {
    use ospf_rust_bridge::{ArchiveReport, ArchiveRequest};

    let engine = NativeEngine::default();
    let verify = |document: ArchiveRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::ParetoArchiveVerify);
        request.pareto_archive = Some(document);
        engine.submit(request)
    };
    let run = |document: ArchiveRequest| {
        let view = verify(document).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: ArchiveReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        engine.release(&view.id).unwrap();
        report
    };
    let plain = ArchiveRequest { capacity: 8, mode: "minimization".to_owned(),
        directions: Vec::new(), objective_amount: Some(2), batches: Vec::new(),
        offer_mode: "batch".to_owned() };

    // 空请求：八条规则照旧可读。 / An empty request keeps the eight rules readable.
    let empty = run(plain.clone());
    assert_eq!(empty.size, 0);
    assert!(empty.items.is_empty());
    assert_eq!(empty.capacity, 8);
    assert_eq!(empty.offer_mode, "batch");
    assert!(empty.a_dominated_candidate_is_discarded);
    assert!(empty.a_newcomer_evicts_the_members_it_dominates);
    assert!(empty.a_duplicate_objective_vector_keeps_the_first_offer);
    assert!(empty.an_incomparable_candidate_is_kept);
    assert!(empty.trim_reorders_the_archive_by_crowding_distance_descending);
    assert!(empty.a_zero_capacity_archive_stays_empty);
    assert!(empty.clear_empties_the_archive);
    assert!(empty.a_batch_trims_once_while_offer_one_trims_per_candidate);

    // 被支配的丢弃、支配的淘汰。 / Dominated dropped, dominating evicts.
    let evicted = run(ArchiveRequest { batches: vec![
        vec![vec![1.0, 3.0], vec![2.0, 2.0], vec![3.0, 1.0]], vec![vec![2.0, 3.0]],
        vec![vec![0.5, 0.5]]], ..plain.clone() });
    assert_eq!(evicted.size, 1);
    assert_eq!(evicted.items, vec![vec![0.5, 0.5]]);

    // 重复先到者胜。 / A duplicate keeps the first offer.
    let duplicated = run(ArchiveRequest { batches: vec![vec![vec![1.0, 2.0]],
        vec![vec![1.0, 2.0]]], ..plain.clone() });
    assert_eq!(duplicated.size, 1);

    // 裁剪会重排：容量 3、四点反链 → 幸存者顺序 `[0,3,2]`。 
    // The trim reorders: capacity 3 over four anti-chain points gives survivors `[0,3,2]`.
    let reordered = run(ArchiveRequest { capacity: 3, batches: vec![vec![vec![0.0, 100.0],
        vec![1.0, 99.0], vec![2.0, 1.0], vec![100.0, 0.0]]], ..plain.clone() });
    assert_eq!(reordered.size, 3);
    assert_eq!(reordered.items, vec![vec![0.0, 100.0], vec![100.0, 0.0], vec![2.0, 1.0]]);

    // 容量 0 恒空。 / A zero capacity stays empty.
    let nothing = run(ArchiveRequest { capacity: 0, batches: vec![vec![vec![1.0, 1.0],
        vec![2.0, 2.0]]], ..plain.clone() });
    assert_eq!(nothing.size, 0);
    assert!(nothing.items.is_empty());

    // 整批与逐个在容量 3 的两批用例上分歧。 
    // The batch and single paths disagree on the two-batch capacity-3 case.
    let batches = vec![vec![vec![1.0, 5.0], vec![2.0, 4.0], vec![3.0, 3.0]],
        vec![vec![4.0, 2.0], vec![5.0, 1.0]]];
    let batched = run(ArchiveRequest { capacity: 3, batches: batches.clone(), ..plain.clone() });
    let singled = run(ArchiveRequest { capacity: 3, batches: batches.clone(),
        offer_mode: "single".to_owned(), ..plain.clone() });
    assert_eq!(batched.items, vec![vec![1.0, 5.0], vec![5.0, 1.0], vec![2.0, 4.0]]);
    assert_eq!(singled.items, vec![vec![1.0, 5.0], vec![5.0, 1.0], vec![4.0, 2.0]]);
    // 容量不裁时两条路径一致。 / With no trimming both paths agree.
    let wide_batched = run(ArchiveRequest { capacity: 42, batches: batches.clone(),
        ..plain.clone() });
    let wide_singled = run(ArchiveRequest { capacity: 42, batches,
        offer_mode: "single".to_owned(), ..plain.clone() });
    assert_eq!(wide_batched.items, wide_singled.items);

    // 方向表决定支配方向：两轴最大化 → 两个角点都在。 
    // The direction table decides the sense: maximising both axes keeps both corners.
    let corners = run(ArchiveRequest { mode: "directions".to_owned(), objective_amount: None,
        directions: vec![false, false], batches: vec![vec![vec![9.0, 1.0], vec![1.0, 9.0]]],
        ..plain.clone() });
    assert_eq!(corners.size, 2);

    // 拒绝：未知模式/提交方式、容量上限、参差维度、非有限值、批次上限、缺请求。 
    // Refusals: an unknown mode or offer mode, the capacity cap, ragged axes, a non-finite value, the batch cap, a
    // missing request.
    assert!(verify(ArchiveRequest { mode: "nope".to_owned(), ..plain.clone() }).unwrap_err()
        .message.contains("unknown mode"));
    assert!(verify(ArchiveRequest { offer_mode: "nope".to_owned(), ..plain.clone() })
        .unwrap_err().message.contains("unknown offer mode"));
    assert!(verify(ArchiveRequest { capacity: 65, ..plain.clone() }).unwrap_err()
        .message.contains("at most 64 capacity"));
    assert!(verify(ArchiveRequest { batches: vec![vec![vec![0.0]], vec![vec![0.0, 1.0]]],
        ..plain.clone() }).unwrap_err().message.contains("same axis count"));
    assert!(verify(ArchiveRequest { batches: vec![vec![vec![f64::NAN, 0.0]]],
        ..plain.clone() }).unwrap_err().message.contains("non-finite objective values"));
    assert!(verify(ArchiveRequest { batches: (0..9).map(|_| vec![vec![0.0, 0.0]]).collect(),
        ..plain.clone() }).unwrap_err().message.contains("at most 8 batches"));
    let mut missing_request = base_request();
    missing_request.model_kind = Some(ModelKind::ParetoArchiveVerify);
    assert_eq!(engine.submit(missing_request).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        let payload = serde_json::to_string(&ArchiveRequest { capacity: 3,
            batches: vec![vec![vec![0.0, 100.0], vec![1.0, 99.0], vec![2.0, 1.0],
                vec![100.0, 0.0]]], ..plain }).unwrap();
        std::fs::write(directory.join("pareto-archive.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn constraint_ranking_entry_shows_the_tolerance_inversion() {
    use ospf_rust_bridge::{ConstraintRankingReport, ConstraintRankingRequest,
        RankingAssessmentSpec};

    let engine = NativeEngine::default();
    let verify = |document: ConstraintRankingRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::ConstraintRankingVerify);
        request.constraint_ranking = Some(document);
        engine.submit(request)
    };
    let run = |document: ConstraintRankingRequest| {
        let view = verify(document).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: ConstraintRankingReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        engine.release(&view.id).unwrap();
        report
    };
    let spec = |value: u32, objective: f64, violation: f64| RankingAssessmentSpec { value,
        objective, violation };
    let order = |report: &ConstraintRankingReport| report.ranked.iter().map(|entry| entry.value)
        .collect::<Vec<_>>();
    let plain = ConstraintRankingRequest { mode: "epsilon".to_owned(),
        initial_epsilon: Some(0.0), horizon: Some(1000), exponent: Some(2.0), step: Some(0),
        objective_probability: None, seed: None, entries: Vec::new() };

    // 空排序：八条规则照旧可读。 / An empty ranking keeps the eight rules readable.
    let empty = run(plain.clone());
    assert!(empty.ranked.is_empty());
    assert_eq!(empty.mode, "epsilon");
    assert_eq!(empty.random_state, None);
    assert!(empty.within_the_tolerance_the_objective_decides);
    assert!(empty.tightening_to_zero_restores_a_violation_first_ranking);
    assert!(empty.beyond_the_horizon_behaves_like_the_horizon);
    assert!(empty.an_equal_violation_falls_back_to_the_objective);
    assert!(empty.the_sort_is_stable_so_ties_keep_the_input_order);
    assert!(empty.an_invalid_assessment_is_refused);
    assert!(empty.the_stochastic_ranker_is_reproducible_for_a_fixed_seed);
    assert!(empty.the_stochastic_ranker_compares_feasible_pairs_by_objective);

    // 容差内目标说了算：违约 2.0 的条目排到违约 0 的条目之前。 
    // Inside the tolerance the objective decides: the violation-2.0 entry precedes the feasible one.
    let tolerant = run(ConstraintRankingRequest { initial_epsilon: Some(2.0), horizon: Some(2),
        entries: vec![spec(0, 3.0, 0.0), spec(1, 1.0, 2.0)], ..plain.clone() });
    assert_eq!(order(&tolerant), vec![1, 0]);
    // `step = horizon` → 容差归零 → 违约优先。 / Step == horizon zeroes the tolerance and the violation decides.
    let tightened = run(ConstraintRankingRequest { initial_epsilon: Some(2.0), horizon: Some(2),
        step: Some(2), entries: vec![spec(0, 3.0, 0.0), spec(1, 1.0, 2.0)], ..plain.clone() });
    assert_eq!(order(&tightened), vec![0, 1]);
    // 超过 horizon 与等于 horizon 完全一致。 / Beyond the horizon is identical to at the horizon.
    let beyond = run(ConstraintRankingRequest { initial_epsilon: Some(2.0), horizon: Some(2),
        step: Some(9), entries: vec![spec(0, 3.0, 0.0), spec(1, 1.0, 2.0)], ..plain.clone() });
    assert_eq!(beyond, tightened);

    // 违约相等 → 回落到目标（即使都在容差外）。 
    // An equal violation falls back to the objective even outside the tolerance.
    let equal = run(ConstraintRankingRequest { entries: vec![spec(0, 5.0, 1.5), spec(1, 2.0, 1.5)],
        ..plain.clone() });
    assert_eq!(order(&equal), vec![1, 0]);

    // 全相等 → 保持输入顺序。 / Full ties keep the input order.
    let tied = run(ConstraintRankingRequest { entries: vec![spec(7, 1.0, 1.0), spec(8, 1.0, 1.0),
        spec(9, 1.0, 1.0)], ..plain.clone() });
    assert_eq!(order(&tied), vec![7, 8, 9]);

    // 随机模式：同 seed 完全一致并给出流状态，换 seed 变状态。 
    // Stochastic mode: the same seed agrees exactly and reports the stream state, while another seed moves it.
    let stochastic = ConstraintRankingRequest { mode: "stochastic".to_owned(),
        objective_probability: Some(0.45), seed: Some(42),
        entries: vec![spec(0, 1.0, 2.0), spec(1, 3.0, 0.0), spec(2, 2.0, 1.0)], ..plain.clone() };
    let first = run(stochastic.clone());
    let second = run(stochastic.clone());
    assert_eq!(first, second);
    assert!(first.random_state.is_some());
    let other = run(ConstraintRankingRequest { seed: Some(43), ..stochastic.clone() });
    assert_ne!(other.random_state, first.random_state);

    // 概率 0 时可行对仍按目标排。 / Feasible pairs still sort by objective at probability 0.
    let feasible = run(ConstraintRankingRequest { mode: "stochastic".to_owned(),
        objective_probability: Some(0.0), seed: Some(7),
        entries: vec![spec(0, 3.0, 0.0), spec(1, 1.0, 0.0), spec(2, 2.0, 0.0)], ..plain.clone() });
    assert_eq!(order(&feasible), vec![1, 2, 0]);

    // 拒绝：负违约、非正指数、越界概率、未知模式、上限、非有限值、缺请求。 
    // Refusals: a negative violation, a non-positive exponent, an out-of-range probability, an unknown mode, the caps, a
    // non-finite value, a missing request.
    assert!(verify(ConstraintRankingRequest { entries: vec![spec(0, 1.0, -1.0)],
        ..plain.clone() }).is_err());
    assert!(verify(ConstraintRankingRequest { exponent: Some(0.0), ..plain.clone() }).is_err());
    assert!(verify(ConstraintRankingRequest { mode: "stochastic".to_owned(),
        objective_probability: Some(-1.0), ..plain.clone() }).is_err());
    assert!(verify(ConstraintRankingRequest { mode: "nope".to_owned(), ..plain.clone() })
        .unwrap_err().message.contains("unknown mode"));
    assert!(verify(ConstraintRankingRequest { entries: (0..33)
        .map(|index| spec(index, 1.0, 0.0)).collect(), ..plain.clone() }).unwrap_err()
        .message.contains("at most 32 assessments"));
    assert!(verify(ConstraintRankingRequest { step: Some(2_000_000), ..plain.clone() })
        .unwrap_err().message.contains("below one million"));
    assert!(verify(ConstraintRankingRequest { entries: vec![spec(0, f64::INFINITY, 0.0)],
        ..plain.clone() }).unwrap_err().message.contains("non-finite assessment values"));
    let mut missing_request = base_request();
    missing_request.model_kind = Some(ModelKind::ConstraintRankingVerify);
    assert_eq!(engine.submit(missing_request).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        let payload = serde_json::to_string(&ConstraintRankingRequest {
            initial_epsilon: Some(2.0), horizon: Some(2),
            entries: vec![spec(0, 3.0, 0.0), spec(1, 1.0, 2.0)], ..plain }).unwrap();
        std::fs::write(directory.join("constraint-ranking.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn constraint_priority_entry_shows_the_non_injective_storage_value() {
    use ospf_rust_bridge::{PriorityReport, PriorityRequest};

    let engine = NativeEngine::default();
    let verify = |document: PriorityRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::ConstraintPriorityVerify);
        request.constraint_priority = Some(document);
        engine.submit(request)
    };
    let run = |document: PriorityRequest| {
        let view = verify(document).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: PriorityReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        engine.release(&view.id).unwrap();
        report
    };

    // 空请求：八条规则照旧可读。 / An empty request keeps the eight rules readable.
    let empty = run(PriorityRequest { priorities: Vec::new() });
    assert!(empty.entries.is_empty());
    assert_eq!(empty.non_null_count, 0);
    assert_eq!(empty.basic_count, 0);
    assert_eq!(empty.linear_count, 0);
    assert!(empty.the_named_ladder_is_0_10_50_100);
    assert!(empty.the_storage_round_trip_is_exact_in_one_direction);
    assert!(empty.four_storage_values_have_two_representations);
    assert!(empty.is_non_null_is_exactly_a_nonzero_value);
    assert!(empty.custom_priorities_sit_anywhere_on_the_ladder);
    assert!(empty.the_linear_wrapper_delegates_to_the_basic_count);
    assert!(empty.the_count_reads_storage_values_not_variants);
    assert!(empty.the_wrapper_never_changes_the_storage_value);

    // 四个有名值再加一个自定义值。 / The four named values plus one custom value.
    let named = run(PriorityRequest { priorities: vec![0, 10, 50, 100, 7] });
    assert_eq!(named.entries.iter().map(|entry| entry.variant.clone()).collect::<Vec<_>>(),
        vec!["none", "nice-to-have", "suggested", "mandatory", "custom"]);
    assert_eq!(named.entries.iter().map(|entry| entry.canonical_storage)
        .collect::<Vec<_>>(), vec![0, 10, 50, 100, 7]);
    assert_eq!(named.entries[4].variant_debug, "Custom(7)");
    assert_eq!(named.non_null_count, 4);

    // 只有非零计：`0` 与 `0` 都不计。 / Only non-zero counts.
    let counting = run(PriorityRequest { priorities: vec![0, 50, 7, 0] });
    assert_eq!(counting.non_null_count, 2);
    assert_eq!(counting.basic_count, 2);
    assert_eq!(counting.linear_count, 2);
    assert!(!counting.entries[0].non_null);
    assert!(counting.entries[1].non_null);

    // 极值原样保留。 / The extreme value stays intact.
    let extreme = run(PriorityRequest { priorities: vec![u32::MAX] });
    assert_eq!(extreme.entries[0].storage, u32::MAX);
    assert_eq!(extreme.entries[0].canonical_storage, u32::MAX);
    assert_eq!(extreme.entries[0].variant, "custom");
    assert!(extreme.entries[0].non_null);

    // 拒绝：条目上限、缺请求。 / Refusals: the entry cap and a missing request.
    assert!(verify(PriorityRequest { priorities: (0..33).map(|index| index as u32).collect() })
        .unwrap_err().message.contains("at most 32 priorities"));
    let mut missing_request = base_request();
    missing_request.model_kind = Some(ModelKind::ConstraintPriorityVerify);
    assert_eq!(engine.submit(missing_request).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        let payload = serde_json::to_string(&PriorityRequest {
            priorities: vec![0, 10, 50, 100, 7] }).unwrap();
        std::fs::write(directory.join("constraint-priority.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn population_config_entry_shows_the_builder_only_clamp() {
    use ospf_rust_bridge::{PopulationConfigReport, PopulationConfigRequest};

    let engine = NativeEngine::default();
    let verify = |document: PopulationConfigRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::PopulationConfigVerify);
        request.population_config = Some(document);
        engine.submit(request)
    };
    let run = |document: PopulationConfigRequest| {
        let view = verify(document).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: PopulationConfigReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        engine.release(&view.id).unwrap();
        report
    };
    let plain = PopulationConfigRequest { size: 4, mutation_rate: Some(0.1),
        archive_amount: Some(1), use_builders: Some(true), swarm_size: Some(5),
        migration_period: Some(3) };

    // 缺省：0.1 / 1 / 无评估算子 / 空岛。 / Defaults: 0.1 / 1 / no evaluator / no islands.
    let defaults = run(PopulationConfigRequest { size: 4, mutation_rate: None,
        archive_amount: None, use_builders: Some(true), swarm_size: Some(5),
        migration_period: Some(3) });
    assert_eq!(defaults.mutation_rate, Some(0.1));
    assert_eq!(defaults.archive_amount, 1);
    assert_eq!(defaults.size, 4);
    assert!(defaults.used_builders);
    assert_eq!(defaults.swarm_size, 5);
    assert!(!defaults.swarm_has_evaluator);
    assert_eq!(defaults.migration_period, 3);
    assert_eq!(defaults.island_count, 0);
    assert!(defaults.the_constructor_defaults_are_point_one_and_one);
    assert!(defaults.the_mutation_rate_builder_validates_nothing);
    assert!(defaults.the_archive_amount_builder_clamps_zero_to_one);
    assert!(defaults.the_size_is_never_clamped);
    assert!(defaults.the_builder_returns_the_same_config);
    assert!(defaults.a_struct_literal_bypasses_the_clamp);
    assert!(defaults.the_swarm_config_starts_without_an_evaluator);
    assert!(defaults.the_island_config_has_no_constructor);

    // 同一份 `archiveAmount: 0`：builder 抬成 1、字面量仍是 0。 
    // The same `archiveAmount: 0` becomes 1 through the builder and stays 0 through a literal.
    let built = run(PopulationConfigRequest { archive_amount: Some(0), ..plain.clone() });
    assert_eq!(built.archive_amount, 1);
    let literal = run(PopulationConfigRequest { archive_amount: Some(0),
        use_builders: Some(false), ..plain.clone() });
    assert_eq!(literal.archive_amount, 0);
    assert!(!literal.used_builders);
    // 非零值两条路一样。 / A non-zero value is identical on both paths.
    let five = run(PopulationConfigRequest { archive_amount: Some(5), use_builders: Some(false),
        ..plain.clone() });
    assert_eq!(five.archive_amount, 5);

    // 变异率原样写入（负数与大于 1 照收）。 / The mutation rate is stored verbatim, negatives included.
    let negative = run(PopulationConfigRequest { mutation_rate: Some(-5.0), ..plain.clone() });
    assert_eq!(negative.mutation_rate, Some(-5.0));
    assert!(!negative.mutation_rate_infinite);
    let above = run(PopulationConfigRequest { mutation_rate: Some(7.5), ..plain.clone() });
    assert_eq!(above.mutation_rate, Some(7.5));

    // 规模 0 照收。 / A zero size passes.
    let zero = run(PopulationConfigRequest { size: 0, ..plain.clone() });
    assert_eq!(zero.size, 0);

    // 拒绝：上限、非有限变异率、缺请求。 / Refusals: the cap, a non-finite rate, a missing request.
    assert!(verify(PopulationConfigRequest { size: 2_000_000, ..plain.clone() }).unwrap_err()
        .message.contains("below one million"));
    assert!(verify(PopulationConfigRequest { mutation_rate: Some(f64::INFINITY),
        ..plain.clone() }).unwrap_err().message.contains("non-finite mutation rate"));
    let mut missing_request = base_request();
    missing_request.model_kind = Some(ModelKind::PopulationConfigVerify);
    assert_eq!(engine.submit(missing_request).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        // 载荷故意是"字面量路径 + 容量 0"，读侧要能看到那个钳制被绕过。 
        // The payload deliberately takes the literal path with a zero capacity so the read side sees the bypassed clamp.
        let payload = serde_json::to_string(&PopulationConfigRequest { archive_amount: Some(0),
            use_builders: Some(false), ..plain }).unwrap();
        std::fs::write(directory.join("population-config.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn boundary_entry_shows_the_three_degenerate_paths() {
    use ospf_rust_bridge::{BoundaryReport, BoundaryRequest};

    let engine = NativeEngine::default();
    let verify = |document: BoundaryRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::BoundaryVerify);
        request.boundary = Some(document);
        engine.submit(request)
    };
    let run = |document: BoundaryRequest| {
        let view = verify(document).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: BoundaryReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        engine.release(&view.id).unwrap();
        report
    };
    let plain = BoundaryRequest { value: 1.5, lower_bound: -2.0, upper_bound: 5.0 };

    // 开区间内三路一致。 / Inside the open interval all three agree.
    let inside = run(plain);
    assert_eq!(inside.clamped, Some(1.5));
    assert_eq!(inside.reflected, 1.5);
    assert_eq!(inside.wrapped, 1.5);
    assert!(!inside.clamp_panicked);
    assert!(inside.nan_lands_on_the_lower_bound_for_all_three);
    assert!(inside.the_infinities_diverge_between_clamp_and_the_others);
    assert!(inside.an_inverted_range_panics_only_in_clamp);
    assert!(inside.a_zero_width_clamps_to_the_value_itself);
    assert!(inside.reflection_folds_once_by_mirroring);
    assert!(inside.reflection_gives_up_after_64_periods);
    assert!(inside.wrap_uses_rem_euclid);
    assert!(inside.wrapping_treats_the_interval_as_half_open);

    // 上界外 2：截断停在上界、反射镜像到 3、环绕回卷到 0。 
    // Two above the upper bound: the clamp stops high, reflection mirrors to 3 and wrapping rolls to 0.
    let above = run(BoundaryRequest { value: 7.0, lower_bound: -2.0, upper_bound: 5.0 });
    assert_eq!(above.clamped, Some(5.0));
    assert_eq!(above.reflected, 3.0);
    assert_eq!(above.wrapped, 0.0);

    // 下界外 2：反射到 0、环绕到 3。 / Two below the lower bound: reflection to 0 and wrapping to 3.
    let below = run(BoundaryRequest { value: -4.0, lower_bound: -2.0, upper_bound: 5.0 });
    assert_eq!(below.clamped, Some(-2.0));
    assert_eq!(below.reflected, 0.0);
    assert_eq!(below.wrapped, 3.0);

    // 正好落在上界：环绕跳回下界（半开区间）。 / Exactly on the upper bound wrapping jumps to the lower one.
    let on_upper = run(BoundaryRequest { value: 5.0, lower_bound: -2.0, upper_bound: 5.0 });
    assert_eq!(on_upper.clamped, Some(5.0));
    assert_eq!(on_upper.reflected, 5.0);
    assert_eq!(on_upper.wrapped, -2.0);

    // 倒置区间：只有截断 panic。 / An inverted range panics only the clamp.
    let inverted = run(BoundaryRequest { value: 1.0, lower_bound: 5.0, upper_bound: -2.0 });
    assert!(inverted.clamp_panicked);
    assert_eq!(inverted.clamped, None);
    assert_eq!(inverted.reflected, 5.0);
    assert_eq!(inverted.wrapped, 5.0);

    // 宽度为零：三路都落到那个点。 / A zero width lands on that point on all three.
    let degenerate = run(BoundaryRequest { value: 9.0, lower_bound: 3.0, upper_bound: 3.0 });
    assert_eq!(degenerate.clamped, Some(3.0));
    assert_eq!(degenerate.reflected, 3.0);
    assert_eq!(degenerate.wrapped, 3.0);

    // 远到 130 个宽度：反射被钳在上界；60 个宽度仍精确落回下界。 
    // A hundred and thirty widths away pins reflection high, while sixty widths still lands exactly low.
    let capped = run(BoundaryRequest { value: -2.0 + 130.0 * 7.0, lower_bound: -2.0,
        upper_bound: 5.0 });
    assert_eq!(capped.reflected, 5.0);
    let exact = run(BoundaryRequest { value: -2.0 + 60.0 * 7.0, lower_bound: -2.0,
        upper_bound: 5.0 });
    assert_eq!(exact.reflected, -2.0);

    // 拒绝：非有限输入、缺请求。 / Refusals: non-finite input and a missing request.
    assert!(verify(BoundaryRequest { value: f64::INFINITY, lower_bound: -2.0,
        upper_bound: 5.0 }).unwrap_err().message.contains("non-finite values cannot cross"));
    let mut missing_request = base_request();
    missing_request.model_kind = Some(ModelKind::BoundaryVerify);
    assert_eq!(engine.submit(missing_request).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        // 载荷故意选"正好落在上界"：这一刻三路分家（截断/反射停住、环绕回卷）。 
        // The payload deliberately sits exactly on the upper bound, where the three split apart.
        let payload = serde_json::to_string(&BoundaryRequest { value: 5.0, lower_bound: -2.0,
            upper_bound: 5.0 }).unwrap();
        std::fs::write(directory.join("boundary.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn acceptance_entry_shows_the_tier_gate_and_the_tie_split() {
    use ospf_rust_bridge::{AcceptanceReport, AcceptanceRequest};

    let engine = NativeEngine::default();
    let verify = |document: AcceptanceRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::AcceptanceVerify);
        request.acceptance = Some(document);
        engine.submit(request)
    };
    let run = |document: AcceptanceRequest| {
        let view = verify(document).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: AcceptanceReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        engine.release(&view.id).unwrap();
        report
    };
    let plain = AcceptanceRequest { current_fitness: 3.0, candidate_fitness: 5.0, tier: None,
        comparison: None, temperature: Some(100.0), samples: Some(0), seed: Some(0) };

    // 更差的候选：两种判据都拒绝、`Always` 层级相同仍接受。 
    // A worse candidate: both criteria reject while `Always` accepts at an equal tier.
    let worse = run(plain.clone());
    assert_eq!(worse.ordering, "greater");
    assert!(!worse.better_only);
    assert!(!worse.not_worse);
    assert!(worse.always);
    assert_eq!(worse.metropolis_first, None);
    assert_eq!(worse.metropolis_accepted, 0);
    assert!(worse.better_only_accepts_only_strict_improvement);
    assert!(worse.not_worse_accepts_ties_as_well);
    assert!(worse.a_tie_is_detected_by_both_directions_agreeing);
    assert!(worse.a_none_comparison_falls_back_to_equal);
    assert!(worse.always_acceptance_is_not_unconditional);
    assert!(worse.metropolis_settles_on_the_tier_first);
    assert!(worse.the_temperature_guard_precedes_the_tie_acceptance);
    assert!(worse.metropolis_is_reproducible_and_shrinks_with_delta);

    // 更优的候选：两种判据都接受。 / A better candidate: both accept.
    let better = run(AcceptanceRequest { current_fitness: 5.0, candidate_fitness: 3.0,
        ..plain.clone() });
    assert_eq!(better.ordering, "less");
    assert!(better.better_only);
    assert!(better.not_worse);

    // 平局：只有 `NotWorse` 接受。 / A tie: only `NotWorse` accepts.
    let tied = run(AcceptanceRequest { current_fitness: 3.0, candidate_fitness: 3.0,
        ..plain.clone() });
    assert_eq!(tied.ordering, "equal");
    assert!(!tied.better_only);
    assert!(tied.not_worse);

    // 目标更好但层级更差 → `AlwaysAcceptance` 拒绝。 
    // A better objective on a worse tier is rejected by `AlwaysAcceptance`.
    let gated = run(AcceptanceRequest { current_fitness: 5.0, candidate_fitness: 3.0,
        tier: Some("candidateWorse".to_owned()), ..plain.clone() });
    assert!(gated.better_only);
    assert!(!gated.always);

    // 自相矛盾的比较器读作平局。 / A self-contradictory comparator reads as a tie.
    let contradictory = run(AcceptanceRequest { current_fitness: 3.0, candidate_fitness: 9.0,
        comparison: Some("alwaysLess".to_owned()), ..plain.clone() });
    assert_eq!(contradictory.ordering, "equal");
    assert!(!contradictory.better_only);
    assert!(contradictory.not_worse);

    // `Metropolis`：同 seed 可复现、温度 0 一律拒绝、更优的对在温度之前就接受。 
    // `Metropolis`: reproducible per seed, everything rejected at zero temperature, and an improving pair accepted
    // before the temperature is read.
    let sampling = AcceptanceRequest { current_fitness: 5.0, candidate_fitness: 9.0,
        tier: Some("equal".to_owned()), comparison: None, temperature: Some(10.0),
        samples: Some(200), seed: Some(42) };
    let first = run(sampling.clone());
    let second = run(sampling.clone());
    assert_eq!(first.metropolis_accepted, second.metropolis_accepted);
    assert!(first.metropolis_accepted > 0 && first.metropolis_accepted < 200);
    // 首票只是确定性的一票（`exp(-4/10) ≈ 0.67`，这一票恰好接受），不能断言它必为假。 
    // The first draw is only one deterministic vote (exp(-4/10) is about 0.67 and this one happens to accept), so it
    // cannot be asserted false.
    assert!(first.metropolis_first.is_some());
    let frozen = run(AcceptanceRequest { temperature: Some(0.0), ..sampling.clone() });
    assert_eq!(frozen.metropolis_accepted, 0);
    let improving = run(AcceptanceRequest { current_fitness: 9.0, candidate_fitness: 5.0,
        ..sampling });
    assert_eq!(improving.metropolis_accepted, 200);

    // 拒绝：非有限适应度、未知层级、未知比较器、抽样上限、负温度、缺请求。 
    // Refusals: non-finite fitness, an unknown tier, an unknown comparator, the sample cap, a negative temperature,
    // a missing request.
    assert!(verify(AcceptanceRequest { current_fitness: f64::INFINITY, ..plain.clone() })
        .unwrap_err().message.contains("non-finite fitness"));
    assert!(verify(AcceptanceRequest { tier: Some("nope".to_owned()), ..plain.clone() })
        .unwrap_err().message.contains("unknown tier"));
    assert!(verify(AcceptanceRequest { comparison: Some("nope".to_owned()), ..plain.clone() })
        .unwrap_err().message.contains("unknown comparison"));
    assert!(verify(AcceptanceRequest { samples: Some(20_000), ..plain.clone() })
        .unwrap_err().message.contains("at most 10000 samples"));
    assert!(verify(AcceptanceRequest { temperature: Some(-1.0), ..plain.clone() })
        .unwrap_err().message.contains("finite and nonnegative"));
    let mut missing_request = base_request();
    missing_request.model_kind = Some(ModelKind::AcceptanceVerify);
    assert_eq!(engine.submit(missing_request).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        // 载荷故意让"目标更好但层级更差"：这一刻 `AlwaysAcceptance` 露出它并非无条件。 
        // The payload puts a better objective on a worse tier, which is where `AlwaysAcceptance` shows it is not
        // unconditional.
        let payload = serde_json::to_string(&AcceptanceRequest { current_fitness: 5.0,
            candidate_fitness: 3.0, tier: Some("candidateWorse".to_owned()), comparison: None,
            temperature: Some(10.0), samples: Some(50), seed: Some(7) }).unwrap();
        std::fs::write(directory.join("acceptance.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn ranking_entry_shows_that_the_model_decides_the_order() {
    use ospf_rust_bridge::{RankingItemRow, RankingProbeReport, RankingProbeRequest};

    let engine = NativeEngine::default();
    let verify = |document: RankingProbeRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::RankingProbeVerify);
        request.ranking_probe = Some(document);
        engine.submit(request)
    };
    let run = |document: RankingProbeRequest| {
        let view = verify(document).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: RankingProbeReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        engine.release(&view.id).unwrap();
        report
    };
    let indices = |rows: &[RankingItemRow]| rows.iter().map(|row| row.index).collect::<Vec<_>>();
    let plain = RankingProbeRequest { fitnesses: vec![3.0, 1.0, 2.0], rank_mode: None };

    // 默认排序：适应度 `[3, 1, 2]` 的下标变成 `[1, 2, 0]`，反向是 `[0, 2, 1]`。 
    // With the default the fitness order becomes `[1, 2, 0]` and the reversed variant gives `[0, 2, 1]`.
    let plain_report = run(plain.clone());
    assert_eq!(indices(&plain_report.input), vec![0, 1, 2]);
    assert_eq!(indices(&plain_report.objective), vec![1, 2, 0]);
    assert_eq!(indices(&plain_report.reversed), vec![0, 2, 1]);
    assert_eq!(indices(&plain_report.identity), vec![0, 1, 2]);
    assert!(!plain_report.objective_panicked);
    assert_eq!(plain_report.objective[0].fitness, 1.0);
    assert!(plain_report.objective_ranking_delegates_to_the_model);
    assert!(plain_report.reversed_ranking_flips_the_tie_blocks);
    assert!(plain_report.the_default_rank_indices_is_stable);
    assert!(plain_report.a_duplicate_index_is_cloned_twice);
    assert!(plain_report.an_out_of_range_index_panics);
    assert!(plain_report.identity_ranking_never_touches_the_model);
    assert!(plain_report.ranking_returns_clones_and_leaves_the_input_untouched);
    assert!(plain_report.empty_and_single_inputs_are_fixed_points);

    // 模型反向 → 输出降序（排序算子一个比较都没做）。 
    // A reversing model yields a descending output: the operator compared nothing.
    let reversed_model = run(RankingProbeRequest { fitnesses: vec![1.0, 2.0, 3.0],
        rank_mode: Some("reverse".to_owned()) });
    assert_eq!(indices(&reversed_model.objective), vec![2, 1, 0]);
    assert_eq!(reversed_model.objective[0].fitness, 3.0);

    // 并列：升序保序、反向把并列块倒过来。 
    // Ties: ascending keeps the order while the reversed variant flips the tie block.
    let tied = run(RankingProbeRequest { fitnesses: vec![5.0, 5.0, 5.0], rank_mode: None });
    assert_eq!(indices(&tied.objective), vec![0, 1, 2]);
    assert_eq!(indices(&tied.reversed), vec![2, 1, 0]);

    // 重复下标 → 同一条出现两次。 / A duplicate index shows the same individual twice.
    let duplicate = run(RankingProbeRequest { fitnesses: vec![7.0, 8.0],
        rank_mode: Some("duplicate".to_owned()) });
    assert_eq!(duplicate.objective.len(), 2);
    assert!(duplicate.objective.iter().all(|row| row.index == 0));

    // 越界下标只打崩 `ObjectiveRanking` 与反向那路，`IdentityRanking` 不理模型。 
    // An out-of-range index kills only the objective and reversed paths; IdentityRanking ignores the model.
    let out_of_range = run(RankingProbeRequest { fitnesses: vec![1.0, 2.0],
        rank_mode: Some("outOfRange".to_owned()) });
    assert!(out_of_range.objective_panicked);
    assert!(out_of_range.objective.is_empty());
    assert!(out_of_range.reversed.is_empty());
    assert_eq!(indices(&out_of_range.identity), vec![0, 1]);

    // 丢弃所有下标 → 空输出。 / Dropping every index yields an empty output.
    let dropped = run(RankingProbeRequest { fitnesses: vec![7.0, 8.0],
        rank_mode: Some("drop".to_owned()) });
    assert!(dropped.objective.is_empty());

    // 拒绝：条目上限、非有限适应度、未知模式、缺请求。 
    // Refusals: the entry cap, non-finite fitness, an unknown mode, a missing request.
    assert!(verify(RankingProbeRequest { fitnesses: (0..33).map(f64::from).collect(),
        rank_mode: None }).unwrap_err().message.contains("at most 32 individuals"));
    assert!(verify(RankingProbeRequest { fitnesses: vec![f64::INFINITY], rank_mode: None })
        .unwrap_err().message.contains("non-finite fitness"));
    assert!(verify(RankingProbeRequest { fitnesses: vec![1.0],
        rank_mode: Some("nope".to_owned()) }).unwrap_err().message
        .contains("unknown rank mode"));
    let mut missing_request = base_request();
    missing_request.model_kind = Some(ModelKind::RankingProbeVerify);
    assert_eq!(engine.submit(missing_request).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        // 载荷故意选"三条并列"：升序与反向在这一刻给出互为逆序的两份顺序。 
        // The payload deliberately ties all three so the ascending and reversed orders are exact reverses.
        let payload = serde_json::to_string(&RankingProbeRequest { fitnesses: vec![5.0, 5.0, 5.0],
            rank_mode: None }).unwrap();
        std::fs::write(directory.join("ranking-probe.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn topology_entry_shows_the_boundary_arithmetic() {
    use ospf_rust_bridge::{TopologyReport, TopologyRequest};

    let engine = NativeEngine::default();
    let verify = |document: TopologyRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::TopologyVerify);
        request.topology = Some(document);
        engine.submit(request)
    };
    let run = |document: TopologyRequest| {
        let view = verify(document).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: TopologyReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        engine.release(&view.id).unwrap();
        report
    };
    let plain = TopologyRequest { index: 0, size: 6, radius: Some(1), grid_width: Some(3),
        random_amount: Some(2), seed: Some(0) };

    // 普通查询：环形 `[0, 1, 5]`、k 邻环同、网格左上角 `[0, 1, 3]`。 
    // A plain query: the ring is `[0, 1, 5]`, the k-ring agrees and the grid corner is `[0, 1, 3]`.
    let base = run(plain.clone());
    assert_eq!(base.global, vec![0, 1, 2, 3, 4, 5]);
    assert_eq!(base.ring, vec![0, 1, 5]);
    assert_eq!(base.ring_k, vec![0, 1, 5]);
    assert_eq!(base.von_neumann, vec![0, 1, 3]);
    assert!(base.global_topology_ignores_the_index);
    assert!(base.an_empty_population_still_reports_neighbour_zero);
    assert!(base.a_two_member_ring_is_complete);
    assert!(base.the_k_ring_saturates_at_the_whole_population);
    assert!(base.the_plain_ring_keeps_the_raw_index_while_the_k_ring_wraps_it);
    assert!(base.the_grid_does_not_wrap_at_the_edges);
    assert!(base.a_partial_last_row_drops_the_missing_neighbours);
    assert!(base.the_random_topology_is_seeded_and_keeps_the_index_inside);

    // 空种群：全局给空，其余给 `[0]`。 / An empty population: global empty, the rest `[0]`.
    let empty = run(TopologyRequest { size: 0, ..plain.clone() });
    assert!(empty.global.is_empty());
    assert_eq!(empty.ring, vec![0]);
    assert_eq!(empty.ring_k, vec![0]);
    assert_eq!(empty.von_neumann, vec![0]);

    // 越界下标：普通环留下 7，k 邻环绕回。 / An out-of-range index: the ring keeps 7 while the k-ring wraps.
    let wrapped = run(TopologyRequest { index: 7, size: 3, ..plain.clone() });
    assert_eq!(wrapped.ring, vec![0, 2, 7]);
    assert!(wrapped.ring_k.iter().all(|value| *value < 3));

    // 半径够大就饱和；末行不满会丢邻居。 / A large radius saturates and a partial last row loses neighbours.
    let saturated = run(TopologyRequest { index: 2, size: 6, radius: Some(30), ..plain.clone() });
    assert_eq!(saturated.ring_k, vec![0, 1, 2, 3, 4, 5]);
    let partial = run(TopologyRequest { index: 4, size: 5, grid_width: Some(3), ..plain.clone() });
    assert_eq!(partial.von_neumann, vec![1, 3, 4]);

    // 随机拓扑同 seed 可复现且含自己。 / The random topology is reproducible and keeps the index.
    let random = run(TopologyRequest { index: 2, size: 6, random_amount: Some(3), seed: Some(11),
        ..plain.clone() });
    let again = run(TopologyRequest { index: 2, size: 6, random_amount: Some(3), seed: Some(11),
        ..plain.clone() });
    assert_eq!(random.random, again.random);
    assert!(random.random.contains(&2));

    // 拒绝：规模/下标/半径/数量上限、缺请求。 / Refusals: the caps and a missing request.
    assert!(verify(TopologyRequest { size: 65, ..plain.clone() }).unwrap_err().message
        .contains("the population stays at 64"));
    assert!(verify(TopologyRequest { index: 2_000, ..plain.clone() }).unwrap_err().message
        .contains("the population stays at 64"));
    assert!(verify(TopologyRequest { radius: Some(40), ..plain.clone() }).unwrap_err().message
        .contains("the population stays at 64"));
    let mut missing_request = base_request();
    missing_request.model_kind = Some(ModelKind::TopologyVerify);
    assert_eq!(engine.submit(missing_request).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        // 载荷故意用"空种群"：这一刻三条拓扑都返回一个不存在的邻居 0。 
        // The payload deliberately uses an empty population, where three topologies hand back the non-existent
        // neighbour zero.
        let payload = serde_json::to_string(&TopologyRequest { index: 0, size: 0,
            radius: Some(1), grid_width: Some(4), random_amount: Some(2),
            seed: Some(0) }).unwrap();
        std::fs::write(directory.join("topology.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn migration_entry_shows_the_growing_target_and_the_tie_freeze() {
    use ospf_rust_bridge::{MigrationReport, MigrationRequest};

    let engine = NativeEngine::default();
    let verify = |document: MigrationRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::MigrationVerify);
        request.migration = Some(document);
        engine.submit(request)
    };
    let run = |document: MigrationRequest| {
        let view = verify(document).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: MigrationReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        engine.release(&view.id).unwrap();
        report
    };
    let plain = MigrationRequest { islands: vec![vec![1.0, 2.0, 3.0], vec![4.0, 5.0, 6.0]],
        rate: Some(0.34), amount: Some(1) };

    // 普通两岛：每个岛只动一个（环迁移送来源最优、掉自己最差）。 
    // A plain pair: each island moves one, with the ring sending the source's best and dropping its own worst.
    let base = run(plain.clone());
    assert_eq!(base.ring, vec![vec![0, 1, 3], vec![3, 4, 0]]);
    assert!(base.a_single_island_is_never_migrated);
    assert!(base.the_ring_rate_of_zero_still_migrates_one);
    assert!(base.the_ring_preserves_every_island_size);
    assert!(base.the_ring_moves_the_best_in_and_drops_the_worst);
    assert!(base.an_empty_island_merely_starves_its_successor);
    assert!(base.a_best_to_worst_tie_migrates_nothing);
    assert!(base.the_best_to_worst_target_can_grow);
    assert!(base.an_empty_island_blocks_everything_only_in_best_to_worst);

    // 比例 0 也照迁一个。 / A rate of zero still moves one.
    let zero = run(MigrationRequest { rate: Some(0.0), ..plain.clone() });
    assert_eq!(zero.ring, base.ring);

    // 单岛不动。 / A single island stays put.
    let single = run(MigrationRequest { islands: vec![vec![3.0, 1.0, 2.0]], ..plain.clone() });
    assert_eq!(single.ring, vec![vec![0, 1, 2]]);
    assert_eq!(single.best_to_worst, vec![vec![0, 1, 2]]);

    // 并列时优岛到劣岛一个都不动，而环迁移照旧。 
    // A tie freezes best-to-worst while the ring keeps moving.
    let tied = run(MigrationRequest { islands: vec![vec![5.0, 5.0], vec![5.0, 5.0]],
        rate: Some(0.5), amount: Some(1) });
    assert_eq!(tied.best_to_worst, vec![vec![0, 1], vec![2, 3]]);
    assert_ne!(tied.ring, vec![vec![0, 1], vec![2, 3]]);

    // 目标岛被撑大：从 2 个长到 5 个。 / The target grows from two members to five.
    let grown = run(MigrationRequest { islands: vec![vec![1.0, 2.0, 3.0, 4.0, 5.0],
        vec![9.0, 8.0]], amount: Some(5), ..plain.clone() });
    assert_eq!(grown.best_to_worst[1].len(), 5);
    assert_eq!(grown.best_to_worst[1], vec![0, 1, 2, 3, 4]);
    // 迁移者是**克隆**进目标的：来源岛仍留着原件，所以同一个编号在两处出现。 
    // Migrants are **cloned** into the target while the source keeps the original, so one id appears in both places.
    assert_eq!(grown.best_to_worst[0], vec![0, 1, 2, 3, 4]);

    // 空岛：环迁移在空岛旁边仍搬动，优岛到劣岛整体放弃。 
    // An empty island: the ring still moves around it while best-to-worst gives up entirely.
    let starved = run(MigrationRequest { islands: vec![vec![1.0, 2.0], vec![3.0],
        Vec::new()], ..plain.clone() });
    assert_eq!(starved.best_to_worst, vec![vec![0, 1], vec![2], Vec::new()]);
    assert_eq!(starved.ring[1], vec![0]);

    // 拒绝：岛数上限、岛内上限、非有限适应度、比例越界、迁移量上限、缺请求。 
    // Refusals: the island cap, the member cap, non-finite fitness, the rate bounds, the amount cap and a missing request.
    assert!(verify(MigrationRequest { islands: vec![Vec::new(); 5], ..plain.clone() })
        .unwrap_err().message.contains("at most four islands"));
    assert!(verify(MigrationRequest { islands: vec![vec![1.0; 9]], ..plain.clone() })
        .unwrap_err().message.contains("at most eight individuals"));
    assert!(verify(MigrationRequest { islands: vec![vec![f64::INFINITY]], ..plain.clone() })
        .unwrap_err().message.contains("non-finite fitness"));
    assert!(verify(MigrationRequest { rate: Some(2.0), ..plain.clone() }).unwrap_err()
        .message.contains("the rate must stay inside [0, 1]"));
    let mut missing_request = base_request();
    missing_request.model_kind = Some(ModelKind::MigrationVerify);
    assert_eq!(engine.submit(missing_request).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        // 载荷故意让"来源岛比目标岛大"：这一刻目标岛被撑大。 
        // The payload makes the source island larger than the target, which is where the target grows.
        let payload = serde_json::to_string(&MigrationRequest { islands: vec![
            vec![1.0, 2.0, 3.0, 4.0, 5.0], vec![9.0, 8.0]], rate: Some(0.1),
            amount: Some(5) }).unwrap();
        std::fs::write(directory.join("migration.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn archive_entry_shows_the_flag_belonging_to_the_archive_not_the_batch() {
    use ospf_rust_bridge::{ArchiveProbeReport, ArchiveProbeRequest};

    let engine = NativeEngine::default();
    let verify = |document: ArchiveProbeRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::ArchiveProbeVerify);
        request.archive_probe = Some(document);
        engine.submit(request)
    };
    let run = |document: ArchiveProbeRequest| {
        let view = verify(document).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: ArchiveProbeReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        engine.release(&view.id).unwrap();
        report
    };
    let plain = ArchiveProbeRequest { batches: vec![vec![5.0], vec![9.0, 8.0], vec![4.0]],
        amount: Some(3) };

    // 三轮：首轮真、差批假（但候选仍进档案）、第三轮 4 更优 → 真且排首位。 
    // Three rounds: true, then false on a worse batch that is nevertheless archived, then four wins and heads the list.
    let base = run(plain.clone());
    assert_eq!(base.best[0].members, vec![0]);
    assert_eq!(base.best[1].members, vec![0, 2, 1]);
    assert_eq!(base.best[2].members, vec![3, 0, 2]);
    assert!(base.best[0].improved && !base.best[1].improved && base.best[2].improved);
    assert_eq!(base.history[2].members, vec![3]);
    assert!(base.the_first_update_always_reports_improvement);
    assert!(base.a_zero_capacity_archive_swallows_everything);
    assert!(base.an_equal_candidate_is_stored_but_never_reported);
    assert!(base.the_flag_tracks_the_archive_best);
    assert!(base.a_tie_keeps_the_older_entry_first);
    assert!(base.clear_resets_the_flag);
    assert!(base.the_history_archive_reports_any_improvement);
    assert!(base.the_history_archive_never_takes_an_equal_candidate);

    // 首轮塞最差的也报真。 / The first round reports true even for the worst candidate.
    let worst = run(ArchiveProbeRequest { batches: vec![vec![100.0]], amount: Some(2) });
    assert!(worst.best[0].improved && worst.history[0].improved);

    // 容量 0 吞掉一切，而历史档案不受影响。 / Capacity zero swallows everything while history is unaffected.
    let zero = run(ArchiveProbeRequest { batches: vec![vec![1.0, 2.0], vec![3.0]],
        amount: Some(0) });
    assert!(zero.best.iter().all(|row| !row.improved && row.members.is_empty()));
    assert!(zero.history[0].improved);
    assert_eq!(zero.history[1].members, vec![0]);

    // 等值候选：精英档案内容变长却不报改进，历史档案拒收并留住原个体。 
    // An equal candidate grows the elite archive without reporting, while history refuses it and keeps the incumbent.
    let equal = run(ArchiveProbeRequest { batches: vec![vec![5.0], vec![5.0]],
        amount: Some(3) });
    assert!(equal.best[0].improved && !equal.best[1].improved);
    assert_eq!(equal.best[1].members, vec![0, 1]);
    assert!(!equal.history[1].improved);
    assert_eq!(equal.history[1].members, vec![0]);

    // 容量截断：只留最优两个。 / The capacity truncates: only the two best survive.
    let truncated = run(ArchiveProbeRequest { batches: vec![vec![5.0, 6.0, 7.0]],
        amount: Some(2) });
    assert_eq!(truncated.best[0].members, vec![0, 1]);

    // 拒绝：批数上限、批内上限、非有限适应度、容量上限、缺请求。 
    // Refusals: the batch cap, the candidate cap, non-finite fitness, the capacity cap and a missing request.
    assert!(verify(ArchiveProbeRequest { batches: vec![Vec::new(); 9], amount: Some(1) })
        .unwrap_err().message.contains("at most eight batches"));
    assert!(verify(ArchiveProbeRequest { batches: vec![vec![1.0; 9]], amount: Some(1) })
        .unwrap_err().message.contains("at most eight candidates"));
    assert!(verify(ArchiveProbeRequest { batches: vec![vec![f64::INFINITY]], amount: Some(1) })
        .unwrap_err().message.contains("non-finite fitness"));
    assert!(verify(ArchiveProbeRequest { batches: vec![vec![1.0]], amount: Some(9) })
        .unwrap_err().message.contains("the capacity stays at eight"));
    let mut missing_request = base_request();
    missing_request.model_kind = Some(ModelKind::ArchiveProbeVerify);
    assert_eq!(engine.submit(missing_request).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        // 载荷故意让"第二轮是等值候选"：精英档案因此变长却不报改进。 
        // The payload makes the second batch an equal candidate, so the elite archive grows without reporting.
        let payload = serde_json::to_string(&ArchiveProbeRequest { batches: vec![vec![5.0],
            vec![5.0]], amount: Some(3) }).unwrap();
        std::fs::write(directory.join("archive-probe.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn piecewise_entry_shows_the_constructor_gates_and_the_silent_rewrite() {
    use ospf_rust_bridge::{PiecewisePointSpec, PiecewiseReport, PiecewiseRequest};

    let engine = NativeEngine::default();
    let verify = |document: PiecewiseRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::PiecewiseVerify);
        request.piecewise = Some(document);
        engine.submit(request)
    };
    let run = |document: PiecewiseRequest| {
        let view = verify(document).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: PiecewiseReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        engine.release(&view.id).unwrap();
        report
    };
    let point = |x: f64, y: f64| PiecewisePointSpec { x, y };
    let plain = PiecewiseRequest { name: Some("f".to_owned()),
        points: vec![point(0.0, 0.0), point(1.0, 1.0), point(2.0, 4.0)],
        breakpoints: Vec::new(), slopes: Vec::new(), intercepts: Vec::new(),
        use_segments: None };

    // 三点：λ 三个、选择器两个、值域 `[0, 4]`、`display()` 带名字。 
    // Three points give three lambdas, two selectors, the range `[0, 4]` and a named `display()`.
    let base = run(plain.clone());
    assert!(base.constructed);
    assert_eq!(base.lambda_count, 3);
    assert_eq!(base.selector_count, 2);
    assert_eq!(base.result_lower, Some(0.0));
    assert_eq!(base.result_upper, Some(4.0));
    assert_eq!(base.display, "ulp(f)");
    assert_eq!(base.points.len(), 3);
    assert!(base.at_least_two_points_are_required);
    assert!(base.the_x_values_must_strictly_ascend);
    assert!(base.non_finite_coordinates_are_refused);
    assert!(base.the_result_range_is_the_y_envelope);
    assert!(base.the_lambda_and_selector_counts_follow_the_point_count);
    assert!(base.segments_must_match_the_breakpoint_count);
    assert!(base.an_inconsistent_segment_graph_is_silently_rewritten);
    assert!(base.the_display_wraps_the_name);

    // 点数不足与 x 不递增都 panic（被兜住 → `constructed: false`）。 
    // Too few points and a non-ascending x both panic, caught as `constructed: false`.
    let single = run(PiecewiseRequest { points: vec![point(0.0, 1.0)], ..plain.clone() });
    assert!(!single.constructed);
    let equal = run(PiecewiseRequest { points: vec![point(1.0, 0.0), point(1.0, 5.0)],
        ..plain.clone() });
    assert!(!equal.constructed);
    let descending = run(PiecewiseRequest { points: vec![point(2.0, 0.0), point(1.0, 5.0)],
        ..plain.clone() });
    assert!(!descending.constructed);

    // 值域是 y 的包络。 / The range is the y envelope.
    let envelope = run(PiecewiseRequest { points: vec![point(0.0, 5.0), point(1.0, 1.0),
        point(2.0, 9.0)], ..plain.clone() });
    assert_eq!(envelope.result_lower, Some(1.0));
    assert_eq!(envelope.result_upper, Some(9.0));

    // 不一致的线段图被静默改写：断点 1 取右边那条线段的 101。 
    // The inconsistent segment graph is silently rewritten: breakpoint one takes 101 from the right-hand segment.
    let rewritten = run(PiecewiseRequest { use_segments: Some(true),
        breakpoints: vec![0.0, 1.0, 2.0], slopes: vec![1.0, 1.0], intercepts: vec![0.0, 100.0],
        ..plain.clone() });
    assert!(rewritten.constructed);
    assert!((rewritten.points[1].y - 101.0).abs() < 1e-12);
    assert!((rewritten.points[2].y - 102.0).abs() < 1e-12);

    // 线段数与断点数不匹配 → panic。 / A mismatched segment count panics.
    let mismatched = run(PiecewiseRequest { use_segments: Some(true),
        breakpoints: vec![0.0, 1.0, 2.0], slopes: vec![1.0], intercepts: vec![0.0, 0.0],
        ..plain.clone() });
    assert!(!mismatched.constructed);

    // 拒绝：点数上限、缺请求。 / Refusals: the point cap and a missing request.
    assert!(verify(PiecewiseRequest { points: (0..9).map(|index| point(index as f64, 0.0))
        .collect(), ..plain.clone() }).unwrap_err().message.contains("at most eight points"));
    let mut missing_request = base_request();
    missing_request.model_kind = Some(ModelKind::PiecewiseVerify);
    assert_eq!(engine.submit(missing_request).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        // 载荷故意用"不一致的线段图"：这一刻构造器静默改写了断点上的值。 
        // The payload uses an inconsistent segment graph, where the constructor silently rewrites the breakpoint value.
        let payload = serde_json::to_string(&PiecewiseRequest { name: Some("gap".to_owned()),
            points: Vec::new(), breakpoints: vec![0.0, 1.0, 2.0], slopes: vec![1.0, 1.0],
            intercepts: vec![0.0, 100.0], use_segments: Some(true) }).unwrap();
        std::fs::write(directory.join("piecewise.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn diversity_entry_shows_the_two_metrics_disagreeing_about_missing_keys() {
    use ospf_rust_bridge::{DiversityReport, DiversityRequest};

    let engine = NativeEngine::default();
    let verify = |document: DiversityRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::DiversityVerify);
        request.diversity = Some(document);
        engine.submit(request)
    };
    let run = |document: DiversityRequest| {
        let view = verify(document).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: DiversityReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        engine.release(&view.id).unwrap();
        report
    };
    let values = |items: &[f64]| items.iter().map(|item| Some(*item)).collect::<Vec<_>>();
    let plain = DiversityRequest { solutions: vec![values(&[0.0]), values(&[3.0]),
        values(&[4.0])], order_len: Some(1) };

    // `0 / 3 / 4` → 距离均值 `8/3`（不是平方均值 `26/3`）。 
    // `0 / 3 / 4` gives the mean distance `8/3`, not the squared mean `26/3`.
    let base = run(plain.clone());
    let mean = base.mean.unwrap();
    assert!((mean - 8.0 / 3.0).abs() < 1e-12);
    assert!(!base.mean_nan);
    assert_eq!(base.first_entry_count, 1);
    assert_eq!(base.order_len, 1);
    assert!(base.the_mean_returns_zero_below_two_solutions);
    assert!(base.the_mean_takes_its_dimension_from_the_first_solution);
    assert!(base.a_missing_value_poisons_the_mean);
    assert!(base.the_mean_of_identical_solutions_is_zero);
    assert!(base.the_mean_averages_distances_not_their_squares);
    assert!(base.the_fractal_metric_reads_a_missing_value_as_zero);
    assert!(base.the_fractal_metric_sees_every_order_position);
    assert!(base.both_metrics_collapse_a_degenerate_population_to_zero);

    // 不到两个解读 0。 / Fewer than two solutions reads zero.
    let single = run(DiversityRequest { solutions: vec![values(&[4.0])], order_len: Some(1) });
    assert_eq!(single.mean, Some(0.0));
    assert_eq!(single.fractal, 0.0);

    // 缺一个键：均值 NaN、分形有限 —— 两个度量对缺值答案不同。 
    // One missing key: the mean is NaN and the fractal stays finite, so the metrics answer differently.
    let missing = run(DiversityRequest { solutions: vec![values(&[1.0, 2.0]),
        vec![Some(1.0), None]], order_len: Some(2) });
    assert!(missing.mean_nan);
    assert_eq!(missing.mean, None);
    assert!(missing.fractal.is_finite());
    assert_eq!(missing.first_entry_count, 2);

    // 同一份几何：键装在第一个解上 → 100，装在第二个解上 → 0。 
    // One geometry: keys on the first solution read 100 while keys on the second read 0.
    let deep = run(DiversityRequest { solutions: vec![values(&[0.0, 0.0, 0.0]),
        values(&[0.0, 100.0, 0.0])], order_len: Some(3) });
    assert_eq!(deep.mean, Some(100.0));
    let shallow = run(DiversityRequest { solutions: vec![vec![Some(0.0), None, None],
        values(&[0.0, 100.0, 0.0])], order_len: Some(3) });
    assert_eq!(shallow.mean, Some(0.0));
    // 分形读数在两种装法下一致（它把缺失的键补 0）。 
    // The fractal reading agrees between the two layouts because it pads missing keys with zeros.
    assert!((deep.fractal - shallow.fractal).abs() < 1e-12);

    // 全同点：两个度量都是 0。 / Identical solutions read zero on both.
    let identical = run(DiversityRequest { solutions: vec![values(&[3.0, 1.0]),
        values(&[3.0, 1.0]), values(&[3.0, 1.0])], order_len: Some(2) });
    assert_eq!(identical.mean, Some(0.0));
    assert_eq!(identical.fractal, 0.0);

    // 拒绝：解数上限、条目上限、非有限值、`order` 上限、缺请求。 
    // Refusals: the solution cap, the entry cap, non-finite values, the order cap and a missing request.
    assert!(verify(DiversityRequest { solutions: vec![values(&[1.0]); 9], order_len: Some(1) })
        .unwrap_err().message.contains("at most eight solutions"));
    assert!(verify(DiversityRequest { solutions: vec![values(&[1.0; 9])], order_len: Some(1) })
        .unwrap_err().message.contains("at most eight entries"));
    assert!(verify(DiversityRequest { solutions: vec![values(&[f64::NAN])],
        order_len: Some(1) }).unwrap_err().message.contains("non-finite values"));
    assert!(verify(DiversityRequest { solutions: vec![values(&[1.0])], order_len: Some(9) })
        .unwrap_err().message.contains("at most eight ids"));
    let mut missing_request = base_request();
    missing_request.model_kind = Some(ModelKind::DiversityVerify);
    assert_eq!(engine.submit(missing_request).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        // 载荷故意让"第二个解缺一个键"：这一刻两个度量给出不同答案。 
        // The payload makes the second solution miss one key, where the two metrics answer differently.
        let payload = serde_json::to_string(&DiversityRequest { solutions: vec![
            vec![Some(1.0), Some(2.0)], vec![Some(1.0), None]], order_len: Some(2) }).unwrap();
        std::fs::write(directory.join("diversity.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn solution_status_entry_shows_the_six_statuses_and_the_unguarded_objective() {
    use ospf_rust_bridge::{SolutionStatusReport, SolutionStatusRequest};

    let engine = NativeEngine::default();
    let verify = |document: SolutionStatusRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::SolutionStatusVerify);
        request.solution_status = Some(document);
        engine.submit(request)
    };
    let run = |document: SolutionStatusRequest| {
        let view = verify(document).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: SolutionStatusReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        engine.release(&view.id).unwrap();
        report
    };
    let plain = SolutionStatusRequest { solution: vec![1.0, 2.0],
        mode: Some("optimal".to_owned()), status: None, attach_objective: None,
        objective: Some(7.0) };

    // `optimal` 自带目标值，两个判据都真。 / `optimal` carries the objective and satisfies both predicates.
    let base = run(plain.clone());
    assert_eq!(base.status, "Optimal");
    assert!(base.is_optimal && base.is_feasible);
    assert_eq!(base.objective, Some(7.0));
    assert_eq!(base.solution_len, 2);
    assert!(base.the_default_is_not_solved_with_an_empty_solution);
    assert!(base.the_status_enum_has_six_variants_defaulting_to_not_solved);
    assert!(base.only_the_optimal_constructor_fills_the_objective);
    assert!(base.with_status_drops_the_objective);
    assert!(base.optimal_is_a_strict_subset_of_feasible);
    assert!(base.unbounded_and_unknown_are_not_feasible);
    assert!(base.the_infeasible_constructor_carries_an_empty_solution);
    assert!(base.with_objective_never_checks_the_status);

    // 默认是 NotSolved + 空解。 / The default is NotSolved with an empty solution.
    let fresh = run(SolutionStatusRequest { solution: Vec::new(), mode: Some("new".to_owned()),
        ..plain.clone() });
    assert_eq!(fresh.status, "NotSolved");
    assert!(!fresh.is_feasible && !fresh.is_optimal);
    assert_eq!(fresh.solution_len, 0);
    assert_eq!(fresh.objective, None);

    // `with_status` 丢掉目标值，链一次 `with_objective` 才回来。 
    // `with_status` drops the objective; chaining `with_objective` brings it back.
    let bare = run(SolutionStatusRequest { mode: Some("withStatus".to_owned()),
        status: Some("Feasible".to_owned()), ..plain.clone() });
    assert_eq!(bare.objective, None);
    assert!(bare.is_feasible && !bare.is_optimal);
    let attached = run(SolutionStatusRequest { mode: Some("withStatus".to_owned()),
        status: Some("Feasible".to_owned()), attach_objective: Some(true), ..plain.clone() });
    assert_eq!(attached.objective, Some(7.0));

    // 四个"不算可行"的状态。 / Four statuses that do not count as feasible.
    for name in ["Unbounded", "Unknown", "Infeasible", "NotSolved"] {
        let report = run(SolutionStatusRequest { mode: Some("withStatus".to_owned()),
            status: Some(name.to_owned()), ..plain.clone() });
        assert_eq!(report.status, name);
        assert!(!report.is_feasible && !report.is_optimal, "{name}");
    }

    // `infeasible()` 丢掉解，却照样能挂目标值。 
    // `infeasible()` drops the solution yet still accepts an objective.
    let infeasible = run(SolutionStatusRequest { mode: Some("infeasible".to_owned()),
        attach_objective: Some(true), ..plain.clone() });
    assert_eq!(infeasible.status, "Infeasible");
    assert_eq!(infeasible.solution_len, 0);
    assert_eq!(infeasible.objective, Some(7.0));
    assert!(!infeasible.is_feasible);
    // 对照组：同一个状态走 `withStatus` 就**保留**了调用方给的解 —— 空解是构造器的、不是状态的。 
    // Control: the same status through `withStatus` **keeps** the caller's solution, so emptiness is the constructor's.
    let kept = run(SolutionStatusRequest { mode: Some("withStatus".to_owned()),
        status: Some("Infeasible".to_owned()), ..plain.clone() });
    assert_eq!(kept.status, "Infeasible");
    assert_eq!(kept.solution_len, 2);

    // 拒绝：条目上限、非有限值、未知模式、未知状态名、缺请求。 
    // Refusals: the entry cap, non-finite values, an unknown mode, an unknown status name and a missing request.
    assert!(verify(SolutionStatusRequest { solution: vec![1.0; 9], ..plain.clone() })
        .unwrap_err().message.contains("at most eight entries"));
    assert!(verify(SolutionStatusRequest { solution: vec![f64::INFINITY], ..plain.clone() })
        .unwrap_err().message.contains("non-finite values"));
    assert!(verify(SolutionStatusRequest { mode: Some("bogus".to_owned()), ..plain.clone() })
        .unwrap_err().message.contains("unknown mode"));
    assert!(verify(SolutionStatusRequest { mode: Some("withStatus".to_owned()),
        status: Some("Nearly".to_owned()), ..plain.clone() }).unwrap_err()
        .message.contains("unknown status name"));
    let mut missing_request = base_request();
    missing_request.model_kind = Some(ModelKind::SolutionStatusVerify);
    assert_eq!(engine.submit(missing_request).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        // 载荷故意给 `Infeasible` 挂目标值：这一刻"不校验状态"被看见。 
        // The payload attaches an objective to `Infeasible`, which is where the missing status check shows up.
        let payload = serde_json::to_string(&SolutionStatusRequest { solution: vec![4.0, 5.0],
            mode: Some("withStatus".to_owned()), status: Some("Infeasible".to_owned()),
            attach_objective: Some(true), objective: Some(-2.5) }).unwrap();
        std::fs::write(directory.join("solution-status.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn funnel_entry_shows_the_nan_dual_and_the_shortlist_order() {
    use ospf_rust_bridge::{FunnelReport, FunnelRequest, FunnelProbeSpec};

    let engine = NativeEngine::default();
    let verify = |document: FunnelRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::FunnelVerify);
        request.funnel = Some(document);
        engine.submit(request)
    };
    let run = |document: FunnelRequest| {
        let view = verify(document).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: FunnelReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        engine.release(&view.id).unwrap();
        report
    };
    let spec = |id: &str, activity: &str, dual: Option<f64>| FunnelProbeSpec {
        id: id.to_owned(), activity: activity.to_owned(), dual };
    let plain = FunnelRequest { candidates: vec![spec("c", "Inactive", None),
        spec("a", "Active", Some(1.0)), spec("b", "Active", Some(0.0))],
        dual_threshold: Some(1e-9), shortlist_limit: Some(2) };

    // 顺序故意是 C、A、B：短名单先收 A/B，保持既有顺序。 
    // The order is deliberately C, A, B: the shortlist takes A and B in their existing order.
    let base = run(plain.clone());
    assert_eq!(base.tier_a, 1);
    assert_eq!(base.tier_b, 1);
    assert_eq!(base.tier_c, 1);
    assert_eq!(base.unclassified, 0);
    assert_eq!(base.shortlist, vec!["a".to_owned(), "b".to_owned()]);
    assert!(base.ranking_valid);
    assert!(base.bad_counts_refused);
    // 短名与线上名并存。 / The short and wire names both appear.
    assert_eq!(base.rows[1].tier, "A");
    assert_eq!(base.rows[1].wire, "TIER_A");
    assert_eq!(base.rows[0].tier, "C");
    assert_eq!(base.rows[0].wire, "TIER_C");
    assert!(base.the_short_tier_names_differ_from_the_wire_names);
    assert!(base.nearly_active_shares_the_active_bucket);
    assert!(base.an_active_candidate_without_a_dual_is_unclassified);
    assert!(base.an_inactive_candidate_is_tier_c_whatever_its_dual);
    assert!(base.a_nan_dual_lands_in_tier_b);
    assert!(base.the_classifier_ignores_the_threshold_and_the_config_the_limit);
    assert!(base.the_shortlist_keeps_the_existing_order);
    assert!(base.a_violated_candidate_is_unclassified);

    // `NaN` 对偶落进 Tier B（而且排序校验照过）。 
    // A `NaN` dual lands in Tier B and the ranking still validates.
    let nan = run(FunnelRequest { candidates: vec![spec("n", "Active", Some(f64::NAN))],
        dual_threshold: Some(1e-9), shortlist_limit: Some(1) });
    assert_eq!(nan.rows[0].tier, "B");
    assert!(nan.ranking_valid);

    // active 但没有对偶 → 未分层，只在名额剩时进短名单。 
    // Active without a dual is unclassified and only enters the shortlist while slots remain.
    let missing = run(FunnelRequest { candidates: vec![spec("a", "Active", None)],
        dual_threshold: Some(1e-9), shortlist_limit: Some(2) });
    assert_eq!(missing.rows[0].tier, "UNCLASSIFIED");
    assert_eq!(missing.shortlist, vec!["a".to_owned()]);

    // `Violated` 与 `Unknown` 都是未分层，`Inactive` 才是 Tier C。 
    // `Violated` and `Unknown` are unclassified while `Inactive` is the Tier C case.
    let violated = run(FunnelRequest { candidates: vec![spec("v", "Violated", Some(1.0)),
        spec("u", "Unknown", None), spec("i", "Inactive", Some(1.0))],
        dual_threshold: Some(1e-9), shortlist_limit: Some(8) });
    assert_eq!(violated.rows[0].tier, "UNCLASSIFIED");
    assert_eq!(violated.rows[1].tier, "UNCLASSIFIED");
    assert_eq!(violated.rows[2].tier, "C");
    assert_eq!(violated.unclassified, 2);
    assert_eq!(violated.tier_c, 1);

    // 负阈值：分类器让 active 进 Tier A，但排序校验拒绝。 
    // A negative threshold pushes active candidates into Tier A while the ranking validation refuses it.
    let negative = run(FunnelRequest { candidates: vec![spec("a", "Active", Some(0.0))],
        dual_threshold: Some(-1.0), shortlist_limit: Some(1) });
    assert_eq!(negative.rows[0].tier, "A");
    assert!(!negative.ranking_valid);

    // 上限 0 → 空短名单；上限 8 → 全部。 / A limit of zero empties the shortlist; eight takes everything.
    assert!(run(FunnelRequest { shortlist_limit: Some(0), ..plain.clone() }).shortlist.is_empty());
    assert_eq!(run(FunnelRequest { shortlist_limit: Some(8), ..plain.clone() }).shortlist,
        vec!["a".to_owned(), "b".to_owned(), "c".to_owned()]);

    // 拒绝：候选数上限、空身份、未知活动性、非有限阈值、上限越界、缺请求。 
    // Refusals: the candidate cap, a blank identity, an unknown activity, a non-finite threshold, the limit cap and a
    // missing request.
    assert!(verify(FunnelRequest { candidates: (0..9).map(|index|
        spec(&format!("c{index}"), "Active", Some(1.0))).collect(), ..plain.clone() })
        .unwrap_err().message.contains("at most eight candidates"));
    assert!(verify(FunnelRequest { candidates: vec![spec("  ", "Active", Some(1.0))],
        ..plain.clone() }).unwrap_err().message.contains("may not be blank"));
    assert!(verify(FunnelRequest { candidates: vec![spec("a", "Nearly", Some(1.0))],
        ..plain.clone() }).unwrap_err().message.contains("unknown activity name"));
    assert!(verify(FunnelRequest { dual_threshold: Some(f64::INFINITY), ..plain.clone() })
        .unwrap_err().message.contains("must be finite"));
    assert!(verify(FunnelRequest { shortlist_limit: Some(9), ..plain.clone() })
        .unwrap_err().message.contains("the limit stays at eight"));
    let mut missing_request = base_request();
    missing_request.model_kind = Some(ModelKind::FunnelVerify);
    assert_eq!(engine.submit(missing_request).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        // 载荷故意让"active 但没有对偶"：这一刻它只能停在未分层。 
        // The payload makes an active candidate without a dual, which can only stay unclassified.
        let payload = serde_json::to_string(&FunnelRequest { candidates: vec![
            spec("a", "Active", None), spec("b", "Active", Some(1.0)),
            spec("c", "Inactive", None), spec("d", "Violated", Some(5.0))],
            dual_threshold: Some(1e-9), shortlist_limit: Some(3) }).unwrap();
        std::fs::write(directory.join("funnel.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn effectiveness_entry_shows_the_unvalidated_order_and_the_positive_only_rule() {
    use ospf_rust_bridge::{EffectivenessProbeReport, EffectivenessProbeRequest,
        EffectivenessProbeSpec};

    let engine = NativeEngine::default();
    let verify = |document: EffectivenessProbeRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::EffectivenessProbeVerify);
        request.effectiveness_probe = Some(document);
        engine.submit(request)
    };
    let run = |document: EffectivenessProbeRequest| {
        let view = verify(document).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: EffectivenessProbeReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        engine.release(&view.id).unwrap();
        report
    };
    let spec = |id: &str, improvement: Option<f64>| EffectivenessProbeSpec {
        id: id.to_owned(), dual: None, local_effective: None, removal_effective: None,
        first_effective_delta: None, max_improvement: improvement, marginal_effect: None,
        status: None, unavailable_reason: None };
    let plain = EffectivenessProbeRequest { baseline_objective: Some(10.0),
        schema_version: Some("probe".to_owned()),
        entries: vec![spec("a", Some(3.0)), spec("b", Some(0.0)), spec("c", None)] };

    // 只有严格正的改善进全局列表，且升序/降序都通过校验。 
    // Only a strictly positive improvement reaches the global list, and both orders validate.
    let base = run(plain.clone());
    assert_eq!(base.globally_effective, vec!["a".to_owned()]);
    assert!(base.ranking_valid);
    assert!(base.reversed_still_valid);
    assert_eq!(base.rows[0].globally_effective, true);
    assert_eq!(base.rows[1].globally_effective, false);
    assert_eq!(base.rows[2].globally_effective, false);
    assert!(base.global_effectiveness_needs_a_positive_improvement);
    assert!(base.local_effectiveness_needs_exactly_some_true);
    assert!(base.the_order_of_entries_is_never_validated);
    assert!(base.the_first_delta_must_be_positive);
    assert!(base.a_local_verdict_requires_a_dual);
    assert!(base.a_removal_verdict_requires_a_supported_status);
    assert!(base.the_reason_is_never_tied_to_the_status);
    assert!(base.blank_and_duplicate_identities_are_refused);

    // 负改善：不算有效，也不被拒。 / A negative improvement is neither effective nor refused.
    let negative = run(EffectivenessProbeRequest { entries: vec![spec("a", Some(-5.0))],
        ..plain.clone() });
    assert_eq!(negative.rows[0].globally_effective, false);
    assert!(negative.ranking_valid);
    assert!(negative.globally_effective.is_empty());

    // 局部有效需要对偶。 / Local effectiveness needs a dual.
    let unpaired = run(EffectivenessProbeRequest { entries: vec![EffectivenessProbeSpec {
        local_effective: Some(true), ..spec("a", None) }], ..plain.clone() });
    assert!(!unpaired.ranking_valid);
    let paired = run(EffectivenessProbeRequest { entries: vec![EffectivenessProbeSpec {
        local_effective: Some(true), dual: Some(0.5), ..spec("a", None) }], ..plain.clone() });
    assert!(paired.ranking_valid);
    assert_eq!(paired.locally_effective, vec!["a".to_owned()]);

    // `first_effective_delta` 必须为正。 / `first_effective_delta` must be positive.
    for value in [0.0, -1.0] {
        let refused = run(EffectivenessProbeRequest { entries: vec![EffectivenessProbeSpec {
            first_effective_delta: Some(value), ..spec("a", None) }], ..plain.clone() });
        assert!(!refused.ranking_valid, "{value}");
    }
    let accepted = run(EffectivenessProbeRequest { entries: vec![EffectivenessProbeSpec {
        first_effective_delta: Some(0.5), ..spec("a", None) }], ..plain.clone() });
    assert!(accepted.ranking_valid);

    // 单删结论不能挂在 `Unsupported` 上。 / A removal verdict cannot sit under `Unsupported`.
    let unsupported = run(EffectivenessProbeRequest { entries: vec![EffectivenessProbeSpec {
        removal_effective: Some(false), status: Some("Unsupported".to_owned()),
        ..spec("a", None) }], ..plain.clone() });
    assert!(!unsupported.ranking_valid);
    let reachable = run(EffectivenessProbeRequest { entries: vec![EffectivenessProbeSpec {
        removal_effective: Some(false), status: Some("Reachable".to_owned()),
        ..spec("a", None) }], ..plain.clone() });
    assert!(reachable.ranking_valid);

    // 理由与状态无关：`Reachable` 带理由照样通过。 
    // The reason is tied to nothing: a `Reachable` entry with a reason still validates.
    let reasoned = run(EffectivenessProbeRequest { entries: vec![EffectivenessProbeSpec {
        status: Some("Reachable".to_owned()), unavailable_reason: Some("why not".to_owned()),
        ..spec("a", None) }], ..plain.clone() });
    assert!(reasoned.ranking_valid);

    // 身份重复/为空被拒。/ Duplicate and blank identities are refused.
    assert!(!run(EffectivenessProbeRequest { entries: vec![spec("same", None),
        spec("same", None)], ..plain.clone() }).ranking_valid);
    assert!(!run(EffectivenessProbeRequest { entries: vec![spec("  ", None)],
        ..plain.clone() }).ranking_valid);

    // 拒绝：条目上限、非有限 baseline、未知状态、缺请求。 
    // Refusals: the entry cap, a non-finite baseline, an unknown status and a missing request.
    assert!(verify(EffectivenessProbeRequest { entries: (0..9).map(|index|
        spec(&format!("c{index}"), None)).collect(), ..plain.clone() }).unwrap_err()
        .message.contains("at most eight entries"));
    assert!(verify(EffectivenessProbeRequest { baseline_objective: Some(f64::NAN),
        ..plain.clone() }).unwrap_err().message.contains("baseline objective must be finite"));
    assert!(verify(EffectivenessProbeRequest { entries: vec![EffectivenessProbeSpec {
        status: Some("Nearly".to_owned()), ..spec("a", None) }], ..plain.clone() })
        .unwrap_err().message.contains("unknown analysis status"));
    let mut missing_request = base_request();
    missing_request.model_kind = Some(ModelKind::EffectivenessProbeVerify);
    assert_eq!(engine.submit(missing_request).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        // 载荷故意把条目排成"升序改善"：文档说降序，校验却不看顺序。 
        // The payload orders entries by ascending improvement, which the doc forbids but the validator ignores.
        let payload = serde_json::to_string(&EffectivenessProbeRequest {
            baseline_objective: Some(0.0), schema_version: Some("probe".to_owned()),
            entries: vec![spec("low", Some(1.0)), spec("high", Some(9.0)),
                spec("zero", Some(0.0))] }).unwrap();
        std::fs::write(directory.join("effectiveness-probe.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn critical_report_entry_shows_the_unchecked_conclusion_and_the_textual_scan() {
    use ospf_rust_bridge::{CriticalReportRequest, CriticalReportVerdict};

    let engine = NativeEngine::default();
    let verify = |document: CriticalReportRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::CriticalReportVerify);
        request.critical_report = Some(document);
        engine.submit(request)
    };
    let run = |document: CriticalReportRequest| {
        let view = verify(document).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let verdict: CriticalReportVerdict =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        engine.release(&view.id).unwrap();
        verdict
    };
    let plain = CriticalReportRequest { schema_version: Some("probe".to_owned()),
        objective_id: Some("objective".to_owned()), sense: Some("minimize".to_owned()),
        status: Some("Reachable".to_owned()), proven_optimal: Some(true),
        optimal_value: Some(1.0), unavailable_reasons: Vec::new(),
        inject_broken_funnel: None };

    // 一个阶段都没跑，却带着"已证明"的总体结论 —— 校验照样通过。 
    // No stage ran at all, yet a proven overall conclusion validates.
    let base = run(plain.clone());
    assert!(base.valid);
    assert!(!base.minimal_blocking_set);
    assert_eq!(base.blocking_summary, "阻塞分析未执行 / Blocking analysis was not run");
    assert!(base.an_empty_report_with_a_proven_status_still_validates);
    assert!(base.a_proven_baseline_requires_a_value);
    assert!(base.the_forbidden_word_scan_reads_every_string);
    assert!(base.blank_schema_and_baseline_fields_are_refused);
    assert!(base.the_report_delegates_to_each_present_stage);
    assert!(base.a_missing_conflict_reports_no_minimal_blocking_set);
    assert!(base.a_missing_conflict_summarises_as_not_run);
    assert!(base.the_status_is_never_checked_against_the_stages);

    // 已证明却没有目标值 → 拒绝；不声明已证明 → 通过。 
    // Proven without a value is refused; without the claim it passes.
    assert!(!run(CriticalReportRequest { optimal_value: None, ..plain.clone() }).valid);
    assert!(run(CriticalReportRequest { proven_optimal: Some(false), optimal_value: None,
        ..plain.clone() }).valid);

    // 调用方自己的理由里带禁用词 → 拒绝（扫描的是整份报告的 `Debug` 文本）。 
    // The caller's own reason carrying a forbidden word is refused, because the scan reads the Debug text.
    for word in ["solver_row", "solver_column", "auxiliary_variable", "native_handle"] {
        let poisoned = run(CriticalReportRequest {
            unavailable_reasons: vec![format!("hidden {word}")], ..plain.clone() });
        assert!(!poisoned.valid, "{word}");
    }
    let allowed = run(CriticalReportRequest {
        unavailable_reasons: vec!["the solver was unavailable".to_owned()], ..plain.clone() });
    assert!(allowed.valid);

    // 空 schema / 空身份 → 拒绝。 / Blank schema and identities are refused.
    assert!(!run(CriticalReportRequest { schema_version: Some(" ".to_owned()),
        ..plain.clone() }).valid);
    assert!(!run(CriticalReportRequest { objective_id: Some(String::new()),
        ..plain.clone() }).valid);
    assert!(!run(CriticalReportRequest { sense: Some("  ".to_owned()), ..plain.clone() }).valid);

    // 一个坏阶段就能让整份报告失效。 / One broken stage invalidates the whole report.
    assert!(!run(CriticalReportRequest { inject_broken_funnel: Some(true),
        ..plain.clone() }).valid);

    // 四个总体状态配"一个阶段都没有"都通过。 / All four statuses pass with no stage at all.
    for name in ["Reachable", "Unreachable", "Unknown", "Unsupported"] {
        let verdict = run(CriticalReportRequest { status: Some(name.to_owned()),
            proven_optimal: Some(false), optimal_value: None, ..plain.clone() });
        assert!(verdict.valid, "{name}");
    }

    // 拒绝：理由条数上限、未知状态名、缺请求。 
    // Refusals: the reason cap, an unknown status name and a missing request.
    assert!(verify(CriticalReportRequest {
        unavailable_reasons: (0..9).map(|index| format!("r{index}")).collect(),
        ..plain.clone() }).unwrap_err().message.contains("at most eight reasons"));
    assert!(verify(CriticalReportRequest { status: Some("Nearly".to_owned()),
        ..plain.clone() }).unwrap_err().message.contains("unknown status name"));
    let mut missing_request = base_request();
    missing_request.model_kind = Some(ModelKind::CriticalReportVerify);
    assert_eq!(engine.submit(missing_request).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        // 载荷故意让"理由里出现禁用词"：这一刻文本扫描被看见。 
        // The payload puts a forbidden word in the reason, which is where the textual scan shows up.
        let payload = serde_json::to_string(&CriticalReportRequest {
            schema_version: Some("probe".to_owned()),
            objective_id: Some("objective".to_owned()), sense: Some("minimize".to_owned()),
            status: Some("Unreachable".to_owned()), proven_optimal: Some(false),
            optimal_value: None,
            unavailable_reasons: vec!["the native_handle leaked through".to_owned()],
            inject_broken_funnel: None }).unwrap();
        std::fs::write(directory.join("critical-report.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn group_summary_entry_shows_the_ordering_and_the_effectiveness_only_group() {
    use ospf_rust_bridge::{GroupActivitySpec, GroupEffectivenessSpec, GroupSummaryReport,
        GroupSummaryRequest};

    let engine = NativeEngine::default();
    let verify = |document: GroupSummaryRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::GroupSummaryVerify);
        request.group_summary = Some(document);
        engine.submit(request)
    };
    let run = |document: GroupSummaryRequest| {
        let view = verify(document).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: GroupSummaryReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        engine.release(&view.id).unwrap();
        report
    };
    let row = |id: &str, group: Option<&str>, activity: &str| GroupActivitySpec {
        id: id.to_owned(), group: group.map(str::to_owned), activity: activity.to_owned() };
    let entry = |id: &str, group: Option<&str>, improvement: Option<f64>|
        GroupEffectivenessSpec { id: id.to_owned(), group: group.map(str::to_owned),
            max_improvement: improvement };
    let plain = GroupSummaryRequest { activity: Some(vec![row("a1", Some("alpha"), "Active"),
        row("a2", Some("alpha"), "Active"), row("b1", Some("beta"), "Inactive"),
        row("u1", None, "Active")]), effectiveness: None };

    // 顺序：alpha（2 active）→ 未分组（1）→ beta（0）。 
    // The order: alpha with two actives, the ungrouped bucket, then beta.
    let base = run(plain.clone());
    assert_eq!(base.rows.len(), 3);
    assert_eq!(base.rows[0].group, "alpha");
    assert_eq!(base.rows[0].active_instances, 2);
    assert!(base.rows[1].ungrouped);
    assert_eq!(base.rows[1].active_instances, 1);
    assert_eq!(base.rows[2].group, "beta");
    assert_eq!(base.rows[2].active_instances, 0);
    assert!(base.rows.iter().all(|row| row.max_improvement.is_none()));
    assert!(base.the_order_is_blocking_then_active_then_name);
    assert!(base.the_ungrouped_bucket_wins_a_tie);
    assert!(base.an_effectiveness_without_an_improvement_creates_no_group);
    assert!(base.the_max_improvement_is_a_maximum_not_a_sum);
    assert!(base.only_active_counts_as_active);
    assert!(base.an_effectiveness_only_group_has_zero_counts);
    assert!(base.an_activity_only_group_has_no_improvement);
    assert!(base.no_stage_means_an_empty_summary);

    // 两个阶段都缺 → 空汇总。 / Both stages missing means an empty summary.
    let none = run(GroupSummaryRequest { activity: None, effectiveness: None });
    assert!(none.rows.is_empty());

    // 只由有效性建组：三项计数全 0，改善取最大值。 
    // Effectiveness only: zero counts and the improvement is a maximum.
    let only = run(GroupSummaryRequest { activity: None, effectiveness: Some(vec![
        entry("e1", Some("delta"), Some(3.0)), entry("e2", Some("delta"), Some(9.0)),
        entry("e3", Some("gamma"), None)]) });
    assert_eq!(only.rows.len(), 1);
    assert_eq!(only.rows[0].group, "delta");
    assert_eq!(only.rows[0].involved_instances, 0);
    assert_eq!(only.rows[0].max_improvement, Some(9.0));

    // `NearlyActive` 不进 active 计数。 / `NearlyActive` does not count as active.
    let nearly = run(GroupSummaryRequest { activity: Some(vec![row("a1", Some("eps"), "Active"),
        row("a2", Some("eps"), "NearlyActive")]), effectiveness: None });
    assert_eq!(nearly.rows[0].involved_instances, 2);
    assert_eq!(nearly.rows[0].active_instances, 1);

    // 并列时未分组先行。 / On a tie the ungrouped bucket leads.
    let tied = run(GroupSummaryRequest { activity: Some(vec![row("u1", None, "Inactive"),
        row("b1", Some("beta"), "Inactive")]), effectiveness: None });
    assert!(tied.rows[0].ungrouped);
    assert_eq!(tied.rows[1].group, "beta");

    // 拒绝：记录条数上限、未知活动性、非有限改善、缺请求。 
    // Refusals: the record cap, an unknown activity, a non-finite improvement and a missing request.
    assert!(verify(GroupSummaryRequest { activity: Some((0..9).map(|index|
        row(&format!("c{index}"), None, "Active")).collect()), effectiveness: None })
        .unwrap_err().message.contains("at most eight records"));
    assert!(verify(GroupSummaryRequest { activity: Some(vec![GroupActivitySpec {
        id: "a".to_owned(), group: None, activity: "Nearly".to_owned() }]),
        effectiveness: None }).unwrap_err().message.contains("unknown activity name"));
    assert!(verify(GroupSummaryRequest { activity: None, effectiveness: Some(vec![
        entry("e", None, Some(f64::INFINITY))]) }).unwrap_err()
        .message.contains("non-finite improvements"));
    let mut missing_request = base_request();
    missing_request.model_kind = Some(ModelKind::GroupSummaryVerify);
    assert_eq!(engine.submit(missing_request).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        // 载荷故意让"只有有效性、没有活动性"：这一刻组的三项计数全为 0。 
        // The payload has effectiveness only, where all three counts stay zero.
        let payload = serde_json::to_string(&GroupSummaryRequest { activity: None,
            effectiveness: Some(vec![entry("e1", Some("delta"), Some(3.0)),
                entry("e2", Some("delta"), Some(9.0))]) }).unwrap();
        std::fs::write(directory.join("group-summary.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn band_probe_entry_shows_the_inclusive_edges_and_the_recounted_summary() {
    use ospf_rust_bridge::{BandProbeReport, BandProbeRequest, BandProbeSpec};

    let engine = NativeEngine::default();
    let verify = |document: BandProbeRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::BandProbeVerify);
        request.band_probe = Some(document);
        engine.submit(request)
    };
    let run = |document: BandProbeRequest| {
        let view = verify(document).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: BandProbeReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        engine.release(&view.id).unwrap();
        report
    };
    let probe = |slack: f64, scale: f64| BandProbeSpec { slack, scale };
    let plain = BandProbeRequest { tolerance: Some(1e-7), nearly_active_tolerance: Some(1e-5),
        relative_tolerance: Some(0.0),
        probes: vec![probe(0.0, 1.0), probe(-1e-7, 1.0), probe(2e-6, 1.0),
            probe(2e-5, 1.0), probe(-2e-6, 1.0)],
        break_summary: None };

    // 五个探针的分类：Active / Active / NearlyActive / Inactive / Violated。 
    // Five classifications: active, active, nearly, inactive, violated.
    let base = run(plain.clone());
    assert_eq!(base.rows.iter().map(|row| row.status.as_str()).collect::<Vec<_>>(),
        vec!["Active", "Active", "NearlyActive", "Inactive", "Violated"]);
    assert_eq!(base.rows[0].active_tolerance, 1e-7);
    assert_eq!(base.rows[0].nearly_tolerance, 1e-5);
    assert!(base.config_valid);
    assert!(base.report_valid);
    assert!(base.a_non_finite_slack_or_scale_is_unknown);
    assert!(base.minus_the_tolerance_is_active_not_violated);
    assert!(base.beyond_the_active_band_is_violated);
    assert!(base.the_scale_widens_the_tolerance);
    assert!(base.the_scale_never_shrinks_the_tolerance);
    assert!(base.the_nearly_band_never_sits_below_active);
    assert!(base.the_summary_must_match_the_records);
    assert!(base.the_group_summary_must_match_the_records);

    // 非有限值 → Unknown。 / Non-finite values read as unknown.
    let nan = run(BandProbeRequest { probes: vec![probe(f64::NAN, 1.0),
        probe(1.0, f64::INFINITY)], ..plain.clone() });
    assert_eq!(nan.rows[0].status, "Unknown");
    assert_eq!(nan.rows[1].status, "Unknown");

    // 尺度放大容差：slack 5 在尺度 100 下算 Active，在尺度 0.5 下算 Inactive。 
    // The scale widens: five is active at scale one hundred and inactive at a half.
    let scaled = run(BandProbeRequest { relative_tolerance: Some(0.1),
        probes: vec![probe(5.0, 100.0), probe(5.0, 0.5)], ..plain.clone() });
    assert_eq!(scaled.rows[0].status, "Active");
    assert_eq!(scaled.rows[0].active_tolerance, 10.0);
    assert_eq!(scaled.rows[1].status, "Inactive");
    assert_eq!(scaled.rows[1].active_tolerance, 0.1);

    // 非法容差顺序：配置不合法，但分类仍按 `.max(active)` 工作。 
    // An illegal tolerance order fails validation while classification still uses `.max(active)`.
    let illegal = run(BandProbeRequest { tolerance: Some(1e-5),
        nearly_active_tolerance: Some(1e-7), probes: vec![probe(5e-6, 1.0)],
        ..plain.clone() });
    assert!(!illegal.config_valid);
    assert_eq!(illegal.rows[0].status, "Active");

    // 计数被改坏 → 报告校验失败。 / A broken count fails the report validation.
    let broken = run(BandProbeRequest { break_summary: Some(true), ..plain.clone() });
    assert!(!broken.report_valid);

    // 拒绝：探针条数上限、缺请求。 / Refusals: the probe cap and a missing request.
    assert!(verify(BandProbeRequest { probes: (0..9).map(|index|
        probe(index as f64, 1.0)).collect(), ..plain.clone() }).unwrap_err()
        .message.contains("at most eight probes"));
    let mut missing_request = base_request();
    missing_request.model_kind = Some(ModelKind::BandProbeVerify);
    assert_eq!(engine.submit(missing_request).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        // 载荷故意让"恰好 -active"：这一刻边界包含性被看见。 
        // The payload probes exactly `-active`, where the inclusive edge shows up.
        let payload = serde_json::to_string(&BandProbeRequest { tolerance: Some(1e-7),
            nearly_active_tolerance: Some(1e-5), relative_tolerance: Some(0.0),
            probes: vec![probe(-1e-7, 1.0), probe(-1e-7 - 1e-12, 1.0), probe(0.0, 1.0)],
            break_summary: None }).unwrap();
        std::fs::write(directory.join("band-probe.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn evidence_probe_entry_shows_the_source_only_authority_gate() {
    use ospf_rust_bridge::{EvidenceProbeReport, EvidenceProbeRequest, InfeasibilityEvidenceSpec};

    let engine = NativeEngine::default();
    let verify = |document: EvidenceProbeRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::EvidenceProbeVerify);
        request.evidence_probe = Some(document);
        engine.submit(request)
    };
    let run = |document: EvidenceProbeRequest| {
        let view = verify(document).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: EvidenceProbeReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        engine.release(&view.id).unwrap();
        report
    };
    let spec = |source: &str, reliability: Option<&str>, completeness: Option<&str>|
        InfeasibilityEvidenceSpec { source: source.to_owned(),
            reliability: reliability.map(str::to_owned),
            completeness: completeness.map(str::to_owned), minimality: None,
            constraint_ids: Vec::new(), members: Vec::new(), unavailable_reason: None };
    let ask = |document: InfeasibilityEvidenceSpec| run(EvidenceProbeRequest { evidence: document });

    // Farkas 无条件权威，默认最小性是 NotChecked。 
    // Farkas is unconditionally authoritative and minimality defaults to not checked.
    let farkas = ask(InfeasibilityEvidenceSpec { constraint_ids: vec!["c1".to_owned()],
        members: vec!["Constraint:c1".to_owned(), "LowerBound:x".to_owned()],
        ..spec("Farkas", Some("Exact"), Some("Complete")) });
    assert!(farkas.authoritative);
    assert_eq!(farkas.minimality, "NotChecked");
    assert_eq!(farkas.constraint_id_count, 1);
    assert_eq!(farkas.member_count, 2);
    assert!(farkas.native_and_farkas_are_authoritative_whatever_their_quality);
    assert!(farkas.rebuild_needs_both_exact_and_complete);
    assert!(farkas.the_filters_are_never_authoritative);
    assert!(farkas.the_unavailable_source_is_never_authoritative);
    assert!(farkas.minimality_defaults_to_not_checked);
    assert!(farkas.the_two_member_sets_never_derive_from_each_other);
    assert!(farkas.the_unavailable_reason_is_tied_to_no_state);
    assert!(farkas.the_wire_names_are_screaming_snake_case);

    // 质量再差，原生 IIS 也权威。 / Even at the worst quality a native IIS stays authoritative.
    assert!(ask(spec("NativeIis", Some("Heuristic"), Some("Unavailable"))).authoritative);
    assert!(ask(spec("NativeIis", None, None)).authoritative);

    // CP 重建必须同时 EXACT 与 COMPLETE。 / A rebuild needs both exact and complete.
    assert!(ask(spec("RebuildVerification", Some("Exact"), Some("Complete"))).authoritative);
    assert!(!ask(spec("RebuildVerification", Some("Reliable"), Some("Complete"))).authoritative);
    assert!(!ask(spec("RebuildVerification", Some("Exact"), Some("Partial"))).authoritative);
    assert!(!ask(spec("RebuildVerification", None, None)).authoritative);

    // 两种过滤与 Unavailable 永不权威。 / The filters and the unavailable source never are.
    for source in ["DeletionFilter", "ElasticFilter", "Unavailable"] {
        assert!(!ask(spec(source, Some("Exact"), Some("Complete"))).authoritative, "{source}");
    }

    // 两套集合互不推导。 / The two identity sets never derive from each other.
    let only_ids = ask(InfeasibilityEvidenceSpec { constraint_ids: vec!["c1".to_owned()],
        ..spec("NativeIis", Some("Exact"), Some("Complete")) });
    assert_eq!((only_ids.constraint_id_count, only_ids.member_count), (1, 0));
    let only_members = ask(InfeasibilityEvidenceSpec {
        members: vec!["Constraint:c1".to_owned()],
        ..spec("NativeIis", Some("Exact"), Some("Complete")) });
    assert_eq!((only_members.constraint_id_count, only_members.member_count), (0, 1));

    // 显式最小性被回读。 / An explicit minimality is echoed.
    assert_eq!(ask(InfeasibilityEvidenceSpec { minimality: Some("Irreducible".to_owned()),
        ..spec("NativeIis", Some("Exact"), Some("Complete")) }).minimality, "Irreducible");

    // 拒绝：未知名字、成员格式、集合上限、缺请求。 
    // Refusals: unknown names, the member shape, the set cap and a missing request.
    assert!(verify(EvidenceProbeRequest { evidence: spec("Nearly", None, None) }).unwrap_err()
        .message.contains("unknown evidence source"));
    assert!(verify(EvidenceProbeRequest { evidence: InfeasibilityEvidenceSpec {
        members: vec!["Constraint".to_owned()], ..spec("Farkas", None, None) } }).unwrap_err()
        .message.contains("Kind:id"));
    assert!(verify(EvidenceProbeRequest { evidence: InfeasibilityEvidenceSpec {
        constraint_ids: (0..17).map(|index| format!("c{index}")).collect(),
        ..spec("Farkas", None, None) } }).unwrap_err().message.contains("at most sixteen"));
    let mut missing_request = base_request();
    missing_request.model_kind = Some(ModelKind::EvidenceProbeVerify);
    assert_eq!(engine.submit(missing_request).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        // 载荷故意让"重建只有 RELIABLE + COMPLETE"：这一刻闸门的两条要求被看见。 
        // The payload is a rebuild with `RELIABLE` plus `COMPLETE`, where both requirements of the gate show up.
        let payload = serde_json::to_string(&EvidenceProbeRequest {
            evidence: InfeasibilityEvidenceSpec { constraint_ids: vec!["c1".to_owned()],
                members: vec!["Constraint:c1".to_owned(), "Assumption:a".to_owned()],
                unavailable_reason: Some("deletion checking was skipped".to_owned()),
                ..spec("RebuildVerification", Some("Reliable"), Some("Complete")) } }).unwrap();
        std::fs::write(directory.join("evidence-probe.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn proof_probe_entry_shows_the_proof_contract_and_the_pricing_gate() {
    use ospf_rust_bridge::{ProofProbeReport, ProofProbeRequest, ProofSpec};

    let engine = NativeEngine::default();
    let verify = |document: ProofProbeRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::ProofProbeVerify);
        request.proof_probe = Some(document);
        engine.submit(request)
    };
    let run = |document: ProofProbeRequest| {
        let view = verify(document).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: ProofProbeReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        engine.release(&view.id).unwrap();
        report
    };
    let plain = ProofSpec { kind: None, status: None, reliability: None, completeness: None,
        problem: None, presence: None, termination: None, has_solution: None, dual: None };
    let spec = |kind: Option<&str>, status: Option<&str>, reliability: Option<&str>,
        completeness: Option<&str>, problem: Option<&str>, presence: Option<&str>,
        termination: Option<&str>, has_solution: Option<bool>, dual: Option<&str>| ProofSpec {
        kind: kind.map(str::to_owned), status: status.map(str::to_owned),
        reliability: reliability.map(str::to_owned),
        completeness: completeness.map(str::to_owned), problem: problem.map(str::to_owned),
        presence: presence.map(str::to_owned), termination: termination.map(str::to_owned),
        has_solution, dual: dual.map(str::to_owned) };
    let ask = |document: ProofSpec| run(ProofProbeRequest { report: document });

    // 默认报告合法，闸门放行。 / The default report validates and the gate passes.
    let base = ask(plain.clone());
    assert!(base.report_valid, "{}", base.report_error);
    assert!(base.gate_ok, "{}", base.gate_error);
    assert!(base.a_fresh_proof_is_verified_exact_and_complete);
    assert!(base.a_verified_proof_must_be_complete);
    assert!(base.a_verified_proof_must_be_reliable);
    assert!(base.a_verified_proof_needs_a_completed_termination);
    assert!(base.a_none_status_proof_is_refused);
    assert!(base.an_optimal_presence_needs_a_verified_optimality_proof);
    assert!(base.an_infeasibility_proof_needs_an_infeasible_status);
    assert!(base.the_exact_pricing_gate_needs_a_non_empty_dual);

    // 已核验但不完整 / 不可靠 / 未以 Completed 收尾 → 都拒。 
    // Verified but incomplete, unreliable or not completed: all refused.
    let partial = ask(spec(None, None, None, Some("Partial"), None, None, None, None, None));
    assert!(!partial.report_valid);
    assert!(partial.report_error.contains("complete"), "{}", partial.report_error);
    assert!(!ask(spec(None, None, Some("Heuristic"), None, None, None, None, None, None))
        .report_valid);
    assert!(!ask(spec(None, None, None, None, None, None, Some("TimeLimit"), None, None))
        .report_valid);
    assert!(ask(spec(None, None, Some("Reliable"), None, None, None, None, None, None))
        .report_valid);

    // `Claimed` 不受收尾原因约束。 / A claimed proof is not bound by the termination.
    assert!(ask(spec(None, Some("Claimed"), None, None, None, Some("Incumbent"),
        Some("TimeLimit"), None, None)).report_valid);

    // `None` 状态被拒。 / A none-status proof is refused.
    assert!(!ask(spec(None, Some("None"), None, None, None, Some("Incumbent"), None, None, None))
        .report_valid);

    // `Optimal` 需要已核验的最优性证明。 / An optimal presence needs a verified optimality proof.
    assert!(!ask(spec(None, Some("Claimed"), None, None, None, None, None, None, None))
        .report_valid);
    assert!(!ask(spec(Some("Infeasibility"), None, None, None, None, None, None, None, None))
        .report_valid);

    // 不可行证明要求不可行状态。 / An infeasibility proof demands an infeasible status.
    let infeasible = ask(spec(Some("Infeasibility"), None, None, None, Some("Infeasible"),
        Some("None"), None, Some(false), None));
    assert!(infeasible.report_valid, "{}", infeasible.report_error);

    // 闸门要非空对偶。 / The gate needs a non-empty dual.
    let missing_dual = ask(spec(None, None, None, None, None, None, None, None, Some("none")));
    assert!(missing_dual.report_valid);
    assert!(!missing_dual.gate_ok);
    assert!(missing_dual.gate_error.contains("dual"), "{}", missing_dual.gate_error);
    let empty_dual = ask(spec(None, None, None, None, None, None, None, None, Some("empty")));
    assert!(!empty_dual.gate_ok);
    assert!(empty_dual.gate_error.contains("empty dual"), "{}", empty_dual.gate_error);

    // 拒绝：未知名字、缺请求。 / Refusals: unknown names and a missing request.
    assert!(verify(ProofProbeRequest { report: spec(Some("Nearly"), None, None, None, None,
        None, None, None, None) }).unwrap_err().message.contains("unknown proof kind"));
    assert!(verify(ProofProbeRequest { report: spec(None, None, None, None, None, None, None,
        None, Some("two")) }).unwrap_err().message.contains("unknown dual shape"));
    let mut missing_request = base_request();
    missing_request.model_kind = Some(ModelKind::ProofProbeVerify);
    assert_eq!(engine.submit(missing_request).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        // 载荷故意让"已核验、可靠、完整，但以 TimeLimit 收尾"：这一刻收尾条件被看见。 
        // The payload is verified, reliable and complete yet finished with a time limit, where that condition shows up.
        let payload = serde_json::to_string(&ProofProbeRequest {
            report: spec(None, None, None, None, None, None, Some("TimeLimit"), None, None) })
            .unwrap();
        std::fs::write(directory.join("proof-probe.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn gap_probe_entry_shows_the_computed_gap_and_the_trace_invariants() {
    use ospf_rust_bridge::{GapProbeReport, GapProbeRequest, SnapshotSpec};

    let engine = NativeEngine::default();
    let verify = |document: GapProbeRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::GapProbeVerify);
        request.gap_probe = Some(document);
        engine.submit(request)
    };
    let run = |document: GapProbeRequest| {
        let view = verify(document).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: GapProbeReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        engine.release(&view.id).unwrap();
        report
    };
    let plain = GapProbeRequest { objective_value: Some(2.0), best_bound_value: Some(0.0),
        absolute_gap: Some(2.0), relative_gap: Some(1.0), snapshots: Vec::new() };
    let snapshot = |iteration: u32| SnapshotSpec { iteration, stage: "master".to_owned(),
        objective_value: None, best_bound: None, relative_gap: None, items_added: Some(1),
        items_total: Some(1) };
    let ask = |document: GapProbeRequest| run(document);

    // 默认报告合法。 / The default report validates.
    let base = ask(plain.clone());
    assert!(base.report_valid, "{}", base.report_error);
    assert!(base.the_absolute_gap_must_match_the_incumbent_and_bound);
    assert!(base.the_relative_gap_floors_its_denominator_at_one);
    assert!(base.a_negative_gap_is_refused);
    assert!(base.a_non_finite_gap_is_refused);
    assert!(base.the_iteration_numbers_are_one_based_and_contiguous);
    assert!(base.a_blank_iteration_stage_is_refused);
    assert!(base.the_cumulative_item_count_never_decreases);
    assert!(base.the_snapshot_gap_must_match_its_own_numbers);

    // 绝对 gap 算错被拒；缺任一侧时 gap 整体不许出现。 
    // A wrong absolute gap is refused, and a gap is forbidden when either side is missing.
    let wrong = ask(GapProbeRequest { absolute_gap: Some(3.0), ..plain.clone() });
    assert!(!wrong.report_valid);
    assert!(wrong.report_error.contains("absolute gap"), "{}", wrong.report_error);
    let orphan = ask(GapProbeRequest { best_bound_value: None, ..plain.clone() });
    assert!(!orphan.report_valid);
    assert!(orphan.report_error.contains("requires both"), "{}", orphan.report_error);

    // 分母地板：目标 0.5、下界 0 → 相对 0.5；配下界 −3 → 相对 3.5（大于 1）。 
    // The floor: 0.5 against zero gives 0.5, while against minus three it gives 3.5.
    assert!(ask(GapProbeRequest { objective_value: Some(0.5), absolute_gap: Some(0.5),
        relative_gap: Some(0.5), ..plain.clone() }).report_valid);
    let unfloored = ask(GapProbeRequest { objective_value: Some(0.5), absolute_gap: Some(0.5),
        relative_gap: Some(1.0), ..plain.clone() });
    assert!(!unfloored.report_valid);
    assert!(unfloored.report_error.contains("relative gap"), "{}", unfloored.report_error);
    assert!(ask(GapProbeRequest { objective_value: Some(0.5), best_bound_value: Some(-3.0),
        absolute_gap: Some(3.5), relative_gap: Some(3.5), ..plain.clone() }).report_valid);

    // 负与非有限 gap 被拒。 / Negative and non-finite gaps are refused.
    assert!(!ask(GapProbeRequest { absolute_gap: Some(-1.0), ..plain.clone() }).report_valid);
    assert!(!ask(GapProbeRequest { relative_gap: Some(f64::NAN), ..plain.clone() })
        .report_valid);

    // 逐轮编号、阶段名、累计条目、每轮 gap。 
    // Iteration numbering, stage, cumulative count and per-iteration gap.
    assert!(ask(GapProbeRequest { snapshots: vec![snapshot(1), snapshot(2)],
        ..plain.clone() }).report_valid);
    let zero = ask(GapProbeRequest { snapshots: vec![snapshot(0)], ..plain.clone() });
    assert!(!zero.report_valid);
    assert!(zero.report_error.contains("one-based"), "{}", zero.report_error);
    let blank = ask(GapProbeRequest { snapshots: vec![SnapshotSpec {
        stage: "  ".to_owned(), ..snapshot(1) }], ..plain.clone() });
    assert!(!blank.report_valid);
    assert!(blank.report_error.contains("blank"), "{}", blank.report_error);
    let falling = ask(GapProbeRequest { snapshots: vec![
        SnapshotSpec { items_added: Some(2), items_total: Some(5), ..snapshot(1) },
        SnapshotSpec { items_added: Some(1), items_total: Some(3), ..snapshot(2) },
    ], ..plain.clone() });
    assert!(!falling.report_valid);
    let lying = ask(GapProbeRequest { snapshots: vec![SnapshotSpec {
        objective_value: Some(4.0), best_bound: Some(2.0), relative_gap: Some(0.25),
        ..snapshot(1) }], ..plain.clone() });
    assert!(!lying.report_valid);
    assert!(lying.report_error.contains("iteration trace gap"), "{}", lying.report_error);

    // 拒绝：快照条数上限、缺请求。 / Refusals: the snapshot cap and a missing request.
    assert!(verify(GapProbeRequest { snapshots: (0..5).map(snapshot).collect(),
        ..plain.clone() }).unwrap_err().message.contains("at most four snapshots"));
    let mut missing_request = base_request();
    missing_request.model_kind = Some(ModelKind::GapProbeVerify);
    assert_eq!(engine.submit(missing_request).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        // 载荷故意让"目标值小于 1、分母取地板"：这一刻地板效应被看见。 
        // The payload puts the objective below one, where the floored denominator shows up.
        let payload = serde_json::to_string(&GapProbeRequest { objective_value: Some(0.5),
            best_bound_value: Some(0.0), absolute_gap: Some(0.5), relative_gap: Some(0.5),
            snapshots: vec![SnapshotSpec { objective_value: Some(0.5), best_bound: Some(0.0),
                relative_gap: Some(0.5), ..snapshot(1) }] }).unwrap();
        std::fs::write(directory.join("gap-probe.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn diagnostics_probe_entry_shows_the_unguarded_diagnostic_face() {
    use ospf_rust_bridge::{DiagnosticsProbeReport, DiagnosticsProbeRequest, EvaluationSpec,
        MessageSpec};

    let engine = NativeEngine::default();
    let verify = |document: DiagnosticsProbeRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::DiagnosticsProbeVerify);
        request.diagnostics_probe = Some(document);
        engine.submit(request)
    };
    let run = |document: DiagnosticsProbeRequest| {
        let view = verify(document).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: DiagnosticsProbeReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        engine.release(&view.id).unwrap();
        report
    };
    let plain = DiagnosticsProbeRequest { presence: None, has_solution: None, typed_value: None,
        evaluations: Vec::new(), warnings: Vec::new(), issues: Vec::new() };
    let evaluation = |satisfied: bool, violation: f64| EvaluationSpec { satisfied, violation };
    let message = |code: &str, body: &str| MessageSpec { code: code.to_owned(),
        message: body.to_owned() };
    let ask = |document: DiagnosticsProbeRequest| run(document);

    // 默认报告合法，三个谓词都为真。 / The default report validates and all three predicates hold.
    let base = ask(plain.clone());
    assert!(base.report_valid, "{}", base.report_error);
    assert!(base.has_incumbent);
    assert!(base.is_optimal);
    assert!(base.incumbent_present);
    assert!(base.has_incumbent_reads_only_the_presence);
    assert!(base.is_optimal_reads_only_the_presence);
    assert!(base.the_predicates_never_validate_the_report);
    assert!(base.the_incumbent_reads_the_typed_value);
    assert!(base.the_diagnostics_are_never_validated);
    assert!(base.satisfied_and_violation_are_never_cross_checked);
    assert!(base.the_bound_evaluation_flags_are_never_cross_checked);
    assert!(base.blank_warnings_and_issues_are_accepted);

    // 谓词只读 presence。 / The predicates read only the presence.
    let incumbent = ask(DiagnosticsProbeRequest { presence: Some("Incumbent".to_owned()),
        ..plain.clone() });
    assert!(incumbent.has_incumbent && !incumbent.is_optimal);
    let none = ask(DiagnosticsProbeRequest { presence: Some("None".to_owned()), ..plain.clone() });
    assert!(!none.has_incumbent && !none.is_optimal);
    assert!(!none.report_valid);

    // 谓词不校验：非法报告上 `has_incumbent()` 仍为真，而 `incumbent()` 为 None。 
    // The predicates never validate: an illegal report still claims an incumbent while `incumbent()` is none.
    let broken = ask(DiagnosticsProbeRequest { presence: Some("Incumbent".to_owned()),
        has_solution: Some(false), ..plain.clone() });
    assert!(!broken.report_valid);
    assert!(broken.has_incumbent);
    assert!(!broken.incumbent_present);
    // 反方向：`None` 的 presence 配 incumbent → 谓词为假，`incumbent()` 却有值。 
    // The other direction: a `None` presence with an incumbent gives a false predicate but a payload.
    let orphan = ask(DiagnosticsProbeRequest { presence: Some("None".to_owned()),
        ..plain.clone() });
    assert!(!orphan.report_valid);
    assert!(!orphan.has_incumbent);
    assert!(orphan.incumbent_present);

    // `incumbent()` 读的是 `solution.value`。 / `incumbent()` reads `solution.value`.
    let untyped = ask(DiagnosticsProbeRequest { typed_value: Some(false), ..plain.clone() });
    assert!(untyped.report_valid && untyped.has_incumbent && !untyped.incumbent_present);

    // 荒谬的诊断内容照样通过校验。 / Absurd diagnostics still validate.
    let absurd = ask(DiagnosticsProbeRequest { evaluations: vec![evaluation(true, 1e9)],
        warnings: vec![message("", "")], issues: vec![message("  ", "   ")],
        ..plain.clone() });
    assert!(absurd.report_valid, "{}", absurd.report_error);
    let contradictory = ask(DiagnosticsProbeRequest {
        evaluations: vec![evaluation(true, 1e8), evaluation(false, 0.0)], ..plain.clone() });
    assert!(contradictory.report_valid);
    let blank = ask(DiagnosticsProbeRequest { warnings: vec![message("", "")],
        issues: vec![message("", "")], ..plain.clone() });
    assert!(blank.report_valid);

    // 拒绝：列表上限、未知 presence、缺请求。 / Refusals: the list cap, an unknown presence, a missing request.
    assert!(verify(DiagnosticsProbeRequest {
        evaluations: (0..5).map(|index| evaluation(index % 2 == 0, 0.0)).collect(),
        ..plain.clone() }).unwrap_err().message.contains("at most four entries"));
    assert!(verify(DiagnosticsProbeRequest { presence: Some("Nearly".to_owned()),
        ..plain.clone() }).unwrap_err().message.contains("unknown solution presence"));
    let mut missing_request = base_request();
    missing_request.model_kind = Some(ModelKind::DiagnosticsProbeVerify);
    assert_eq!(engine.submit(missing_request).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        // 载荷故意让"标记满足却违反一亿"：这一刻诊断面不受约束被看见。 
        // The payload flags satisfaction while violating a hundred million, where the unguarded face shows up.
        let payload = serde_json::to_string(&DiagnosticsProbeRequest {
            evaluations: vec![evaluation(true, 1e8), evaluation(false, 0.0)],
            warnings: vec![message("", "")], issues: vec![message("kept", "kept")],
            ..plain.clone() }).unwrap();
        std::fs::write(directory.join("diagnostics-probe.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn fingerprint_probe_entry_shows_the_structural_match_and_the_unknown_default() {
    use ospf_rust_bridge::{AuditFingerprintSpec, FingerprintProbeReport, FingerprintProbeRequest};

    let engine = NativeEngine::default();
    let verify = |document: FingerprintProbeRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::FingerprintProbeVerify);
        request.fingerprint_probe = Some(document);
        engine.submit(request)
    };
    let run = |document: FingerprintProbeRequest| {
        let view = verify(document).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: FingerprintProbeReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        engine.release(&view.id).unwrap();
        report
    };
    let plain = FingerprintProbeRequest { model: None, expected: None, solver_id: None,
        backend_name: None, deterministic: None, random_seed: None, schema_version: None,
        infeasible: None };
    let fingerprint = || AuditFingerprintSpec { schema_version: None, algorithm: None,
        value: None };
    let ask = |document: FingerprintProbeRequest| run(document);

    // 默认报告合法且闸门放行。 / The default report validates and the gate passes.
    let base = ask(plain.clone());
    assert!(base.report_valid, "{}", base.report_error);
    assert!(base.gate_ok, "{}", base.gate_error);
    assert!(base.the_provenance_default_names_an_unknown_solver);
    assert!(base.the_fingerprints_default_to_nothing);
    assert!(base.the_fingerprint_equality_is_structural);
    assert!(base.the_comparison_reads_every_field);
    assert!(base.the_fingerprint_schema_version_is_never_checked);
    assert!(base.a_missing_model_fingerprint_fails_the_gate);
    assert!(base.the_provenance_is_never_validated);
    assert!(base.the_report_schema_version_is_never_compared);

    // 指纹一致 → 放行；没带指纹 → 拒。 
    // A matching fingerprint passes while a missing one is refused.
    let matching = ask(FingerprintProbeRequest { model: Some(fingerprint()),
        expected: Some(fingerprint()), infeasible: Some(true), ..plain.clone() });
    assert!(matching.gate_ok, "{}", matching.gate_error);
    let bare = ask(FingerprintProbeRequest { model: None, expected: Some(fingerprint()),
        infeasible: Some(true), ..plain.clone() });
    assert!(bare.report_valid);
    assert!(!bare.gate_ok);
    assert!(bare.gate_error.contains("fingerprint"), "{}", bare.gate_error);

    // 同一个 value 配不同算法 → 不匹配。 / The same value with a different algorithm does not match.
    let different = ask(FingerprintProbeRequest { model: Some(fingerprint()),
        expected: Some(AuditFingerprintSpec { algorithm: Some("blake3".to_owned()),
            ..fingerprint() }), infeasible: Some(true), ..plain.clone() });
    assert!(!different.gate_ok);

    // provenance 与 schema 版本都不被校验。 / The provenance and both schema versions are never validated.
    let absurd = ask(FingerprintProbeRequest { solver_id: Some(String::new()),
        backend_name: Some("   ".to_owned()), deterministic: Some(true),
        random_seed: Some(7), schema_version: Some(String::new()), ..plain.clone() });
    assert!(absurd.report_valid, "{}", absurd.report_error);
    let odd = ask(FingerprintProbeRequest { model: Some(AuditFingerprintSpec {
        schema_version: Some("  ".to_owned()), algorithm: Some(String::new()),
        value: Some(String::new()) }), ..plain.clone() });
    assert!(odd.report_valid);
    let wrong_version = ask(FingerprintProbeRequest {
        schema_version: Some("9999-not-the-constant".to_owned()), ..plain.clone() });
    assert!(wrong_version.report_valid);

    // 缺请求是 reject。 / A missing request is refused.
    let mut missing_request = base_request();
    missing_request.model_kind = Some(ModelKind::FingerprintProbeVerify);
    assert_eq!(engine.submit(missing_request).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        // 载荷故意让"同值不同算法"：这一刻结构相等被看见。 
        // The payload puts the same value with a different algorithm, where structural equality shows up.
        let payload = serde_json::to_string(&FingerprintProbeRequest {
            model: Some(AuditFingerprintSpec { schema_version: Some("1".to_owned()),
                algorithm: Some("sha256".to_owned()), value: Some("abc".to_owned()) }),
            expected: Some(AuditFingerprintSpec { schema_version: Some("1".to_owned()),
                algorithm: Some("blake3".to_owned()), value: Some("abc".to_owned()) }),
            solver_id: None, backend_name: None, deterministic: None, random_seed: None,
            schema_version: None, infeasible: Some(true) }).unwrap();
        std::fs::write(directory.join("fingerprint-probe.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn blocking_set_entry_shows_the_single_field_minimality_verdict() {
    use ospf_rust_bridge::{BlockingSetReport, BlockingSetRequest, VerificationSpec};

    let engine = NativeEngine::default();
    let verify = |document: BlockingSetRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::BlockingSetVerify);
        request.blocking_set = Some(document);
        engine.submit(request)
    };
    let run = |document: BlockingSetRequest| {
        let view = verify(document).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: BlockingSetReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        engine.release(&view.id).unwrap();
        report
    };
    let plain = BlockingSetRequest { minimality: Some("Irreducible".to_owned()), status: None,
        validity: None, members: vec!["c1".to_owned()],
        verifications: vec![VerificationSpec { id: "c1".to_owned(), status: None,
            removed: Some(false), retained: Some(true), target_fixed: Some(true) }],
        background_blocking: None, extraction_tier: None };
    let ask = |document: BlockingSetRequest| run(document);

    // 默认：Irreducible → 已证明最小。 / The default: irreducible, hence proven.
    let base = ask(plain.clone());
    assert!(base.verified);
    assert_eq!(base.minimality, "Irreducible");
    assert_eq!(base.extraction_tier, "RepeatedSolving");
    assert!(!base.native_tier);
    assert!(base.verified_reads_only_the_minimality);
    assert!(base.the_minimality_three_way_split_is_shared);
    assert!(base.verified_ignores_the_status);
    assert!(base.verified_ignores_the_validity);
    assert!(base.verified_ignores_the_verifications);
    assert!(base.verified_ignores_the_members);
    assert!(base.verified_ignores_the_background_and_the_tier);
    assert!(base.only_the_native_tier_is_native);

    // Partial / NotChecked / 缺省 → 都不"已证明"。 / Partial, not checked and absent are all unproven.
    for minimality in [Some("Partial"), Some("NotChecked"), None] {
        let unproven = ask(BlockingSetRequest { minimality: minimality.map(str::to_owned),
            ..plain.clone() });
        assert!(!unproven.verified, "{minimality:?}");
    }

    // 空成员 + 空记录 + 背景阻塞 + Unavailable 层级，仍然"已证明最小"。 
    // Empty members and records with background blocking and an unavailable tier is still proven.
    let absurd = ask(BlockingSetRequest { members: Vec::new(), verifications: Vec::new(),
        background_blocking: Some(true), extraction_tier: Some("Unavailable".to_owned()),
        status: Some("Reachable".to_owned()), validity: Some("Unknown".to_owned()),
        ..plain.clone() });
    assert!(absurd.verified);
    assert_eq!(absurd.extraction_tier, "Unavailable");
    assert!(!absurd.native_tier);

    // 只有原生层级算原生。 / Only the native tier is native.
    let native = ask(BlockingSetRequest { extraction_tier: Some("NativeUnsatCore".to_owned()),
        ..plain.clone() });
    assert!(native.native_tier);

    // 拒绝：未知名字、列表上限、缺请求。 / Refusals: unknown names, the list cap, a missing request.
    assert!(verify(BlockingSetRequest { minimality: Some("Nearly".to_owned()), ..plain.clone() })
        .unwrap_err().message.contains("unknown minimality"));
    assert!(verify(BlockingSetRequest { extraction_tier: Some("Nearly".to_owned()),
        ..plain.clone() }).unwrap_err().message.contains("unknown extraction tier"));
    assert!(verify(BlockingSetRequest { members: (0..9).map(|index| format!("c{index}"))
        .collect(), ..plain.clone() }).unwrap_err().message.contains("at most eight entries"));
    let mut missing_request = base_request();
    missing_request.model_kind = Some(ModelKind::BlockingSetVerify);
    assert_eq!(engine.submit(missing_request).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        // 载荷故意让"没有成员却已证明最小"：这一刻单字段判据被看见。 
        // The payload claims proven minimality with no member at all, where the single-field verdict shows up.
        let payload = serde_json::to_string(&BlockingSetRequest {
            minimality: Some("Irreducible".to_owned()), status: Some("Unknown".to_owned()),
            validity: Some("Unknown".to_owned()), members: Vec::new(),
            verifications: Vec::new(), background_blocking: Some(true),
            extraction_tier: Some("Unavailable".to_owned()) }).unwrap();
        std::fs::write(directory.join("blocking-set.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn correction_probe_entry_shows_the_ascending_truncation_and_the_mcs_refusal() {
    use ospf_rust_bridge::{CorrectionCandidateSpec, CorrectionProbeReport, CorrectionProbeRequest};

    let engine = NativeEngine::default();
    let verify = |document: CorrectionProbeRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::CorrectionProbeVerify);
        request.correction_probe = Some(document);
        engine.submit(request)
    };
    let run = |document: CorrectionProbeRequest| {
        let view = verify(document).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: CorrectionProbeReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        engine.release(&view.id).unwrap();
        report
    };
    let spec = |id: &str, weight: f64, relaxation: f64| CorrectionCandidateSpec {
        id: id.to_owned(), weight, relaxation };
    let plain = CorrectionProbeRequest { max_candidates: None, max_plans: None,
        require_positive_weight: None,
        candidates: vec![spec("b", 3.0, 1.0), spec("a", 1.0, 2.0)], verified: None,
        minimal: None };
    let ask = |document: CorrectionProbeRequest| run(document);

    // 成本 2 与 3 → a 在前，总成本 5，且从不 minimal / 从不完整 MCS。 
    // Costs two and three put `a` first; the total is five, and it is never minimal or a complete MCS.
    let base = ask(plain.clone());
    assert!(base.built, "{}", base.error);
    assert_eq!(base.members.iter().map(|row| row.id.as_str()).collect::<Vec<_>>(),
        vec!["constraint:a", "constraint:b"]);
    assert_eq!(base.total_cost, 5.0);
    assert!(!base.minimal);
    assert!(!base.complete_mcs);
    assert!(base.requires_revalidation);
    assert!(base.the_recommendation_sorts_by_cost_ascending);
    assert!(base.the_truncation_keeps_the_cheapest);
    assert!(base.the_total_cost_is_the_sum_of_the_products);
    assert!(base.the_weighted_set_is_never_minimal);
    assert!(base.the_minimal_set_requires_the_verification_claim);
    assert!(base.the_truncation_drops_the_minimal_flag);
    assert!(base.the_policy_refuses_zero_limits);
    assert!(base.a_non_positive_weight_is_policy_dependent);

    // 截断留下最便宜的。 / The truncation keeps the cheapest.
    let capped = ask(CorrectionProbeRequest { max_candidates: Some(2), candidates: vec![
        spec("expensive", 9.0, 1.0), spec("cheap", 1.0, 1.0), spec("middle", 5.0, 1.0)],
        ..plain.clone() });
    assert_eq!(capped.members.iter().map(|row| row.id.as_str()).collect::<Vec<_>>(),
        vec!["constraint:cheap", "constraint:middle"]);

    // minimal 入口必须声明已复验；声明后截断会丢掉 minimal。 
    // The minimal path demands the claim, and truncation then drops the flag.
    let unverified = ask(CorrectionProbeRequest { minimal: Some(true), verified: Some(false),
        candidates: vec![spec("a", 1.0, 1.0)], ..plain.clone() });
    assert!(!unverified.built);
    assert!(unverified.error.contains("deletion verification"), "{}", unverified.error);
    let verified = ask(CorrectionProbeRequest { minimal: Some(true), verified: Some(true),
        candidates: vec![spec("a", 1.0, 1.0)], ..plain.clone() });
    assert!(verified.built, "{}", verified.error);
    assert!(verified.minimal);
    assert!(!verified.complete_mcs);
    let truncated = ask(CorrectionProbeRequest { minimal: Some(true), verified: Some(true),
        max_candidates: Some(2), candidates: vec![spec("a", 1.0, 1.0), spec("b", 2.0, 1.0),
            spec("c", 3.0, 1.0)], ..plain.clone() });
    assert!(truncated.built, "{}", truncated.error);
    assert_eq!(truncated.members.len(), 2);
    assert!(!truncated.minimal);

    // 策略零上限与负权重。 / The policy's zero limit and a negative weight.
    assert!(!ask(CorrectionProbeRequest { max_candidates: Some(0), ..plain.clone() }).built);
    assert!(!ask(CorrectionProbeRequest { max_plans: Some(0), ..plain.clone() }).built);
    assert!(!ask(CorrectionProbeRequest { candidates: vec![spec("a", -1.0, 1.0)],
        ..plain.clone() }).built);
    assert!(ask(CorrectionProbeRequest { require_positive_weight: Some(false),
        candidates: vec![spec("a", -1.0, 1.0)], ..plain.clone() }).built);
    assert!(!ask(CorrectionProbeRequest { require_positive_weight: Some(false),
        candidates: vec![spec("a", 1.0, 0.0)], ..plain.clone() }).built);

    // 拒绝：候选条数上限、缺请求。 / Refusals: the candidate cap and a missing request.
    assert!(verify(CorrectionProbeRequest {
        candidates: (0..9).map(|index| spec(&format!("c{index}"), 1.0, 1.0)).collect(),
        ..plain.clone() }).unwrap_err().message.contains("at most eight candidates"));
    let mut missing_request = base_request();
    missing_request.model_kind = Some(ModelKind::CorrectionProbeVerify);
    assert_eq!(engine.submit(missing_request).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        // 载荷故意让"截断后丢掉 minimal"：这一刻声明与截断的张力被看见。 
        // The payload truncates after a verification claim, where the tension between claim and truncation shows up.
        let payload = serde_json::to_string(&CorrectionProbeRequest { max_candidates: Some(2),
            max_plans: None, require_positive_weight: None, minimal: Some(true),
            verified: Some(true), candidates: vec![spec("a", 1.0, 1.0), spec("b", 2.0, 1.0),
                spec("c", 3.0, 1.0)] }).unwrap();
        std::fs::write(directory.join("correction-probe.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn sensitivity_probe_entry_shows_the_unordered_tolerances_and_the_loose_dual_test() {
    use ospf_rust_bridge::{LocalSensitivitySpec, SensitivityProbeReport, SensitivityProbeRequest};

    let engine = NativeEngine::default();
    let verify = |document: SensitivityProbeRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::SensitivityProbeVerify);
        request.sensitivity_probe = Some(document);
        engine.submit(request)
    };
    let run = |document: SensitivityProbeRequest| {
        let view = verify(document).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: SensitivityProbeReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        engine.release(&view.id).unwrap();
        report
    };
    let spec = |id: &str, dual: Option<&str>| LocalSensitivitySpec { id: id.to_owned(),
        dual: dual.map(str::to_owned), local_effective: None };
    let plain = SensitivityProbeRequest { integrality_tolerance: None, dual_tolerance: None,
        schema_version: None, baseline_objective: None, lp_objective: None,
        constraints: vec![spec("c1", Some("one"))], query: None };
    let ask = |document: SensitivityProbeRequest| run(document);

    // 默认配置与报告都合法。 / The default configuration and report both validate.
    let base = ask(plain.clone());
    assert!(base.config_valid, "{}", base.config_error);
    assert!(base.report_valid, "{}", base.report_error);
    assert!(base.has_duals);
    assert!(base.the_tolerances_must_be_finite_and_non_negative);
    assert!(base.the_tolerances_are_not_ordered);
    assert!(base.the_defaults_are_the_documented_pair);
    assert!(base.the_report_validation_covers_five_fields);
    assert!(base.has_duals_counts_any_some);
    assert!(base.has_duals_ignores_validity);
    assert!(base.the_lookup_is_first_wins);
    assert!(base.the_scope_name_differs_from_the_wire);

    // 容差只受"有限且非负"约束，没有顺序约束。 
    // The tolerances are bounded but unordered.
    assert!(ask(SensitivityProbeRequest { integrality_tolerance: Some(1e-12),
        dual_tolerance: Some(1e-3), ..plain.clone() }).config_valid);
    assert!(ask(SensitivityProbeRequest { integrality_tolerance: Some(1e-3),
        dual_tolerance: Some(1e-12), ..plain.clone() }).config_valid);
    let negative = ask(SensitivityProbeRequest { integrality_tolerance: Some(-1.0),
        ..plain.clone() });
    assert!(!negative.config_valid);
    assert!(negative.config_error.contains("finite and non-negative"), "{}", negative.config_error);

    // 报告校验覆盖五个字段。 / The report validation covers five fields.
    for spoiled in [
        SensitivityProbeRequest { schema_version: Some("  ".to_owned()), ..plain.clone() },
        SensitivityProbeRequest { baseline_objective: Some(f64::NAN), ..plain.clone() },
        SensitivityProbeRequest { lp_objective: Some(f64::INFINITY), ..plain.clone() },
        SensitivityProbeRequest { constraints: vec![spec("  ", Some("one"))], ..plain.clone() },
        SensitivityProbeRequest { constraints: vec![spec("c1", Some("infinite"))],
            ..plain.clone() }] {
        assert!(!ask(spoiled).report_valid);
    }

    // 零对偶算有对偶；非有限对偶让报告非法却仍算"有对偶"。 
    // A zero dual counts, and a non-finite one makes the report illegal yet still counts.
    assert!(ask(SensitivityProbeRequest { constraints: vec![spec("c1", Some("zero"))],
        ..plain.clone() }).has_duals);
    assert!(!ask(SensitivityProbeRequest { constraints: Vec::new(), ..plain.clone() }).has_duals);
    let broken = ask(SensitivityProbeRequest {
        constraints: vec![spec("c1", Some("infinite"))], ..plain.clone() });
    assert!(!broken.report_valid);
    assert!(broken.has_duals);

    // 查记录先到先得。 / The lookup is first wins.
    let duplicated = ask(SensitivityProbeRequest { constraints: vec![spec("c1", Some("one")),
        spec("c1", Some("zero"))], query: Some("c1".to_owned()), ..plain.clone() });
    assert!(duplicated.query_hit);
    assert_eq!(duplicated.query_dual, Some(1.0));
    let missing = ask(SensitivityProbeRequest { query: Some("c2".to_owned()),
        ..plain.clone() });
    assert!(!missing.query_hit);

    // 拒绝：约束条数上限、非法对偶形状、缺请求。 
    // Refusals: the constraint cap, an illegal dual shape, a missing request.
    assert!(verify(SensitivityProbeRequest {
        constraints: (0..9).map(|index| spec(&format!("c{index}"), None)).collect(),
        ..plain.clone() }).unwrap_err().message.contains("at most eight constraints"));
    assert!(verify(SensitivityProbeRequest { constraints: vec![spec("c1", Some("two"))],
        ..plain.clone() }).unwrap_err().message.contains("none/zero/one/infinite"));
    let mut missing_request = base_request();
    missing_request.model_kind = Some(ModelKind::SensitivityProbeVerify);
    assert_eq!(engine.submit(missing_request).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        // 载荷故意让"零对偶"：这一刻"有对偶"的宽松判定被看见。 
        // The payload carries a zero dual, where the loose "has duals" test shows up.
        let payload = serde_json::to_string(&SensitivityProbeRequest {
            integrality_tolerance: Some(1e-7), dual_tolerance: Some(1e-9),
            schema_version: None, baseline_objective: None, lp_objective: Some(0.0),
            constraints: vec![spec("c1", Some("zero")), spec("c2", Some("none"))],
            query: Some("c1".to_owned()) }).unwrap();
        std::fs::write(directory.join("sensitivity-probe.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn funnel_ranking_entry_shows_the_strict_tier_and_the_ignored_limit() {
    use ospf_rust_bridge::{FunnelProbeReport, FunnelProbeRequest, FunnelSortCandidateSpec};

    let engine = NativeEngine::default();
    let verify = |document: FunnelProbeRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::FunnelRankingVerify);
        request.funnel_ranking = Some(document);
        engine.submit(request)
    };
    let run = |document: FunnelProbeRequest| {
        let view = verify(document).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: FunnelProbeReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        engine.release(&view.id).unwrap();
        report
    };
    let spec = |id: &str, status: &str, dual: Option<f64>| FunnelSortCandidateSpec {
        id: id.to_owned(), status: status.to_owned(), group: None, dual };
    let plain = FunnelProbeRequest { dual_threshold: None, default_candidate_limit: None,
        candidates: vec![spec("a", "Active", Some(1.0)), spec("b", "Active", Some(0.0))],
        shortlist: None, with_sensitivity: None };
    let ask = |document: FunnelProbeRequest| run(document);

    // 对偶 1 落 A、0 落 B。 / A dual of one is tier A, zero is tier B.
    let base = ask(plain.clone());
    assert_eq!(base.tier_a, 1);
    assert_eq!(base.tier_b, 1);
    assert_eq!(base.candidates[0].id, "a");
    assert_eq!(base.candidates[0].tier, "A");
    assert_eq!(base.candidates[0].local_effective, Some(true));
    assert_eq!(base.candidates[1].local_effective, Some(false));
    assert!(base.the_tier_boundary_is_strictly_greater_than_the_threshold);
    assert!(base.both_active_states_share_the_dual_rule);
    assert!(base.only_the_slackless_states_reach_tier_c);
    assert!(base.no_sensitivity_means_no_tier_claim);
    assert!(base.the_ordering_is_tier_then_dual_then_identity);
    assert!(base.the_shortlist_prefers_the_strong_tiers);
    assert!(base.the_local_effectiveness_is_recomputed_here);
    assert!(base.the_candidate_limit_is_not_enforced);

    // 严格大于：正好等于阈值落 B。 / Strictly greater: exactly the threshold lands in B.
    let at = ask(FunnelProbeRequest { dual_threshold: Some(1.0),
        candidates: vec![spec("c1", "Active", Some(1.0))], ..plain.clone() });
    assert_eq!(at.candidates[0].tier, "B");
    let above = ask(FunnelProbeRequest { dual_threshold: Some(1.0),
        candidates: vec![spec("c1", "NearlyActive", Some(2.0))], ..plain.clone() });
    assert_eq!(above.candidates[0].tier, "A");

    // 只有无 slack 的两个状态到 C。 / Only the two slackless states reach C.
    let slackless = ask(FunnelProbeRequest { with_sensitivity: Some(false), candidates: vec![
        spec("a", "Inactive", None), spec("b", "SatisfiedWithoutSlackMetric", None),
        spec("c", "Violated", None), spec("d", "Unknown", None)], ..plain.clone() });
    assert_eq!(slackless.tier_c, 2);
    assert_eq!(slackless.unclassified, 2);

    // 没有灵敏度就没有分层，C 除外。 / No sensitivity means no tier claim, C excepted.
    let bare = ask(FunnelProbeRequest { with_sensitivity: Some(false), candidates: vec![
        spec("a", "Active", Some(9.0)), spec("b", "Inactive", None)], ..plain.clone() });
    assert_eq!(bare.tier_a, 0);
    assert_eq!(bare.tier_b, 0);
    assert_eq!(bare.tier_c, 1);
    assert!(bare.candidates.iter().all(|row| row.local_effective.is_none()));

    // 排序与短名单。 / The ordering and the shortlist.
    let ordered = ask(FunnelProbeRequest { shortlist: Some(2), candidates: vec![
        spec("z", "Inactive", None), spec("a", "Active", Some(1.0)),
        spec("b", "Active", Some(5.0)), spec("c", "Active", Some(1.0)),
        spec("d", "Active", Some(0.0))], ..plain.clone() });
    assert_eq!(ordered.candidates.iter().map(|row| row.id.as_str()).collect::<Vec<_>>(),
        vec!["b", "a", "c", "d", "z"]);
    assert_eq!(ordered.candidates.iter().map(|row| row.priority).collect::<Vec<_>>(),
        vec![0, 1, 2, 3, 4]);
    assert_eq!(ordered.shortlist, vec!["b".to_owned(), "a".to_owned()]);

    // 上限设 1 也不截断。 / A limit of one does not truncate.
    let uncapped = ask(FunnelProbeRequest { default_candidate_limit: Some(1),
        candidates: (0..5).map(|index| spec(&format!("c{index}"), "Active", Some(1.0)))
            .collect(), ..plain.clone() });
    assert_eq!(uncapped.candidates.len(), 5);

    // 拒绝：未知状态、条数上限、负阈值、缺请求。 
    // Refusals: an unknown status, the cap, a negative threshold, a missing request.
    assert!(verify(FunnelProbeRequest { candidates: vec![spec("c1", "Nearly", None)],
        ..plain.clone() }).unwrap_err().message.contains("unknown activity status"));
    assert!(verify(FunnelProbeRequest {
        candidates: (0..9).map(|index| spec(&format!("c{index}"), "Active", None)).collect(),
        ..plain.clone() }).unwrap_err().message.contains("at most eight candidates"));
    assert!(verify(FunnelProbeRequest { dual_threshold: Some(-1.0), ..plain.clone() })
        .unwrap_err().message.contains("finite and non-negative"));
    let mut missing_request = base_request();
    missing_request.model_kind = Some(ModelKind::FunnelRankingVerify);
    assert_eq!(engine.submit(missing_request).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        // 载荷故意让"正好等于阈值的对偶"：这一刻严格大于的分界被看见。 
        // The payload puts a dual exactly on the threshold, where the strict boundary shows up.
        let payload = serde_json::to_string(&FunnelProbeRequest {
            dual_threshold: Some(1.0), default_candidate_limit: Some(1),
            candidates: vec![spec("exact", "Active", Some(1.0)),
                spec("strong", "Active", Some(3.0)), spec("weak", "NearlyActive", Some(0.5)),
                spec("gone", "Inactive", None), spec("bad", "Violated", None)],
            shortlist: Some(2), with_sensitivity: Some(true) }).unwrap();
        std::fs::write(directory.join("funnel-ranking.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn effectiveness_ranking_entry_shows_the_two_independent_views() {
    use ospf_rust_bridge::{
        EffectivenessEntrySpec, EffectivenessRankingReport, EffectivenessRankingRequest,
    };

    let engine = NativeEngine::default();
    let verify = |document: EffectivenessRankingRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::EffectivenessRankingVerify);
        request.effectiveness_ranking_probe = Some(document);
        engine.submit(request)
    };
    let run = |document: EffectivenessRankingRequest| {
        let view = verify(document).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: EffectivenessRankingReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        engine.release(&view.id).unwrap();
        report
    };
    let spec = |id: &str| EffectivenessEntrySpec { id: id.to_owned(), dual: None,
        local_effective: None, removal_effective: None, first_effective_delta: None,
        max_improvement: None, marginal_effect: None, status: None };
    let plain = EffectivenessRankingRequest { schema_version: None, baseline_objective: None,
        entries: vec![EffectivenessEntrySpec { dual: Some(1.0), local_effective: Some(true),
            max_improvement: Some(2.0), ..spec("a") }] };
    let ask = |document: EffectivenessRankingRequest| run(document);

    // 默认：局部真、全局真。 / The default is local true and global true.
    let base = ask(plain.clone());
    assert!(base.ranking_valid, "{}", base.ranking_error);
    assert_eq!(base.global_count, 1);
    assert_eq!(base.local_count, 1);
    assert!(base.global_effective_needs_a_strictly_positive_improvement);
    assert!(base.the_views_filter_in_place_and_the_order_is_unchecked);
    assert!(base.local_effective_accepts_exactly_true);
    assert!(base.the_two_views_do_not_imply_each_other);
    assert!(base.validation_refuses_duplicates_and_non_finite_values);
    assert!(base.the_first_delta_must_be_positive);
    assert!(base.the_local_verdict_requires_a_dual);
    assert!(base.the_removal_verdict_only_rejects_unsupported);

    // 局部真、全局假。 / Local true, global false.
    let local_only = ask(EffectivenessRankingRequest { entries: vec![
        EffectivenessEntrySpec { dual: Some(1.0), local_effective: Some(true), ..spec("a") }],
        ..plain.clone() });
    assert!(local_only.rows[0].in_local_view);
    assert!(!local_only.rows[0].global_effective);
    // 全局真、局部假。 / Global true, local false.
    let global_only = ask(EffectivenessRankingRequest { entries: vec![
        EffectivenessEntrySpec { dual: Some(1.0), local_effective: Some(false),
            max_improvement: Some(3.0), ..spec("a") }], ..plain.clone() });
    assert!(!global_only.rows[0].in_local_view);
    assert!(global_only.rows[0].global_effective);

    // 改善为 0 / 负 / 缺失都不算全局有效。 / Zero, negative and absent improvements are not global.
    for improvement in [Some(0.0), Some(-1.0), None] {
        let report = ask(EffectivenessRankingRequest { entries: vec![
            EffectivenessEntrySpec { max_improvement: improvement, ..spec("a") }],
            ..plain.clone() });
        assert!(!report.rows[0].global_effective, "{improvement:?}");
    }

    // 乱序照样通过校验，视图保持原顺序。 
    // An unsorted ranking still validates and the view keeps the original order.
    let unsorted = ask(EffectivenessRankingRequest { entries: vec![
        EffectivenessEntrySpec { max_improvement: Some(1.0), ..spec("a") },
        EffectivenessEntrySpec { max_improvement: Some(5.0), ..spec("b") }], ..plain.clone() });
    assert!(unsorted.ranking_valid, "{}", unsorted.ranking_error);
    assert_eq!(unsorted.rows[0].id, "a");
    assert_eq!(unsorted.global_count, 2);

    // 拒绝：重复身份、空身份、非有限数值、非正 delta、无对偶的局部判定。 
    // Refusals: duplicates, a blank identity, non-finite values, a non-positive delta, a dual-less local verdict.
    for spoiled in [
        EffectivenessRankingRequest { entries: vec![spec("a"), spec("a")], ..plain.clone() },
        EffectivenessRankingRequest { entries: vec![spec("  ")], ..plain.clone() },
        EffectivenessRankingRequest { entries: vec![EffectivenessEntrySpec {
            dual: Some(f64::NAN), ..spec("a") }], ..plain.clone() },
        EffectivenessRankingRequest { entries: vec![EffectivenessEntrySpec {
            first_effective_delta: Some(0.0), ..spec("a") }], ..plain.clone() },
        EffectivenessRankingRequest { entries: vec![EffectivenessEntrySpec {
            local_effective: Some(true), ..spec("a") }], ..plain.clone() },
    ] {
        assert!(!ask(spoiled).ranking_valid);
    }

    // 单删判定只在 Unsupported 时被拒。 / The removal verdict is refused only for unsupported.
    let unsupported = ask(EffectivenessRankingRequest { entries: vec![
        EffectivenessEntrySpec { removal_effective: Some(true),
            status: Some("Unsupported".to_owned()), ..spec("a") }], ..plain.clone() });
    assert!(!unsupported.ranking_valid);
    let unknown = ask(EffectivenessRankingRequest { entries: vec![
        EffectivenessEntrySpec { removal_effective: Some(true),
            status: Some("Unknown".to_owned()), ..spec("a") }], ..plain.clone() });
    assert!(unknown.ranking_valid, "{}", unknown.ranking_error);

    // 拒绝：条数上限、未知状态、缺请求。 / Refusals: the cap, an unknown status, a missing request.
    assert!(verify(EffectivenessRankingRequest {
        entries: (0..9).map(|index| spec(&format!("c{index}"))).collect(),
        ..plain.clone() }).unwrap_err().message.contains("at most eight entries"));
    assert!(verify(EffectivenessRankingRequest { entries: vec![EffectivenessEntrySpec {
        status: Some("Nearly".to_owned()), ..spec("a") }], ..plain.clone() })
        .unwrap_err().message.contains("unknown entry status"));
    let mut missing_request = base_request();
    missing_request.model_kind = Some(ModelKind::EffectivenessRankingVerify);
    assert_eq!(engine.submit(missing_request).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        // 载荷故意让"两个视角各真一次"：这一刻互不蕴含被看见。 
        // The payload makes each view true once, where their non-implication shows up.
        let payload = serde_json::to_string(&EffectivenessRankingRequest {
            schema_version: None, baseline_objective: Some(0.0), entries: vec![
                EffectivenessEntrySpec { dual: Some(1.0), local_effective: Some(true),
                    ..spec("local-only") },
                EffectivenessEntrySpec { dual: Some(1.0), local_effective: Some(false),
                    max_improvement: Some(3.0), ..spec("global-only") },
                EffectivenessEntrySpec { dual: Some(1.0), local_effective: Some(true),
                    max_improvement: Some(2.0), ..spec("both") },
                EffectivenessEntrySpec { max_improvement: Some(0.0), ..spec("neither") }] })
            .unwrap();
        std::fs::write(directory.join("effectiveness-ranking-probe.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn persistence_sort_entry_shows_the_append_and_the_direction_only_support() {
    use ospf_rust_bridge::{
        PersistenceSortItemSpec, PersistenceSortProbeReport, PersistenceSortProbeRequest,
    };

    let engine = NativeEngine::default();
    let verify = |document: PersistenceSortProbeRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::PersistenceSortVerify);
        request.persistence_sort_probe = Some(document);
        engine.submit(request)
    };
    let run = |document: PersistenceSortProbeRequest| {
        let view = verify(document).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: PersistenceSortProbeReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        engine.release(&view.id).unwrap();
        report
    };
    let spec = |path: &str, direction: Option<&str>, nulls: Option<&str>| PersistenceSortItemSpec {
        path: path.to_owned(), direction: direction.map(str::to_owned),
        nulls: nulls.map(str::to_owned) };
    let plain = PersistenceSortProbeRequest {
        items: vec![spec("a", Some("Asc"), None), spec("b", Some("Desc"), Some("NullsLast"))],
        probe_direction: Some("Asc".to_owned()) };
    let ask = |document: PersistenceSortProbeRequest| run(document);

    // 默认：两项按顺序、支持判据只看 Asc。 / The default: two ordered items and an Asc-only support verdict.
    let base = ask(plain.clone());
    assert!(!base.empty);
    assert!(base.not_empty);
    assert_eq!(base.items.len(), 2);
    assert_eq!(base.items[0].path, "a");
    assert_eq!(base.items[1].nulls, Some("NullsLast".to_owned()));
    assert!(base.auto_supported && base.always_supported && !base.never_supported);
    assert!(base.only_asc_supported);
    assert!(base.the_then_family_appends_to_the_tail);
    assert!(base.then_does_not_merge_the_same_path);
    assert!(base.the_add_operator_is_then);
    assert!(base.emptiness_is_complementary);
    assert!(base.the_support_test_reads_the_direction_only);
    assert!(base.the_support_test_ignores_the_nulls_field);
    assert!(base.the_nulls_constructors_differ_only_by_the_nulls_field);
    assert!(base.the_many_constructors_keep_the_input_order);

    // 方向换成 Desc：只有 OnlyAsc 翻假。 / With Desc only the OnlyAsc verdict flips.
    let descending = ask(PersistenceSortProbeRequest {
        probe_direction: Some("Desc".to_owned()), ..plain.clone() });
    assert!(!descending.only_asc_supported);
    assert!(descending.auto_supported && descending.always_supported);
    assert!(!descending.never_supported);

    // 同路径不合并。 / The same path is never merged.
    let duplicated = ask(PersistenceSortProbeRequest { items: vec![
        spec("a", Some("Asc"), None), spec("a", Some("Desc"), None)], ..plain.clone() });
    assert_eq!(duplicated.items.len(), 2);
    assert_eq!(duplicated.items[1].direction, "Desc");

    // 空描述。 / An empty descriptor.
    let blank = ask(PersistenceSortProbeRequest { items: Vec::new(), ..plain.clone() });
    assert!(blank.empty && !blank.not_empty);
    assert!(blank.items.is_empty());

    // 拒绝：条数上限、未知方向、未知空值排序、未知探针方向、缺请求。 
    // Refusals: the cap, an unknown direction, an unknown null ordering, an unknown probe direction, a missing request.
    assert!(verify(PersistenceSortProbeRequest {
        items: (0..9).map(|index| spec(&format!("f{index}"), None, None)).collect(),
        ..plain.clone() }).unwrap_err().message.contains("at most eight items"));
    assert!(verify(PersistenceSortProbeRequest {
        items: vec![spec("a", Some("Sideways"), None)], ..plain.clone() }).unwrap_err()
        .message.contains("unknown direction"));
    assert!(verify(PersistenceSortProbeRequest {
        items: vec![spec("a", None, Some("Middle"))], ..plain.clone() }).unwrap_err()
        .message.contains("unknown null ordering"));
    assert!(verify(PersistenceSortProbeRequest {
        probe_direction: Some("Sideways".to_owned()), ..plain.clone() }).unwrap_err()
        .message.contains("unknown probe direction"));
    let mut missing_request = base_request();
    missing_request.model_kind = Some(ModelKind::PersistenceSortVerify);
    assert_eq!(engine.submit(missing_request).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        // 载荷故意让"同一字段写两次"：这一刻不合并被看见。 
        // The payload writes one field twice, where the absence of merging shows up.
        let payload = serde_json::to_string(&PersistenceSortProbeRequest {
            items: vec![spec("created_at", Some("Desc"), Some("NullsLast")),
                spec("id", Some("Asc"), None), spec("id", Some("Desc"), None)],
            probe_direction: Some("Desc".to_owned()) }).unwrap();
        std::fs::write(directory.join("persistence-sort-probe.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn persistence_schema_entry_shows_two_contradictory_lookup_faces() {
    use ospf_rust_bridge::{
        PersistenceFieldSpec, PersistenceSchemaReport, PersistenceSchemaRequest,
    };

    let engine = NativeEngine::default();
    let verify = |document: PersistenceSchemaRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::PersistenceSchemaVerify);
        request.persistence_schema_probe = Some(document);
        engine.submit(request)
    };
    let run = |document: PersistenceSchemaRequest| {
        let view = verify(document).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: PersistenceSchemaReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        engine.release(&view.id).unwrap();
        report
    };
    let spec = |path: &str, backend: Option<&str>| PersistenceFieldSpec {
        path: path.to_owned(), backend_field: backend.map(str::to_owned) };
    let plain = PersistenceSchemaRequest { name: Some("orders".to_owned()),
        fields: vec![spec("a", Some("A")), spec("b", None)],
        queries: vec!["a".to_owned(), "b".to_owned()] };
    let ask = |document: PersistenceSchemaRequest| run(document);

    // 默认：两条映射、两条查中。 / The default: two mappings and two hits.
    let base = ask(plain.clone());
    assert_eq!(base.name, "orders");
    assert_eq!(base.field_count, 2);
    assert_eq!(base.map_count, 2);
    assert_eq!(base.queries[0].resolved, Some("A".to_owned()));
    assert_eq!(base.queries[0].resolved_by_resolver, Some("A".to_owned()));
    assert_eq!(base.queries[0].resolved_by_map, Some("A".to_owned()));
    assert!(base.the_fields_append_in_order);
    assert!(base.the_resolve_is_first_wins);
    assert!(base.the_field_map_is_last_wins);
    assert!(base.the_unknown_path_resolves_to_nothing);
    assert!(base.the_same_field_mapping_repeats_the_whole_path);
    assert!(base.the_field_builder_never_checks_registration);
    assert!(base.the_parse_matches_the_constructor);
    assert!(base.the_field_map_is_deduplicated_by_path);

    // 同路径两条：`resolve` 给先到的，映射表给后到的。 
    // Two records of one path: `resolve` answers the first while the map answers the last.
    let duplicated = ask(PersistenceSchemaRequest {
        fields: vec![spec("a", Some("FIRST")), spec("a", Some("SECOND"))],
        queries: vec!["a".to_owned()], ..plain.clone() });
    assert_eq!(duplicated.field_count, 2);
    assert_eq!(duplicated.map_count, 1);
    assert_eq!(duplicated.queries[0].resolved, Some("FIRST".to_owned()));
    assert_eq!(duplicated.queries[0].resolved_by_map, Some("SECOND".to_owned()));

    // 整条路径文本当后端名。 / The whole path text becomes the backend name.
    let dotted = ask(PersistenceSchemaRequest {
        fields: vec![spec("order.customer.name", None)],
        queries: vec!["order.customer.name".to_owned()], ..plain.clone() });
    assert_eq!(dotted.queries[0].resolved, Some("order.customer.name".to_owned()));

    // 未登记与前缀都查不中。 / Unknown paths and prefixes both miss.
    let missing = ask(PersistenceSchemaRequest { queries: vec!["nope".to_owned(),
        "a.b".to_owned()], ..plain.clone() });
    for row in &missing.queries {
        assert_eq!(row.resolved, None);
        assert_eq!(row.resolved_by_resolver, None);
        assert_eq!(row.resolved_by_map, None);
    }

    // 拒绝：两条列表各自的上限、空路径、缺请求。 
    // Refusals: the two list caps, a blank path, a missing request.
    assert!(verify(PersistenceSchemaRequest {
        fields: (0..9).map(|index| spec(&format!("f{index}"), None)).collect(),
        ..plain.clone() }).unwrap_err().message.contains("at most eight entries"));
    assert!(verify(PersistenceSchemaRequest {
        queries: (0..9).map(|index| format!("q{index}")).collect(),
        ..plain.clone() }).unwrap_err().message.contains("at most eight entries"));
    assert!(verify(PersistenceSchemaRequest { fields: vec![spec("   ", None)],
        ..plain.clone() }).unwrap_err().message.contains("must not be blank"));
    let mut missing_request = base_request();
    missing_request.model_kind = Some(ModelKind::PersistenceSchemaVerify);
    assert_eq!(engine.submit(missing_request).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        // 载荷故意写两条同路径：这一刻两个查表面给出相反答案。 
        // The payload writes one path twice, where the two lookup faces answer oppositely.
        let payload = serde_json::to_string(&PersistenceSchemaRequest {
            name: Some("orders".to_owned()),
            fields: vec![spec("order.id", Some("FIRST")), spec("order.id", Some("SECOND")),
                spec("order.customer.name", None)],
            queries: vec!["order.id".to_owned(), "order.customer.name".to_owned(),
                "absent".to_owned()] }).unwrap();
        std::fs::write(directory.join("persistence-schema-probe.json"), payload.as_bytes())
            .unwrap();
    }
}

#[test]
fn predicate_scope_entry_shows_the_empty_and_single_item_semantics() {
    use ospf_rust_bridge::{
        PredicateScopeReport, PredicateScopeRequest, PredicateScopeSpec,
    };

    let engine = NativeEngine::default();
    let verify = |document: PredicateScopeRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::PredicateScopeVerify);
        request.predicate_scope_probe = Some(document);
        engine.submit(request)
    };
    let run = |document: PredicateScopeRequest| {
        let view = verify(document).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: PredicateScopeReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        engine.release(&view.id).unwrap();
        report
    };
    let spec = |path: &str, value: &str| PredicateScopeSpec {
        path: path.to_owned(), value: value.to_owned() };
    let plain = PredicateScopeRequest {
        predicates: vec![spec("status", "active"), spec("age", "18")], policy: None };
    let ask = |document: PredicateScopeRequest| run(document);

    // 默认：两项合并成 And/Or，策略是 AlwaysFalse。 
    // The default: two items combine into And/Or and the policy is always-false.
    let base = ask(plain.clone());
    assert_eq!(base.collected, 2);
    assert_eq!(base.and_shape, "and");
    assert_eq!(base.or_shape, "or");
    assert_eq!(base.and_operands, 2);
    assert_eq!(base.policy, "AlwaysFalse");
    assert!(!base.fails_fast && base.always_false);
    assert!(base.the_empty_conjunction_is_true);
    assert!(base.the_empty_disjunction_is_false);
    assert!(base.a_single_item_scope_returns_the_item_itself);
    assert!(base.two_or_more_items_are_combined);
    assert!(base.the_builders_return_and_collect_the_same_expression);
    assert!(base.the_default_is_the_empty_scope);
    assert!(base.only_one_policy_answers_each_way);
    assert!(base.the_client_filter_answers_false_to_both);

    // 空作用域：合取恒真、析取恒假。 / An empty scope: conjunction true, disjunction false.
    let blank = ask(PredicateScopeRequest { predicates: Vec::new(), ..plain.clone() });
    assert_eq!(blank.and_shape, "true");
    assert_eq!(blank.or_shape, "false");
    assert_eq!(blank.collected, 0);

    // 单项：不包 And/Or。 / A single item is left unwrapped.
    let single = ask(PredicateScopeRequest { predicates: vec![spec("a", "1")],
        ..plain.clone() });
    assert_eq!(single.and_shape, "single");
    assert_eq!(single.or_shape, "single");

    // 三种策略的判定。 / The three policy verdicts.
    let fail_fast = ask(PredicateScopeRequest { policy: Some("FailFast".to_owned()),
        ..plain.clone() });
    assert!(fail_fast.fails_fast && !fail_fast.always_false);
    let client = ask(PredicateScopeRequest { policy: Some("ClientFilter".to_owned()),
        ..plain.clone() });
    assert!(!client.fails_fast && !client.always_false);

    // 拒绝：条数上限、空路径、未知策略、缺请求。 
    // Refusals: the cap, a blank path, an unknown policy, a missing request.
    assert!(verify(PredicateScopeRequest {
        predicates: (0..9).map(|index| spec(&format!("f{index}"), "v")).collect(),
        ..plain.clone() }).unwrap_err().message.contains("at most eight predicates"));
    assert!(verify(PredicateScopeRequest { predicates: vec![spec("   ", "v")],
        ..plain.clone() }).unwrap_err().message.contains("must not be blank"));
    assert!(verify(PredicateScopeRequest { policy: Some("Ignore".to_owned()),
        ..plain.clone() }).unwrap_err().message.contains("unknown policy"));
    let mut missing_request = base_request();
    missing_request.model_kind = Some(ModelKind::PredicateScopeVerify);
    assert_eq!(engine.submit(missing_request).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        // 载荷故意只有一项：这一刻"单项不包 And"被看见。 
        // The payload carries one item only, where the unwrapped single item shows up.
        let payload = serde_json::to_string(&PredicateScopeRequest {
            predicates: vec![spec("status", "active")],
            policy: Some("ClientFilter".to_owned()) }).unwrap();
        std::fs::write(directory.join("predicate-scope-probe.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn native_terminal_proof_entry_shows_the_four_condition_gate() {
    use ospf_rust_bridge::{NativeTerminalProofReport, NativeTerminalProofRequest};

    let engine = NativeEngine::default();
    let verify = |document: NativeTerminalProofRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::NativeTerminalProofVerify);
        request.native_terminal_proof_probe = Some(document);
        engine.submit(request)
    };
    let run = |document: NativeTerminalProofRequest| {
        let view = verify(document).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: NativeTerminalProofReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        engine.release(&view.id).unwrap();
        report
    };
    let request = |native: &str| NativeTerminalProofRequest {
        native_status: native.to_owned(), problem_status: None, has_incumbent: None,
        termination: None, has_evidence: None };
    let ask = |document: NativeTerminalProofRequest| run(document);

    // 默认：Optimal 臂写出最优性证明。 / The default: the optimal arm writes an optimality proof.
    let base = ask(request("Optimal"));
    assert!(base.gate_ok, "{}", base.gate_error);
    assert_eq!(base.proof_kind, "Optimality");
    assert_eq!(base.proof_status, "Verified");
    assert_eq!(base.solution_presence, "Optimal");
    assert!(base.the_optimal_arm_writes_an_optimality_proof);
    assert!(base.the_infeasible_arm_writes_an_infeasibility_proof);
    assert!(base.the_old_evidence_is_carried_over);
    assert!(base.the_unbounded_arm_writes_an_unboundedness_proof);
    assert!(base.the_infeasible_or_unbounded_arm_writes_its_proof);
    assert!(base.a_mismatched_status_writes_nothing);
    assert!(base.a_non_completed_termination_writes_nothing);
    assert!(base.an_incumbent_blocks_the_negative_arms);

    // 四条臂各自写自己的种类。 / Each arm writes its own kind.
    for (native, problem, incumbent) in [("Infeasible", "Infeasible", false),
        ("Unbounded", "Unbounded", false),
        ("InfeasibleOrUnbounded", "InfeasibleOrUnbounded", false)] {
        let report = ask(NativeTerminalProofRequest { problem_status: Some(problem.to_owned()),
            has_incumbent: Some(incumbent), ..request(native) });
        assert_eq!(report.proof_status, "Verified", "{native}");
    }

    // 旧证据被搬过去。 / The old evidence survives.
    let evidence = ask(NativeTerminalProofRequest { problem_status: Some("Infeasible".to_owned()),
        has_incumbent: Some(false), has_evidence: Some(true), ..request("Infeasible") });
    assert_eq!(evidence.proof_kind, "Infeasibility");
    assert_eq!(evidence.evidence_count, 1);

    // 不匹配 / 非 Completed / 有 incumbent 都什么都不写。 
    // A mismatch, a non-completed termination and an incumbent all write nothing.
    let mismatched = ask(NativeTerminalProofRequest {
        problem_status: Some("Infeasible".to_owned()), ..request("Optimal") });
    assert_eq!(mismatched.proof_status, "Claimed");
    let timed_out = ask(NativeTerminalProofRequest {
        problem_status: Some("Infeasible".to_owned()), has_incumbent: Some(false),
        termination: Some("TimeLimit".to_owned()), ..request("Infeasible") });
    assert_eq!(timed_out.proof_status, "Claimed");
    let with_incumbent = ask(NativeTerminalProofRequest {
        problem_status: Some("Unbounded".to_owned()), has_incumbent: Some(true),
        ..request("Unbounded") });
    assert_eq!(with_incumbent.proof_status, "Claimed");

    // 拒绝：三个未知名字、缺请求。 / Refusals: three unknown names and a missing request.
    assert!(verify(NativeTerminalProofRequest { native_status: "Nearly".to_owned(),
        ..request("Optimal") }).unwrap_err().message.contains("unknown native status"));
    assert!(verify(NativeTerminalProofRequest { problem_status: Some("Nearly".to_owned()),
        ..request("Optimal") }).unwrap_err().message.contains("unknown problem status"));
    assert!(verify(NativeTerminalProofRequest { termination: Some("Nearly".to_owned()),
        ..request("Optimal") }).unwrap_err().message.contains("unknown termination"));
    let mut missing_request = base_request();
    missing_request.model_kind = Some(ModelKind::NativeTerminalProofVerify);
    assert_eq!(engine.submit(missing_request).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        // 载荷故意让"状态不匹配"：这一刻四条臂同时不成立被看见。 
        // The payload mismatches the statuses, where all four arms simultaneously fail to hold.
        let payload = serde_json::to_string(&NativeTerminalProofRequest {
            native_status: "Optimal".to_owned(),
            problem_status: Some("Infeasible".to_owned()), has_incumbent: Some(true),
            termination: Some("Completed".to_owned()), has_evidence: Some(true) }).unwrap();
        std::fs::write(directory.join("native-terminal-proof-probe.json"),
            payload.as_bytes()).unwrap();
    }
}

#[test]
fn native_lowering_entry_shows_the_uncrosschecked_counters() {
    use ospf_rust_bridge::{
        NativeLoweringProbeReport, NativeLoweringProbeRequest, NativeOutcomeSpec,
    };

    let engine = NativeEngine::default();
    let verify = |document: NativeLoweringProbeRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::NativeLoweringVerify);
        request.native_lowering_probe = Some(document);
        engine.submit(request)
    };
    let run = |document: NativeLoweringProbeRequest| {
        let view = verify(document).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: NativeLoweringProbeReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        engine.release(&view.id).unwrap();
        report
    };
    let spec = |kind: &str, text: Option<&str>| NativeOutcomeSpec {
        kind: kind.to_owned(), text: text.map(str::to_owned) };
    let plain = NativeLoweringProbeRequest {
        outcomes: vec![spec("Native", Some("probe")), spec("NoWriter", None)],
        materialized_fallbacks: None, native_writes: None };
    let ask = |document: NativeLoweringProbeRequest| run(document);

    // 一条原生一条退化。 / One native and one fallback.
    let base = ask(plain.clone());
    assert!(!base.fully_native);
    assert!(base.has_fallbacks);
    assert_eq!(base.outcome_count, 2);
    assert_eq!(base.native_outcomes, 1);
    assert_eq!(base.fallback_outcomes, 1);
    assert!(base.counts_agree);
    assert!(base.an_empty_report_is_not_fully_native);
    assert!(base.an_empty_report_has_no_fallbacks);
    assert!(base.the_two_predicates_are_not_complementary);
    assert!(base.fully_native_reads_only_the_counter);
    assert!(base.has_fallbacks_reads_only_the_counter);
    assert!(base.the_counts_are_never_cross_checked);
    assert!(base.the_two_outcome_predicates_are_complementary);
    assert!(base.the_fallback_reason_equals_structurally);

    // 空报告：两者皆假。 / An empty report: both false.
    let blank = ask(NativeLoweringProbeRequest { outcomes: Vec::new(),
        ..plain.clone() });
    assert!(!blank.fully_native && !blank.has_fallbacks);
    assert_eq!(blank.outcome_count, 0);

    // 计数撒谎也照样通过，只是 `countsAgree` 记录下矛盾。 
    // Lying counters still pass; only `countsAgree` records the contradiction.
    let lying = ask(NativeLoweringProbeRequest {
        materialized_fallbacks: Some(0), native_writes: Some(1), ..plain.clone() });
    assert!(lying.fully_native && !lying.has_fallbacks);
    assert!(!lying.counts_agree);

    // 全原生：自带计数一致时全原生。 / Everything native with honest counters.
    let all_native = ask(NativeLoweringProbeRequest {
        outcomes: vec![spec("Native", Some("a")), spec("Native", Some("b"))],
        ..plain.clone() });
    assert!(all_native.fully_native && !all_native.has_fallbacks);
    assert!(all_native.counts_agree);

    // 拒绝：未知变体名、条数上限、缺请求。 / Refusals: an unknown kind, the cap, a missing request.
    assert!(verify(NativeLoweringProbeRequest { outcomes: vec![spec("Nearly", None)],
        ..plain.clone() }).unwrap_err().message.contains("kind reads"));
    assert!(verify(NativeLoweringProbeRequest {
        outcomes: (0..65).map(|_| spec("NoWriter", None)).collect(),
        ..plain.clone() }).unwrap_err().message.contains("at most sixty-four"));
    let mut missing_request = base_request();
    missing_request.model_kind = Some(ModelKind::NativeLoweringVerify);
    assert_eq!(engine.submit(missing_request).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        // 载荷故意让计数撒谎：这一刻"报告不交叉核对"被看见。 
        // The payload lies in its counters, where the missing cross-check shows up.
        let payload = serde_json::to_string(&NativeLoweringProbeRequest {
            outcomes: vec![spec("NoWriter", None), spec("Rejected", Some("x"))],
            materialized_fallbacks: Some(0), native_writes: Some(1) }).unwrap();
        std::fs::write(directory.join("native-lowering-probe.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn solve_value_entry_shows_the_policy_equivalence_and_the_floor() {
    use ospf_rust_bridge::{SolveValueProbeReport, SolveValueProbeRequest};

    let engine = NativeEngine::default();
    let verify = |document: SolveValueProbeRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::SolveValueVerify);
        request.solve_value_probe = Some(document);
        engine.submit(request)
    };
    let run = |document: SolveValueProbeRequest| {
        let view = verify(document).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: SolveValueProbeReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        engine.release(&view.id).unwrap();
        report
    };
    let request = |value: &str| SolveValueProbeRequest {
        value: value.to_owned(), policy: None };
    let ask = |document: SolveValueProbeRequest| run(document);

    // 默认：Strict、有限值原样通过、上界 floor 到 1。 
    // The default: strict, a finite value passes untouched and the bound floors to one.
    let base = ask(request("1.5"));
    assert_eq!(base.type_name, "f64");
    assert_eq!(base.policy, "Strict");
    assert!(base.from_ok && base.to_ok);
    assert!(base.policies_agree);
    assert_eq!(base.upper_bound, Some(1));
    assert!(base.the_converter_only_refuses_non_finite);
    assert!(base.the_reader_only_refuses_non_finite);
    assert!(base.the_two_policies_agree_on_f64);
    assert!(base.the_default_policy_is_strict);
    assert!(base.the_upper_bound_floors);
    assert!(base.a_negative_zero_is_not_negative);
    assert!(base.a_negative_upper_bound_is_refused);
    assert!(base.an_out_of_range_upper_bound_overflows);

    // 非有限文本被两个方向同时拒掉。 / A non-finite text is refused by both faces.
    for text in ["nan", "inf", "-inf"] {
        let report = ask(request(text));
        assert!(!report.from_ok, "{text}");
        assert!(report.from_error.contains("cannot convert non-finite value"), "{text}");
        assert!(!report.to_ok, "{text}");
        assert!(!report.upper_bound_ok, "{text}");
    }

    // 两种策略给出同一结论。 / Both policies reach the same verdict.
    let rounding = ask(SolveValueProbeRequest { value: "2.7".to_owned(),
        policy: Some("AllowRounding".to_owned()) });
    assert_eq!(rounding.policy, "AllowRounding");
    assert!(rounding.policies_agree);
    assert_eq!(rounding.upper_bound, Some(2));

    // 负数上界被拒、-0.0 不算负数、超范围溢出。 
    // A negative bound is refused, -0.0 is not negative and an out-of-range bound overflows.
    let negative = ask(request("-1.5"));
    assert!(!negative.upper_bound_ok);
    assert!(negative.upper_bound_error.contains("cannot be negative"));
    assert!(negative.from_ok);
    assert_eq!(ask(request("-0")).upper_bound, Some(0));
    let huge = ask(request("1e19"));
    assert!(huge.upper_bound_error.contains("exceeds i64"));

    // 拒绝：数值文本读不懂、策略名不认识、缺请求。 
    // Refusals: an unparsable text, an unknown policy and a missing request.
    assert!(verify(request("nearly")).unwrap_err().message.contains("reads as a decimal"));
    assert!(verify(SolveValueProbeRequest { value: "1".to_owned(),
        policy: Some("Nearly".to_owned()) }).unwrap_err().message.contains("policy reads"));
    let mut missing_request = base_request();
    missing_request.model_kind = Some(ModelKind::SolveValueVerify);
    assert_eq!(engine.submit(missing_request).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        // 载荷故意取非有限的文本值：JSON 载不了非有限，而文本能，这一刻策略等价被看见。 
        // The payload uses a non-finite text: JSON cannot carry one, the text can, and the policy equivalence shows up.
        let payload = serde_json::to_string(&SolveValueProbeRequest {
            value: "inf".to_owned(), policy: Some("AllowRounding".to_owned()) }).unwrap();
        std::fs::write(directory.join("solve-value-probe.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn selection_policy_entry_shows_the_five_arms() {
    use ospf_rust_bridge::{
        SelectionObservationSpec, SelectionProbeReport, SelectionProbeRequest, SelectionStepSpec,
    };

    let engine = NativeEngine::default();
    let verify = |document: SelectionProbeRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::SelectionPolicyVerify);
        request.selection_policy_probe = Some(document);
        engine.submit(request)
    };
    let run = |document: SelectionProbeRequest| {
        let view = verify(document).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: SelectionProbeReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        engine.release(&view.id).unwrap();
        report
    };
    let actions = vec!["a".to_owned(), "b".to_owned(), "c".to_owned()];
    let plain = SelectionProbeRequest { policy: "fixed".to_owned(), actions: actions.clone(),
        learning: None, epsilon: None, alpha: None, gamma: None, states: None,
        stagnation_limit: None, seed: None, selects: Vec::new(), observations: Vec::new() };
    let ask = |document: SelectionProbeRequest| run(document);
    let observation = |action: &str, reward: f64| SelectionObservationSpec {
        action: action.to_owned(), reward, before: 0, after: 0, available: None,
        next_available: None, terminal: false };

    // 默认：规则臂不学习，八条规则全成立。 / The rule arm never learns and all eight rules hold.
    let base = ask(plain.clone());
    assert_eq!(base.snapshot_kind, "fixed");
    assert_eq!(base.revision, 0);
    assert_eq!(base.counts, vec![0, 0, 0]);
    assert_eq!(base.table_rows, 0);
    assert!(base.the_two_rule_kinds_share_one_arm);
    assert!(base.the_rule_cursor_advances_past_the_pick);
    assert!(base.stagnation_advances_only_on_a_nonzero_multiple);
    assert!(base.ucb1_prefers_unobserved_actions);
    assert!(base.a_non_learning_policy_learns_nothing);
    assert!(base.only_three_kinds_accumulate);
    assert!(base.only_q_learning_exports_a_table);
    assert!(base.six_illegal_definitions_are_refused);

    // 规则臂按顺序走掩码。 / The rule arm walks the mask in order.
    let walked = ask(SelectionProbeRequest {
        selects: (0..3).map(|_| SelectionStepSpec { state: 0, stagnation: 0,
            available: Vec::new() }).collect(), ..plain.clone() });
    assert_eq!(walked.selected, vec!["a".to_owned(), "b".to_owned(), "c".to_owned()]);

    // 三种 kind 学习、其余四种不学。 / Three kinds learn; the other four never do.
    let learned = ask(SelectionProbeRequest { learning: Some(true),
        observations: vec![observation("a", 2.0), observation("b", -1.0)],
        ..SelectionProbeRequest { policy: "epsilon-greedy".to_owned(), ..plain.clone() } });
    assert_eq!(learned.revision, 2);
    assert_eq!(learned.counts, vec![1, 1, 0]);
    let q = ask(SelectionProbeRequest { learning: Some(true),
        observations: vec![observation("a", 1.0)],
        ..SelectionProbeRequest { policy: "q-learning".to_owned(), ..plain.clone() } });
    assert!(q.table_rows > 0);
    let idle = ask(SelectionProbeRequest { learning: Some(true),
        observations: vec![observation("a", 2.0)], ..plain.clone() });
    assert_eq!(idle.revision, 0);

    // 拒绝：未知 kind、重复动作、mask 里未知动作、状态越界、参数出界。 
    // Refusals: an unknown kind, duplicate actions, an unknown mask entry, an out-of-range state and parameter.
    assert!(verify(SelectionProbeRequest { policy: "nearly".to_owned(),
        ..plain.clone() }).unwrap_err().message.contains("invalid selector"));
    assert!(verify(SelectionProbeRequest { actions: vec!["a".to_owned(), "a".to_owned()],
        ..plain.clone() }).unwrap_err().message.contains("invalid selector"));
    assert!(verify(SelectionProbeRequest {
        selects: vec![SelectionStepSpec { state: 0, stagnation: 0,
            available: vec!["zzz".to_owned()] }], ..plain.clone() }).unwrap_err()
        .message.contains("unknown action"));
    assert!(verify(SelectionProbeRequest {
        selects: vec![SelectionStepSpec { state: 999, stagnation: 0, available: Vec::new() }],
        ..plain.clone() }).unwrap_err().message.contains("invalid state"));
    assert!(verify(SelectionProbeRequest { epsilon: Some(1.5),
        ..SelectionProbeRequest { policy: "epsilon-greedy".to_owned(), ..plain.clone() } })
        .unwrap_err().message.contains("invalid learning"));
    let mut missing_request = base_request();
    missing_request.model_kind = Some(ModelKind::SelectionPolicyVerify);
    assert_eq!(engine.submit(missing_request).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        // 载荷故意只在 ucb1 上学习一条：这一刻"未观察动作优先"被看见。 
        // The payload learns one observation under ucb1, where the unobserved-action preference shows up.
        let payload = serde_json::to_string(&SelectionProbeRequest {
            policy: "ucb1".to_owned(), actions: actions.clone(), learning: Some(true),
            epsilon: None, alpha: None, gamma: None, states: None, stagnation_limit: None,
            seed: Some(7), selects: vec![SelectionStepSpec { state: 0, stagnation: 0,
                available: actions.clone() }],
            observations: vec![observation("a", 1.0)] }).unwrap();
        std::fs::write(directory.join("selection-policy-probe.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn non_stationary_entry_shows_the_frozen_predecessor_and_the_window() {
    use ospf_rust_bridge::{
        NonStationaryObservationSpec, NonStationaryProbeReport, NonStationaryProbeRequest,
        NonStationarySelectSpec,
    };

    let engine = NativeEngine::default();
    let verify = |document: NonStationaryProbeRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::NonStationaryVerify);
        request.non_stationary_probe = Some(document);
        engine.submit(request)
    };
    let run = |document: NonStationaryProbeRequest| {
        let view = verify(document).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: NonStationaryProbeReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        engine.release(&view.id).unwrap();
        report
    };
    let actions = vec!["a".to_owned(), "b".to_owned(), "c".to_owned()];
    let plain = NonStationaryProbeRequest { policy: "frr-mab".to_owned(),
        actions: actions.clone(), learning: None, window: None, exploration: None,
        decay: None, rate: None, minimum_probability: None, recency_weight: None, seed: None,
        selects: Vec::new(), observations: Vec::new(), predecessor_of: Vec::new(),
        discards: Vec::new() };
    let ask = |document: NonStationaryProbeRequest| run(document);
    let observation = |action: &str, decision: u64, reward: f64| NonStationaryObservationSpec {
        action: action.to_owned(), decision, reward, terminal: false };

    // 默认：八条规则全成立。 / The default: all eight rules hold.
    let base = ask(plain.clone());
    assert!(base.the_bootstrap_follows_the_canonical_order);
    assert!(base.the_window_forgets_the_oldest);
    assert!(base.the_scale_makes_rewards_scale_invariant);
    assert!(base.the_predecessor_is_frozen_at_dispatch);
    assert!(base.a_pending_decision_cannot_be_dispatched_twice);
    assert!(base.a_full_rate_pursuit_lands_on_the_floor);
    assert!(base.a_terminal_completion_clears_the_predecessor);
    assert!(base.a_non_learning_selector_validates_without_writing);
    assert_eq!(base.revision, 0.0);

    // 引导按动作规范顺序：掩码 [c, b] 先给 b。 
    // The bootstrap follows the canonical order: the mask `[c, b]` yields `b` first.
    let bootstrap = ask(NonStationaryProbeRequest { learning: Some(true),
        selects: vec![NonStationarySelectSpec { decision: 0,
            available: Some(vec!["c".to_owned(), "b".to_owned()]) }], ..plain.clone() });
    assert_eq!(bootstrap.selected, vec!["b".to_owned()]);

    // 窗口与时钟随反馈推进。 / The window and clock follow the feedback.
    let windowed = ask(NonStationaryProbeRequest { learning: Some(true), window: Some(3),
        observations: vec![observation("a", 0, 1.0), observation("b", 1, 2.0),
            observation("c", 2, 3.0), observation("a", 3, 4.0)], ..plain.clone() });
    assert_eq!(windowed.revision, 4.0);
    assert_eq!(windowed.clock, 4.0);
    assert_eq!(windowed.window_len, 3);

    // 追逐的保底概率精确落位。 / The pursuit lands exactly on its floor.
    let pursuit = ask(NonStationaryProbeRequest { learning: Some(true), rate: Some(1.0),
        minimum_probability: Some(0.1), observations: vec![observation("a", 0, 2.0)],
        ..NonStationaryProbeRequest { policy: "adaptive-pursuit".to_owned(), ..plain.clone() } });
    assert!((pursuit.probabilities[0] - 0.8).abs() < 1e-12);
    assert!((pursuit.probabilities[1] - 0.1).abs() < 1e-12);

    // 前驱冻结与 discard。 / The frozen predecessor and `discard`.
    let pending = ask(NonStationaryProbeRequest { learning: Some(true),
        predecessor_of: vec![1], observations: vec![observation("a", 0, 2.0)],
        selects: vec![NonStationarySelectSpec { decision: 1,
            available: Some(vec!["b".to_owned()]) }],
        ..NonStationaryProbeRequest { policy: "choice-function".to_owned(), ..plain.clone() } });
    assert_eq!(pending.pending, 1);
    // 反馈先于派发，所以这条派发的前驱是上一条反馈的 a。 
    // Feedback precedes dispatch, so this dispatch's predecessor is the previous feedback's `a`.
    assert_eq!(pending.predecessors, vec![Some("a".to_owned())]);
    let released = ask(NonStationaryProbeRequest { learning: Some(true),
        discards: vec![1],
        selects: vec![NonStationarySelectSpec { decision: 1,
            available: Some(vec!["b".to_owned()]) }],
        ..NonStationaryProbeRequest { policy: "choice-function".to_owned(), ..plain.clone() } });
    assert_eq!(released.pending, 0);

    // 拒绝：未知 kind、窗口越界、参数出界、掩码未知动作、缺请求。 
    // Refusals: an unknown kind, an out-of-range window, an out-of-range parameter, an unknown mask entry, a missing request.
    assert!(verify(NonStationaryProbeRequest { policy: "nearly".to_owned(), ..plain.clone() })
        .unwrap_err().message.contains("invalid non-stationary"));
    assert!(verify(NonStationaryProbeRequest { window: Some(2), ..plain.clone() })
        .unwrap_err().message.contains("invalid non-stationary"));
    assert!(verify(NonStationaryProbeRequest { window: Some(1_000_001), ..plain.clone() })
        .unwrap_err().message.contains("invalid non-stationary"));
    assert!(verify(NonStationaryProbeRequest { minimum_probability: Some(0.4),
        ..NonStationaryProbeRequest { policy: "adaptive-pursuit".to_owned(),
            ..plain.clone() } }).unwrap_err().message.contains("invalid non-stationary"));
    assert!(verify(NonStationaryProbeRequest { learning: Some(true),
        selects: vec![NonStationarySelectSpec { decision: 0,
            available: Some(vec!["zzz".to_owned()]) }], ..plain.clone() }).unwrap_err()
        .message.contains("mask"));
    let mut missing_request = base_request();
    missing_request.model_kind = Some(ModelKind::NonStationaryVerify);
    assert_eq!(engine.submit(missing_request).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        // 载荷故意只派发不反馈：这一刻"待反馈表里前驱被冻结"被看见。 
        // The payload dispatches without feedback, where the frozen predecessor in the pending table shows up.
        let payload = serde_json::to_string(&NonStationaryProbeRequest {
            policy: "choice-function".to_owned(), actions: actions.clone(), learning: Some(true),
            window: None, exploration: None, decay: None, rate: None, minimum_probability: None,
            recency_weight: None, seed: Some(7),
            selects: vec![NonStationarySelectSpec { decision: 1,
                available: Some(vec!["b".to_owned()]) },
                NonStationarySelectSpec { decision: 2,
                    available: Some(vec!["c".to_owned()]) }],
            observations: vec![NonStationaryObservationSpec { action: "a".to_owned(),
                decision: 0, reward: 2.0, terminal: false }],
            predecessor_of: vec![1, 2], discards: Vec::new() }).unwrap();
        std::fs::write(directory.join("non-stationary-probe.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn training_split_entry_shows_the_structural_problem_equality() {
    use ospf_rust_bridge::{
        TrainingCaseSpec, TrainingParameterSpec, TrainingSplitProbeReport,
        TrainingSplitProbeRequest,
    };

    let engine = NativeEngine::default();
    let verify = |document: TrainingSplitProbeRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::TrainingSplitVerify);
        request.training_split_probe = Some(document);
        engine.submit(request)
    };
    let run = |document: TrainingSplitProbeRequest| {
        let view = verify(document).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: TrainingSplitProbeReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        engine.release(&view.id).unwrap();
        report
    };
    let spec = |id: &str, problem: &str| TrainingCaseSpec { id: id.to_owned(),
        problem: problem.to_owned(), version: None, parameters: Vec::new() };
    let plain = TrainingSplitProbeRequest { training: vec![spec("x", "p1")],
        validation: vec![spec("y", "p2")], test: Vec::new() };
    let ask = |document: TrainingSplitProbeRequest| run(document);

    // 默认：合法切分。 / The default: a valid split.
    let base = ask(plain.clone());
    assert!(base.valid, "{}", base.error);
    assert_eq!(base.error, "");
    assert_eq!(base.instances, 2);
    assert!(base.an_empty_id_is_refused);
    assert!(base.a_repeated_id_inside_one_split_is_refused);
    assert!(base.a_repeated_id_across_splits_is_refused);
    assert!(base.an_empty_training_split_is_refused);
    assert!(base.a_problem_leaking_across_splits_is_refused);
    assert!(base.a_repeated_problem_inside_one_split_is_accepted);
    assert!(base.the_problem_equality_is_structural);
    assert!(base.the_id_check_fires_before_the_leak_check);

    // 空 ID / 重复 ID / 跨集合重复 ID 各报 ID 那条。 
    // An empty id, a doubled id and a cross-split id all report the id message.
    let blank = ask(TrainingSplitProbeRequest { training: vec![spec("", "p1")],
        ..plain.clone() });
    assert!(!blank.valid);
    assert!(blank.error.contains("duplicate or empty training instance ID"));
    let doubled = ask(TrainingSplitProbeRequest {
        training: vec![spec("x", "p1"), spec("x", "p2")], ..plain.clone() });
    assert!(doubled.error.contains("duplicate or empty training instance ID"));

    // 训练集为空被拒，验证集为空被接受。 
    // An empty training split is refused; an empty validation part is fine.
    let empty = ask(TrainingSplitProbeRequest { training: Vec::new(),
        validation: vec![spec("y", "p2")], test: Vec::new() });
    assert!(!empty.valid);
    assert!(empty.error.contains("training instances are required"));
    let bare = ask(TrainingSplitProbeRequest { training: vec![spec("x", "p1")],
        validation: Vec::new(), test: Vec::new() });
    assert!(bare.valid, "{}", bare.error);

    // 泄漏判定用结构相等：相同定义泄漏，只差版本不算。 
    // The leak rule compares structurally: identical leaks, a differing version does not.
    let leak = ask(TrainingSplitProbeRequest { training: vec![spec("x", "p1")],
        validation: vec![spec("y", "p1")], test: Vec::new() });
    assert!(!leak.valid);
    assert!(leak.error.contains("leaks across training splits"));
    let versioned = ask(TrainingSplitProbeRequest {
        training: vec![spec("x", "p1")],
        validation: vec![TrainingCaseSpec { version: Some(2), ..spec("y", "p1") }],
        test: Vec::new() });
    assert!(versioned.valid, "{}", versioned.error);
    let parameterized = ask(TrainingSplitProbeRequest {
        training: vec![spec("x", "p1")],
        validation: vec![TrainingCaseSpec {
            parameters: vec![TrainingParameterSpec { key: "seed".to_owned(),
                value: "7".to_owned() }], ..spec("y", "p1") }],
        test: Vec::new() });
    assert!(parameterized.valid, "{}", parameterized.error);
    // 同一集合内同一问题重复被接受。 / A repeated problem inside one split is accepted.
    let twice = ask(TrainingSplitProbeRequest {
        training: vec![spec("x", "p1"), spec("z", "p1")], validation: Vec::new(),
        test: Vec::new() });
    assert!(twice.valid, "{}", twice.error);

    // 拒绝：空白问题名、条目上限、缺请求。 
    // Refusals: a blank problem name, the count cap and a missing request.
    assert!(verify(TrainingSplitProbeRequest { training: vec![spec("x", "  ")],
        ..plain.clone() }).unwrap_err().message.contains("must not be blank"));
    assert!(verify(TrainingSplitProbeRequest {
        training: (0..129).map(|index| spec(&format!("i{index}"), "p1")).collect(),
        ..plain.clone() }).unwrap_err().message.contains("one hundred and twenty-eight"));
    let mut missing_request = base_request();
    missing_request.model_kind = Some(ModelKind::TrainingSplitVerify);
    assert_eq!(engine.submit(missing_request).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        // 载荷故意用"只差版本"的同一问题：这一刻"结构相等参与判定"被看见。 
        // The payload repeats one problem differing only in its version, where the structural equality shows up.
        let payload = serde_json::to_string(&TrainingSplitProbeRequest {
            training: vec![spec("x", "p1")],
            validation: vec![TrainingCaseSpec { version: Some(2), ..spec("y", "p1") }],
            test: vec![spec("z", "p3")] }).unwrap();
        std::fs::write(directory.join("training-split-probe.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn acceptance_recovery_entry_shows_the_definition_and_history_locks() {
    use ospf_rust_bridge::{
        AcceptanceDefinitionSpec, AcceptanceLoadRow, AcceptanceOverrideSpec,
        AcceptanceRecoveryReport, AcceptanceRecoveryRequest,
    };

    let engine = NativeEngine::default();
    let verify = |document: AcceptanceRecoveryRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::AcceptanceRecoveryVerify);
        request.acceptance_recovery_probe = Some(document);
        engine.submit(request)
    };
    let run = |document: AcceptanceRecoveryRequest| {
        let view = verify(document).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: AcceptanceRecoveryReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        engine.release(&view.id).unwrap();
        report
    };
    let definition = |policy: &str| AcceptanceDefinitionSpec { policy: policy.to_owned(),
        length: None, ratio: None, horizon: None };
    let number_override = |key: &str, value: f64| AcceptanceOverrideSpec {
        key: key.to_owned(), null: None, number: Some(value), numbers: None, text: None };
    let plain = AcceptanceRecoveryRequest { definition: definition("late"),
        overrides: Vec::new() };
    let ask = |document: AcceptanceRecoveryRequest| run(document);

    // 默认：late 的保存键就是框架自己那三个。 / The saved keys are the framework's own three.
    let base = ask(plain.clone());
    assert_eq!(base.policy, "late");
    assert!(base.baseline_accepted);
    assert_eq!(base.saved_keys, vec!["cursor".to_owned(), "history".to_owned(),
        "length".to_owned()]);
    assert_eq!(base.history_span, 0);
    assert!(base.the_late_length_range_is_hardcoded);
    assert!(base.the_late_definition_is_locked);
    assert!(base.the_late_history_stays_consistent);
    assert!(base.the_record_ratio_range_is_hardcoded);
    assert!(base.the_record_best_is_bounded_by_its_scale);
    assert!(base.the_auto_span_is_derived);
    assert!(base.the_auto_span_and_improvements_are_bounded);
    assert!(base.the_auto_history_matches_its_decisions);

    // record / auto 的保存键各自不同。 / The record and auto policies save different keys.
    let record = ask(AcceptanceRecoveryRequest {
        definition: AcceptanceDefinitionSpec { ratio: Some(0.05), ..definition("record-to-record") },
        overrides: Vec::new() });
    assert_eq!(record.saved_keys, vec!["best".to_owned(), "clock".to_owned(),
        "horizon".to_owned(), "ratio".to_owned(), "scale".to_owned()]);
    assert!(record.baseline_accepted);
    let auto = ask(AcceptanceRecoveryRequest { definition: definition("auto"),
        overrides: Vec::new() });
    assert!(auto.saved_keys.contains(&"capacity".to_owned()));
    assert_eq!(auto.history_span, 15);

    // 每处改写各回一行；改定义与越界游标都被拒。 
    // Every override comes back as one row; a changed definition and an out-of-range cursor are both refused.
    let rows: Vec<AcceptanceLoadRow> = ask(AcceptanceRecoveryRequest {
        overrides: vec![number_override("length", 6.0), number_override("cursor", 5.0)],
        ..plain.clone() }).rows;
    assert_eq!(rows.len(), 2);
    assert!(!rows[0].accepted && rows[0].error.contains("invalid late acceptance definition"));
    assert!(!rows[1].accepted);

    // 只改 scale 是合法的：record 的 best 仍空、clock 仍 0。 
    // Changing only the scale is legal: the record's best stays empty and its clock stays zero.
    let legal = ask(AcceptanceRecoveryRequest {
        definition: AcceptanceDefinitionSpec { ratio: Some(0.05),
            ..definition("record-to-record") },
        overrides: vec![number_override("scale", 1.0)] });
    assert!(legal.rows[0].accepted, "{}", legal.rows[0].error);

    // 拒绝：未知准则、缺 ratio、length 出界、overrides 上限、缺请求。 
    // Refusals: an unknown policy, a missing ratio, an out-of-range length, the override cap and a missing request.
    assert!(verify(AcceptanceRecoveryRequest { definition: definition("nearly"),
        overrides: Vec::new() }).unwrap_err().message.contains("policy reads"));
    assert!(verify(AcceptanceRecoveryRequest { definition: definition("record-to-record"),
        overrides: Vec::new() }).unwrap_err().message.contains("needs a ratio"));
    assert!(verify(AcceptanceRecoveryRequest {
        definition: AcceptanceDefinitionSpec { length: Some(0), ..definition("late") },
        overrides: Vec::new() }).unwrap_err().message.contains("invalid late acceptance length"));
    assert!(verify(AcceptanceRecoveryRequest { definition: definition("late"),
        overrides: (0..17).map(|index| number_override(&format!("k{index}"), 1.0)).collect() })
        .unwrap_err().message.contains("sixteen overrides"));
    let mut missing_request = base_request();
    missing_request.model_kind = Some(ModelKind::AcceptanceRecoveryVerify);
    assert_eq!(engine.submit(missing_request).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        // 载荷故意改 `best` 而不动 clock：这一刻"时钟与 best 必须同步"被看见。 
        // The payload sets `best` without moving the clock, where the required synchronisation shows up.
        let payload = serde_json::to_string(&AcceptanceRecoveryRequest {
            definition: AcceptanceDefinitionSpec { policy: "record-to-record".to_owned(),
                length: None, ratio: Some(0.05), horizon: Some(10) },
            overrides: vec![number_override("best", 0.5), number_override("scale", 1.0)] })
            .unwrap();
        std::fs::write(directory.join("acceptance-recovery-probe.json"), payload.as_bytes())
            .unwrap();
    }
}

#[test]
fn hypervolume_entry_shows_the_three_phase_lifecycle() {
    use ospf_rust_bridge::{
        HypervolumeMeasurementSpec, HypervolumeProbeReport, HypervolumeProbeRequest,
    };

    let engine = NativeEngine::default();
    let verify = |document: HypervolumeProbeRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::HypervolumeVerify);
        request.hypervolume_probe = Some(document);
        engine.submit(request)
    };
    let run = |document: HypervolumeProbeRequest| {
        let view = verify(document).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: HypervolumeProbeReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        engine.release(&view.id).unwrap();
        report
    };
    let spec = |reference: Vec<Vec<f64>>, candidates: Vec<Vec<f64>>| HypervolumeMeasurementSpec {
        reference, candidates };
    let plain = HypervolumeProbeRequest { mode: None, reference: vec![10.0, 10.0],
        max_work: None, samples: None, seed: None, calibration_observations: None,
        measurements: Vec::new() };
    let ask = |document: HypervolumeProbeRequest| run(document);

    // 默认：档案 5×5、候选 0×0（最小化）→ 增量 75，全精确。 
    // The default: an archive at five and a candidate at zero under minimisation gain seventy-five, exactly.
    let base = ask(HypervolumeProbeRequest {
        measurements: vec![spec(vec![vec![5.0, 5.0]], vec![vec![0.0, 0.0]])],
        ..plain.clone() });
    assert_eq!(base.rows.len(), 1);
    let row = &base.rows[0];
    assert!(row.ok, "{}", row.error);
    assert!(row.exact);
    assert!((row.delta - 75.0).abs() < 1e-9, "{}", row.delta);
    assert!(row.work > 0 && row.calibrated);
    assert!((base.last_delta - 75.0).abs() < 1e-9);
    assert!(base.the_calibration_phase_reports_zero_uncertain);
    assert!(base.the_calibration_freezes_the_reference);
    assert!(base.the_exact_increment_is_nonnegative_and_counted);
    assert!(base.the_sampled_increment_carries_uncertainty);
    assert!(base.an_empty_increment_is_zero);
    assert!(base.bad_coordinates_are_refused);
    assert!(base.the_geometry_budget_is_hardcoded);
    assert!(base.a_failure_keeps_the_last_measurement);

    // 校准：自动参考点、前三步全 0、第三步冻结。 
    // Calibration: an automatic reference, three zero steps and a freeze on the third.
    let calibrated = ask(HypervolumeProbeRequest { reference: Vec::new(),
        calibration_observations: Some(3),
        measurements: (1..=3).map(|step| spec(vec![vec![0.0]],
            vec![vec![step as f64]])).collect(), ..plain.clone() });
    assert_eq!(calibrated.rows.len(), 3);
    assert_eq!(calibrated.rows[0].delta, 0.0);
    assert!(!calibrated.rows[0].calibrated);
    assert!(calibrated.rows[2].calibrated);
    assert_eq!(calibrated.last_delta, 0.0);

    // 采样模式带两个误差量。 / The sampled mode carries two uncertainty figures.
    let sampled = ask(HypervolumeProbeRequest { mode: Some("Sampled".to_owned()),
        samples: Some(1024),
        measurements: vec![spec(vec![vec![2.0, 8.0]], vec![vec![8.0, 2.0]])],
        ..plain.clone() });
    let sampled_row = &sampled.rows[0];
    assert!(sampled_row.ok, "{}", sampled_row.error);
    assert!(!sampled_row.exact && sampled_row.samples == 1024);
    assert!(sampled_row.standard_error > 0.0 && sampled_row.error_bound > 0.0);

    // 坏坐标变成行里的错误，不是请求失败。 
    // Bad coordinates become a row error rather than a request failure.
    let bad = ask(HypervolumeProbeRequest {
        measurements: vec![spec(vec![vec![0.0, 0.0], vec![1.0]], Vec::new())],
        ..plain.clone() });
    assert!(!bad.rows[0].ok);
    assert!(bad.rows[0].error.contains("invalid hypervolume coordinate"));

    // 拒绝：未知模式、三处额度为零、缺请求。 
    // Refusals: an unknown mode, three zero budgets and a missing request.
    assert!(verify(HypervolumeProbeRequest { mode: Some("Nearly".to_owned()),
        ..plain.clone() }).unwrap_err().message.contains("mode reads Exact or Sampled"));
    for zero in [HypervolumeProbeRequest { max_work: Some(0), ..plain.clone() },
        HypervolumeProbeRequest { samples: Some(0), ..plain.clone() },
        HypervolumeProbeRequest { calibration_observations: Some(0), ..plain.clone() }] {
        assert!(verify(zero).unwrap_err().message.contains("invalid hypervolume parameters"));
    }
    let mut missing_request = base_request();
    missing_request.model_kind = Some(ModelKind::HypervolumeVerify);
    assert_eq!(engine.submit(missing_request).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        // 载荷用采样模式 + 部分覆盖：这一刻"误差量非零"被看见。 
        // The payload samples a partly covered region, where the non-zero uncertainty shows up.
        let payload = serde_json::to_string(&HypervolumeProbeRequest {
            mode: Some("Sampled".to_owned()), reference: vec![10.0, 10.0],
            max_work: None, samples: Some(1024), seed: Some(1),
            calibration_observations: None,
            measurements: vec![spec(vec![vec![2.0, 8.0]], vec![vec![8.0, 2.0]])] }).unwrap();
        std::fs::write(directory.join("hypervolume-probe.json"), payload.as_bytes()).unwrap();
    }
}

#[test]
fn operator_capability_entry_shows_the_three_gates() {
    use ospf_rust_bridge::{
        OperatorCapabilityReport, OperatorCapabilityRequest, OperatorRegistrationRow,
        OperatorRegistrationSpec,
    };

    let engine = NativeEngine::default();
    let verify = |document: OperatorCapabilityRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::OperatorCapabilityVerify);
        request.operator_capability_probe = Some(document);
        engine.submit(request)
    };
    let run = |document: OperatorCapabilityRequest| {
        let view = verify(document).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: OperatorCapabilityReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        engine.release(&view.id).unwrap();
        report
    };
    let spec = |face: &str, id: &str, encodings: &[&str]| OperatorRegistrationSpec {
        face: face.to_owned(), id: id.to_owned(),
        encodings: encodings.iter().map(|name| (*name).to_owned()).collect(),
        stateless: None };
    let plain = OperatorCapabilityRequest {
        registrations: vec![spec("mutation", "m", &["Continuous"])] };
    let ask = |document: OperatorCapabilityRequest| run(document);

    // 默认：一次合法注册。 / The default: one legal registration.
    let base = ask(plain.clone());
    assert!(base.rows[0].accepted, "{}", base.rows[0].error);
    assert_eq!(base.rows[0].total_accepted, 1);
    assert!(base.an_empty_encoding_list_is_refused);
    assert!(base.a_non_adjacent_repeat_is_refused);
    assert!(base.a_legal_declaration_registers);
    assert!(base.the_stateless_flag_is_not_validated);
    assert!(base.an_empty_operator_id_is_refused);
    assert!(base.a_duplicate_operator_id_is_refused);
    assert!(base.the_capability_check_precedes_the_id_check);
    assert!(base.each_face_keeps_its_own_capabilities);

    // 空编码表被拒。 / An empty list is refused.
    let empty = ask(OperatorCapabilityRequest {
        registrations: vec![spec("mutation", "m", &[])] });
    assert!(!empty.rows[0].accepted);
    assert!(empty.rows[0].error.contains("unique nonempty encodings"));

    // 非相邻重复被拒，三种各不相同就过。 
    // A non-adjacent repeat is refused while three distinct encodings pass.
    let repeated = ask(OperatorCapabilityRequest {
        registrations: vec![spec("mutation", "m", &["Custom", "Permutation", "Custom"])] });
    assert!(!repeated.rows[0].accepted);
    let distinct = ask(OperatorCapabilityRequest {
        registrations: vec![spec("mutation", "m",
            &["Continuous", "Permutation", "Custom"])] });
    assert!(distinct.rows[0].accepted, "{}", distinct.rows[0].error);

    // stateless 不参与校验：true 与 false 都过。 
    // The stateless flag is not validated: true and false both pass.
    let flagged = ask(OperatorCapabilityRequest { registrations: vec![
        OperatorRegistrationSpec { stateless: Some(true),
            ..spec("mutation", "with", &["Continuous"]) },
        OperatorRegistrationSpec { stateless: Some(false),
            ..spec("neighborhood", "without", &["Permutation"]) }] });
    assert!(flagged.rows.iter().all(|row| row.accepted));

    // id 的三条：空、重复、以及"能力校验先于 id 校验"。 
    // The three id readings: empty, duplicate, and the capability check preceding it.
    let ids: Vec<OperatorRegistrationRow> = ask(OperatorCapabilityRequest { registrations: vec![
        spec("mutation", "", &["Continuous"]),
        spec("mutation", "m", &["Continuous"]),
        spec("mutation", "m", &["Permutation"]),
        spec("mutation", "m", &[])] }).rows;
    assert_eq!(ids.len(), 4);
    assert!(ids[0].error.contains("empty or duplicate operator factory ID"));
    assert!(ids[1].accepted);
    assert!(ids[2].error.contains("empty or duplicate operator factory ID"));
    assert!(ids[3].error.contains("unique nonempty encodings"));
    assert_eq!(ids[3].total_accepted, 1);

    // 三个面各管各的。 / The three faces are independent.
    let faces = ask(OperatorCapabilityRequest { registrations: vec![
        spec("mutation", "x", &["Continuous"]),
        spec("neighborhood", "x", &["Permutation"]),
        spec("update", "x", &["Custom"])] });
    assert!(faces.rows.iter().all(|row| row.accepted));
    assert_eq!(faces.rows[2].total_accepted, 3);

    // 拒绝：未知面、未知编码、四条以上、缺请求。 
    // Refusals: an unknown face, an unknown encoding, the registration cap and a missing request.
    assert!(verify(OperatorCapabilityRequest {
        registrations: vec![spec("nearly", "m", &["Continuous"])] }).unwrap_err()
        .message.contains("face reads update/mutation/neighborhood"));
    assert!(verify(OperatorCapabilityRequest {
        registrations: vec![spec("mutation", "m", &["Nearly"])] }).unwrap_err()
        .message.contains("encoding reads Continuous/Permutation/Custom"));
    assert!(verify(OperatorCapabilityRequest {
        registrations: (0..17).map(|index| spec("mutation", &format!("m{index}"),
            &["Continuous"])).collect() }).unwrap_err().message.contains("sixteen registrations"));
    let mut missing_request = base_request();
    missing_request.model_kind = Some(ModelKind::OperatorCapabilityVerify);
    assert_eq!(engine.submit(missing_request).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        // 载荷故意让能力合法而 id 重复：这一刻"id 那条"与"能力那条"分得开。 
        // The payload keeps the capabilities legal but repeats the id, which tells the two messages apart.
        let payload = serde_json::to_string(&OperatorCapabilityRequest { registrations: vec![
            spec("mutation", "probe", &["Continuous", "Permutation"]),
            spec("mutation", "probe", &["Custom"]),
            spec("mutation", "probe", &[])] }).unwrap();
        std::fs::write(directory.join("operator-capability-probe.json"), payload.as_bytes())
            .unwrap();
    }
}

#[test]
fn schedule_log_entry_shows_the_identity_gate_and_the_ordering_rules() {
    use ospf_rust_bridge::{
        ScheduleEventSpec, ScheduleLogReport, ScheduleLogRequest, ScheduleLogRow, ScheduleLogSpec,
    };

    let engine = NativeEngine::default();
    let verify = |document: ScheduleLogRequest| {
        let mut request = base_request();
        request.model_kind = Some(ModelKind::ScheduleLogVerify);
        request.schedule_log_probe = Some(document);
        engine.submit(request)
    };
    let run = |document: ScheduleLogRequest| {
        let view = verify(document).unwrap();
        let finished = settle(&engine, &view);
        assert_eq!(finished.status, JobStatus::Completed, "{:?}", finished.error);
        let report: ScheduleLogReport =
            serde_json::from_str(finished.report_json.as_deref().unwrap()).unwrap();
        engine.release(&view.id).unwrap();
        report
    };
    let spec = |kind: &str| ScheduleEventSpec { kind: kind.to_owned(), decision: None,
        action: None, evaluations: None, steps: None, random: None, available: None,
        stopped: None, failed: None };
    let dispatch = |decision: f64, action: &str, evaluations: f64, steps: f64| ScheduleEventSpec {
        decision: Some(decision), action: Some(action.to_owned()),
        evaluations: Some(evaluations), steps: Some(steps),
        available: Some(vec![action.to_owned()]), ..spec("dispatch") };
    let complete = |decision: f64, action: &str, evaluations: f64, steps: f64| ScheduleEventSpec {
        decision: Some(decision), action: Some(action.to_owned()),
        evaluations: Some(evaluations), steps: Some(steps), ..spec("complete") };
    let end = |evaluations: f64, steps: f64| ScheduleEventSpec {
        evaluations: Some(evaluations), steps: Some(steps), ..spec("end") };
    // 一份形状完全合法的两派发日志：`end` 要求不在推进，所以收尾前必须有 `yield`。 
    // A perfectly shaped two-dispatch log: the end requires a stopped advance, so a yield comes first.
    let sound = || ScheduleLogRequest { logs: vec![ScheduleLogSpec { version: None,
        configuration: None, problem: None, checksum: None,
        events: vec![spec("begin"), dispatch(0.0, "a", 4.0, 2.0), complete(0.0, "a", 4.0, 2.0),
            dispatch(1.0, "b", 3.0, 1.0), complete(1.0, "b", 3.0, 1.0), spec("yield"),
            end(7.0, 3.0)] }] };
    let ask = |document: ScheduleLogRequest| run(document);

    // 合法日志通过，校验和是 64 位十六进制。 
    // A shaped log passes, and its checksum is sixty-four hex digits.
    let base = ask(sound());
    let row: &ScheduleLogRow = &base.rows[0];
    assert!(row.valid, "{}", row.error);
    assert_eq!(row.events, 7);
    assert_eq!(row.dispatched, 2);
    assert_eq!(row.computed_checksum.len(), 64);
    assert!(row.computed_checksum.chars().all(|character| character.is_ascii_hexdigit()));
    assert!(base.the_identity_gate_comes_first);
    assert!(base.the_begin_is_not_repeatable);
    assert!(base.the_dispatch_preconditions_are_strict);
    assert!(base.the_completion_only_shrinks_its_allownce);
    assert!(base.the_drain_and_yield_need_state);
    assert!(base.the_end_totals_must_reconcile);
    assert!(base.the_log_must_be_sealed);
    assert!(base.the_event_defaults_stay_off_the_wire);

    // 身份门：四种坏法都给同一句话。 / The identity gate: four broken identities, one message.
    let broken = ask(ScheduleLogRequest { logs: vec![
        ScheduleLogSpec { version: Some(2), ..sound().logs[0].clone() },
        ScheduleLogSpec { configuration: Some(String::new()), ..sound().logs[0].clone() },
        ScheduleLogSpec { problem: Some(String::new()), ..sound().logs[0].clone() },
        ScheduleLogSpec { checksum: Some("0".repeat(64)), ..sound().logs[0].clone() }] });
    assert_eq!(broken.rows.len(), 4);
    for row in &broken.rows {
        assert!(!row.valid);
        assert!(row.error.contains("invalid schedule identity, version or checksum"), "{}",
            row.error);
    }

    // 顺序规则各有自己的错误文本。 / Each ordering rule has its own message.
    let violations = ask(ScheduleLogRequest { logs: vec![
        ScheduleLogSpec { events: vec![spec("begin"), spec("begin"), spec("yield"),
            end(0.0, 0.0)], ..sound().logs[0].clone() },
        ScheduleLogSpec { events: vec![spec("begin"), complete(0.0, "a", 4.0, 2.0),
            spec("yield"), end(4.0, 2.0)], ..sound().logs[0].clone() },
        ScheduleLogSpec { events: vec![spec("begin"), dispatch(0.0, "a", 4.0, 2.0),
            complete(0.0, "a", 5.0, 2.0), spec("yield"), end(5.0, 2.0)],
            ..sound().logs[0].clone() },
        ScheduleLogSpec { events: vec![spec("begin"), spec("yield"), end(0.0, 0.0),
            spec("dispatch")], ..sound().logs[0].clone() },
        ScheduleLogSpec { events: vec![spec("begin"), dispatch(0.0, "a", 1.0, 1.0),
            complete(0.0, "a", 1.0, 1.0)], ..sound().logs[0].clone() }] });
    let expectations = ["invalid schedule event ordering", "unexpected schedule completion",
        "inconsistent schedule completion allowances", "schedule contains events after end",
        "truncated or non-quiescent schedule"];
    assert_eq!(violations.rows.len(), expectations.len());
    assert!(violations.rows.iter().all(|row| !row.valid), "{:?}", violations.rows);
    assert!(violations.rows[0].error.contains(expectations[0]));
    assert!(violations.rows[1].error.contains(expectations[1]));
    assert!(violations.rows[2].error.contains(expectations[2]));
    assert!(violations.rows[3].error.contains(expectations[3]));
    assert!(violations.rows[4].error.contains(expectations[4]));

    // 收缩完成与在册 drain 都合法。 / A shrinking completion and a sound drain both pass.
    let sound_edges = ask(ScheduleLogRequest { logs: vec![
        ScheduleLogSpec { events: vec![spec("begin"), dispatch(0.0, "a", 4.0, 2.0),
            complete(0.0, "a", 3.0, 1.0), spec("yield"), end(3.0, 1.0)],
            ..sound().logs[0].clone() },
        ScheduleLogSpec { events: vec![spec("begin"), dispatch(0.0, "a", 1.0, 1.0),
            spec("drain"), complete(0.0, "a", 1.0, 1.0), spec("yield"), end(1.0, 1.0)],
            ..sound().logs[0].clone() }] });
    assert!(sound_edges.rows.iter().all(|row| row.valid), "{:?}", sound_edges.rows);

    // 拒绝：负预算、非整数、超过上限、缺请求。 
    // Refusals: a negative budget, a fractional one, the log cap and a missing request.
    for bad in [dispatch(-1.0, "a", 1.0, 1.0), dispatch(0.5, "a", 1.0, 1.0)] {
        assert!(verify(ScheduleLogRequest { logs: vec![ScheduleLogSpec {
            events: vec![spec("begin"), bad], ..sound().logs[0].clone() }] }).unwrap_err()
            .message.contains("reads a non-negative integer"));
    }
    assert!(verify(ScheduleLogRequest {
        logs: (0..17).map(|index| ScheduleLogSpec { problem: Some(format!("p{index}")),
            ..sound().logs[0].clone() }).collect() }).unwrap_err().message
        .contains("sixteen logs"));
    let mut missing_request = base_request();
    missing_request.model_kind = Some(ModelKind::ScheduleLogVerify);
    assert_eq!(engine.submit(missing_request).unwrap_err().code, "InvalidInput");

    if let Ok(directory) = std::env::var("OSPF_REMOTE_CHECKPOINT_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        // 载荷故意让校验和对、总账错：这一刻"身份门之后才是状态机"被看见。 
        // The payload keeps the checksum right and the totals wrong, which shows the gate really precedes the machine.
        let payload = serde_json::to_string(&ScheduleLogRequest { logs: vec![
            ScheduleLogSpec { events: vec![spec("begin"), dispatch(0.0, "a", 4.0, 2.0),
                complete(0.0, "a", 4.0, 2.0), spec("yield"), end(5.0, 2.0)],
                ..sound().logs[0].clone() },
            ScheduleLogSpec { events: vec![spec("begin"), dispatch(0.0, "a", 1.0, 1.0),
                spec("drain"), complete(0.0, "a", 1.0, 1.0), spec("yield"), end(1.0, 1.0)],
                ..sound().logs[0].clone() }] }).unwrap();
        std::fs::write(directory.join("schedule-log-probe.json"), payload.as_bytes()).unwrap();
    }
}
