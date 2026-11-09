//! 原共享 BPP 模型通过现有认证线性 HTTP 路由执行，不引入领域成功占位入口。
//! Execute original shared BPP models through existing authenticated linear HTTP routes, without placeholder domain endpoints.

#![cfg(any(feature = "scip", feature = "scip-bundled", feature = "scip-from-source", feature = "gurobi10", feature = "gurobi11", feature = "gurobi12"))]

use std::time::{Duration, Instant};
use axum::{Router, body::{Body, to_bytes}, http::{Method, Request, StatusCode}};
use ospf_rust_bridge::*;
use ospf_rust_host::{HostState, router};
use ospf_rust_native::NativeEngine;
use tower::ServiceExt;

const TOKEN: &str = "shared-bpp-in-process-http-integration-token";

#[tokio::test]
async fn authenticated_partition_application_replays_original_children_and_preserves_proof_free_source_plans() {
    let mut backends = Vec::new();
    if cfg!(any(feature = "scip", feature = "scip-bundled", feature = "scip-from-source")) { backends.push(NativeBackend::Scip); }
    if cfg!(any(feature = "gurobi10", feature = "gurobi11", feature = "gurobi12")) { backends.push(NativeBackend::Gurobi); }
    for (backend, finishing) in backends.into_iter().flat_map(|backend| [false, true].map(move |finishing| (backend, finishing))) {
        let app = router(HostState { engine: NativeEngine::new(1), token: TOKEN.into() }, vec![]).unwrap();
        let problem = serde_json::from_str(r#"{"items":[{"id":"cargo","weight":{"value":1,"unit":"kg"},"required":true,"fixedPositionId":null}],"positions":[{"id":"p","maxCount":"1","maxWeight":{"value":2,"unit":"kg"},"fixedOccupied":false,"enabled":true}],"allowedAssignments":[{"itemId":"cargo","positionId":"p"}],"conflicts":[],"layoutGroups":[],"weightUnit":"kg"}"#).unwrap();
        let source = BppLoadingContext::new(problem).unwrap().artifact();
        let mut partition = serde_json::json!({"partition":{"partitions":[{"id":"whole","positionIds":["p"]}]}});
        if finishing { partition["sourceMilpFinishing"] = serde_json::json!({"reserveTimeMs":1000}); }
        let request = serde_json::json!({"protocolVersion":1,"solver":"bpp-loading","modelKind":"bppLoading","model":source,"timeLimitMs":10000,
            "bppLoadingOptions":{"backend":backend,"partition":partition}}).to_string();
        let (status, bytes) = call(&app, Method::POST, "/ospf/v1/jobs", request).await; assert_eq!(status, StatusCode::ACCEPTED);
        let job: JobView = serde_json::from_slice(&bytes).unwrap(); let deadline = Instant::now() + Duration::from_secs(15);
        let view = loop {
            let (status, bytes) = call(&app, Method::GET, &format!("/ospf/v1/jobs/{}", job.id), String::new()).await; assert_eq!(status, StatusCode::OK);
            let view: JobView = serde_json::from_slice(&bytes).unwrap(); if view.status != JobStatus::Running { break view; }
            assert!(Instant::now() < deadline); tokio::task::yield_now().await;
        };
        assert_eq!(view.status, JobStatus::Completed, "{:?}", view.error); assert!(view.progress_json.is_some());
        let report = decode_bpp_loading_partition(view.report_json.as_ref().unwrap(), &source).unwrap(); assert_eq!(report.application.solution.unwrap().loaded_count, "1");
        let core: serde_json::Value = serde_json::from_str(&report.application.envelope.report_json).unwrap(); assert_eq!(core["proof"].is_null(), !finishing);
        if finishing {
            let evidence = report.envelope.certification.unwrap(); assert!(evidence.report_json.is_some());
            let previous = decode_bpp_loading_application(&evidence.previous_application_report_json, &source).unwrap();
            let core: serde_json::Value = serde_json::from_str(&previous.envelope.report_json).unwrap(); assert!(core["proof"].is_null());
        }
        let (_, bytes) = call(&app, Method::POST, &format!("/ospf/v1/jobs/{}/cancel", job.id), String::new()).await;
        assert_eq!(serde_json::from_slice::<JobView>(&bytes).unwrap().report_json, view.report_json);
        assert_eq!(call(&app, Method::DELETE, &format!("/ospf/v1/jobs/{}", job.id), String::new()).await.0, StatusCode::NO_CONTENT);
    }
}

#[tokio::test]
async fn authenticated_compiled_partition_lp_keeps_original_identity_and_verified_pricing_duals() {
    let mut backends = Vec::new();
    if cfg!(any(feature = "scip", feature = "scip-bundled", feature = "scip-from-source")) { backends.push(NativeBackend::Scip); }
    if cfg!(any(feature = "gurobi10", feature = "gurobi11", feature = "gurobi12")) { backends.push(NativeBackend::Gurobi); }
    for backend in backends {
        let app = router(HostState { engine: NativeEngine::new(1), token: TOKEN.into() }, vec![]).unwrap();
        let problem = serde_json::from_str(r#"{"items":[{"id":"a","weight":{"value":1,"unit":"kg"},"required":true,"fixedPositionId":null}],"positions":[{"id":"p","maxCount":"1","maxWeight":{"value":2,"unit":"kg"},"fixedOccupied":false,"enabled":true}],"allowedAssignments":[{"itemId":"a","positionId":"p"}],"conflicts":[],"layoutGroups":[],"weightUnit":"kg"}"#).unwrap();
        let source = BppLoadingContext::new(problem).unwrap().artifact();
        let options = serde_json::from_str(r#"{"partitions":[{"id":"whole","positionIds":["p"]}]}"#).unwrap();
        let mut context = BppLoadingPartitionContext::new(&source, options).unwrap();
        let decisions = context.decisions().iter().map(|decision| (decision.id.clone(), 1.0)).collect();
        context.add_columns(vec![BppLoadingPartitionColumnInput { partition_id: "whole".into(), decisions }]).unwrap();
        let master = context.master(Default::default()).unwrap(); let model = master.artifact();
        let request = serde_json::json!({"protocolVersion":1,"solver":backend.id(),"modelKind":"compiledLinear","model":model,"timeLimitMs":10000}).to_string();
        let (status, bytes) = call(&app, Method::POST, "/ospf/v1/jobs", request).await; assert_eq!(status, StatusCode::ACCEPTED);
        let job: JobView = serde_json::from_slice(&bytes).unwrap(); let deadline = Instant::now() + Duration::from_secs(15);
        let view = loop {
            let (_, bytes) = call(&app, Method::GET, &format!("/ospf/v1/jobs/{}", job.id), String::new()).await;
            let view: JobView = serde_json::from_slice(&bytes).unwrap(); if view.status != JobStatus::Running { break view; }
            assert!(Instant::now() < deadline); tokio::task::yield_now().await;
        };
        assert_eq!(view.status, JobStatus::Completed, "{:?}", view.error); assert_eq!(view.fingerprint, model.fingerprint);
        context.accept_lp_report(&master, view.report_json.as_ref().unwrap()).unwrap(); context.pricing(&master, "whole", false).unwrap();
        assert_eq!(call(&app, Method::DELETE, &format!("/ospf/v1/jobs/{}", job.id), String::new()).await.0, StatusCode::NO_CONTENT);
    }
}

#[tokio::test]
async fn authenticated_benders_routes_keep_real_resource_recourse_and_conservative_source_reports() {
    let mut backends = Vec::new();
    if cfg!(any(feature = "scip", feature = "scip-bundled", feature = "scip-from-source")) { backends.push(NativeBackend::Scip); }
    if cfg!(any(feature = "gurobi10", feature = "gurobi11", feature = "gurobi12")) { backends.push(NativeBackend::Gurobi); }
    for (backend, finishing) in backends.into_iter().flat_map(|backend| [false, true].map(move |finishing| (backend, finishing))) {
        let app = router(HostState { engine: NativeEngine::new(1), token: TOKEN.into() }, vec![]).unwrap();
        let problem = serde_json::from_str(r#"{"items":[{"id":"a","weight":{"value":1,"unit":"kg"},"required":false,"fixedPositionId":null}],"positions":[{"id":"p","maxCount":"1","maxWeight":{"value":2,"unit":"kg"},"fixedOccupied":false,"enabled":true}],"allowedAssignments":[{"itemId":"a","positionId":"p"}],"conflicts":[],"layoutGroups":[],"weightUnit":"kg"}"#).unwrap();
        let objective = serde_json::from_str(r#"{"itemCosts":{"a":-5},"positionCosts":{"p":1},"layoutCosts":[]}"#).unwrap();
        let resource = serde_json::from_str(r#"{"id":"ballast","positionId":"p","minimumWeight":{"value":0,"unit":"kg"},"maximumWeight":{"value":1000,"unit":"g"},"minimumPerLoadedWeight":0.5,"costPerWeight":2}"#).unwrap();
        let source = BppLoadingContext::with_resources(problem, Some(objective), vec![resource]).unwrap().artifact();
        let mut benders = serde_json::json!({"recourseResources":["ballast"]});
        if finishing { benders["sourceMilpFinishing"] = serde_json::json!({"reserveTimeMs":3000}); }
        let request = serde_json::json!({"protocolVersion":1,"solver":"bpp-loading","modelKind":"bppLoading","model":source,
            "timeLimitMs":10_000,"bppLoadingOptions":{"backend":backend,"benders":benders}}).to_string();
        let (status, bytes) = call(&app, Method::POST, "/ospf/v1/jobs", request).await; assert_eq!(status, StatusCode::ACCEPTED);
        let job: JobView = serde_json::from_slice(&bytes).unwrap(); let deadline = Instant::now() + Duration::from_secs(15);
        let view = loop {
            let (_, bytes) = call(&app, Method::GET, &format!("/ospf/v1/jobs/{}", job.id), String::new()).await;
            let view: JobView = serde_json::from_slice(&bytes).unwrap(); if view.status != JobStatus::Running { break view; }
            assert!(Instant::now() < deadline); tokio::task::yield_now().await;
        };
        assert_eq!(view.status, JobStatus::Completed, "{:?}", view.error); assert!(view.progress_json.is_some());
        let report = decode_bpp_loading_benders(view.report_json.as_ref().unwrap(), &source).unwrap();
        let decomposition = report.decomposition.unwrap(); assert_eq!(decomposition.summary.stop, FixedRecourseStop::Converged); assert!(decomposition.summary.optimality_cuts > 0);
        assert!((report.application.envelope.objective.unwrap() + 3.0).abs() < 1e-7);
        let source_report: serde_json::Value = serde_json::from_str(&report.application.envelope.report_json).unwrap(); assert_eq!(source_report["proof"].is_null(), !finishing);
        if finishing { assert!(report.envelope.certification.unwrap().report_json.is_some()); }
        let (_, bytes) = call(&app, Method::POST, &format!("/ospf/v1/jobs/{}/cancel", job.id), String::new()).await;
        assert_eq!(serde_json::from_slice::<JobView>(&bytes).unwrap().report_json, view.report_json);
        assert_eq!(call(&app, Method::DELETE, &format!("/ospf/v1/jobs/{}", job.id), String::new()).await.0, StatusCode::NO_CONTENT);
    }
}

async fn call(app: &Router, method: Method, path: &str, body: String) -> (StatusCode, Vec<u8>) {
    let response = app.clone().oneshot(Request::builder().method(method).uri(path).header("Authorization", format!("Bearer {TOKEN}"))
        .header("Content-Type", "application/json").body(Body::from(body)).unwrap()).await.unwrap();
    (response.status(), to_bytes(response.into_body(), 9 * 1024 * 1024).await.unwrap().to_vec())
}

#[tokio::test]
async fn authenticated_application_routes_execute_business_objectives_and_retain_original_reports() {
    let mut backends = Vec::new();
    if cfg!(any(feature = "scip", feature = "scip-bundled", feature = "scip-from-source")) { backends.push(NativeBackend::Scip); }
    if cfg!(any(feature = "gurobi10", feature = "gurobi11", feature = "gurobi12")) { backends.push(NativeBackend::Gurobi); }
    for backend in backends {
        let app = router(HostState { engine: NativeEngine::new(1), token: TOKEN.into() }, vec![]).unwrap();
        let problem: BppLoadingProblem = serde_json::from_str(r#"{"items":[{"id":"a","weight":{"value":1,"unit":"kg"},"required":false,"fixedPositionId":null}],"positions":[{"id":"p","maxCount":"18446744073709551615","maxWeight":{"value":2,"unit":"kg"},"fixedOccupied":false,"enabled":true}],"allowedAssignments":[{"itemId":"a","positionId":"p"}],"conflicts":[],"layoutGroups":[],"weightUnit":"kg"}"#).unwrap();
        let objective = serde_json::from_str(r#"{"itemCosts":{"a":-5},"positionCosts":{"p":1},"layoutCosts":[]}"#).unwrap();
        let resource = serde_json::from_str(r#"{"id":"ballast","positionId":"p","minimumWeight":{"value":0,"unit":"kg"},"maximumWeight":{"value":1000,"unit":"g"},"minimumPerLoadedWeight":0.5,"costPerWeight":2}"#).unwrap();
        let source = BppLoadingContext::with_resources(problem, Some(objective), vec![resource]).unwrap().artifact();
        let request = serde_json::json!({"protocolVersion":1,"solver":"bpp-loading","modelKind":"bppLoading","model":source,
            "timeLimitMs":10_000,"bppLoadingOptions":{"backend":backend}}).to_string();
        let (status, bytes) = call(&app, Method::POST, "/ospf/v1/jobs", request).await; assert_eq!(status, StatusCode::ACCEPTED);
        let job: JobView = serde_json::from_slice(&bytes).unwrap(); let deadline = Instant::now() + Duration::from_secs(15);
        let view = loop {
            let (status, bytes) = call(&app, Method::GET, &format!("/ospf/v1/jobs/{}", job.id), String::new()).await; assert_eq!(status, StatusCode::OK);
            let view: JobView = serde_json::from_slice(&bytes).unwrap(); if view.status != JobStatus::Running { break view; }
            assert!(Instant::now() < deadline); tokio::task::yield_now().await;
        };
        assert_eq!(view.status, JobStatus::Completed, "{:?}", view.error); assert!(view.progress_json.is_some());
        let report = decode_bpp_loading_application(view.report_json.as_ref().unwrap(), &source).unwrap();
        assert!((report.envelope.objective.unwrap() + 3.0).abs() < 1e-7);
        let solution = report.solution.unwrap(); assert_eq!(solution.loaded_count, "1"); assert!((solution.resources.unwrap().total_weight.value - 1.5).abs() < 1e-7);
        let (_, bytes) = call(&app, Method::POST, &format!("/ospf/v1/jobs/{}/cancel", job.id), String::new()).await;
        assert_eq!(serde_json::from_slice::<JobView>(&bytes).unwrap().report_json, view.report_json);
        assert_eq!(call(&app, Method::DELETE, &format!("/ospf/v1/jobs/{}", job.id), String::new()).await.0, StatusCode::NO_CONTENT);
    }
}

#[tokio::test]
async fn authenticated_linear_routes_preserve_fixed_bpp_models_feasibility_and_cancellation() {
    let mut backends = Vec::new();
    if cfg!(any(feature = "scip", feature = "scip-bundled", feature = "scip-from-source")) { backends.push(NativeBackend::Scip); }
    if cfg!(any(feature = "gurobi10", feature = "gurobi11", feature = "gurobi12")) { backends.push(NativeBackend::Gurobi); }
    for backend in backends {
        let app = router(HostState { engine: NativeEngine::new(1), token: TOKEN.into() }, vec![]).unwrap();
        for variant in 0..3 {
            let mut problem: BppLoadingProblem = serde_json::from_str(r#"{"items":[{"id":"a","weight":{"value":1000,"unit":"g"},"required":true,"fixedPositionId":"p"}],"positions":[{"id":"p","maxCount":"18446744073709551615","maxWeight":{"value":1,"unit":"kg"},"fixedOccupied":false,"enabled":false}],"allowedAssignments":[{"itemId":"a","positionId":"p"}],"conflicts":[],"layoutGroups":[],"weightUnit":"kg"}"#).unwrap();
            if variant == 1 { problem.items[0].fixed_position_id = None; problem.allowed_assignments.clear(); }
            let context = BppLoadingContext::new(problem).unwrap();
            let request = SolveRequest { schedule_log_probe: None, operator_capability_probe: None, hypervolume_probe: None, acceptance_recovery_probe: None, training_split_probe: None, non_stationary_probe: None, selection_policy_probe: None, solve_value_probe: None, native_lowering_probe: None, native_terminal_proof_probe: None, predicate_scope_probe: None, persistence_schema_probe: None, persistence_sort_probe: None, effectiveness_ranking_probe: None, funnel_ranking: None, sensitivity_probe: None, correction_probe: None, blocking_set: None, fingerprint_probe: None, diagnostics_probe: None, gap_probe: None, proof_probe: None, evidence_probe: None, band_probe: None, group_summary: None, critical_report: None, effectiveness_probe: None, funnel: None, solution_status: None, diversity: None, piecewise: None, archive_probe: None, migration: None, topology: None, ranking_probe: None, acceptance: None, boundary: None, population_config: None, constraint_priority: None, constraint_ranking: None, pareto_archive: None, reference_points: None, dispersion: None, dominance: None, evidence_remap: None, tier_selection: None, activation_set: None, container_surface: None, identity_wrap: None, domain_surface: None, expression_bounds: None, assignment_eval: None, constraint_canon: None, snapshot_gate: None, cp_model_build: None, no_good_oracle: None, subproblem_result: None, cp_support_merge: None, capability_matrix: None, analysis_options: None, model_view: None, polynomial_probe: None, matrix_rows: None, sparse_triad: None, master_assignment: None, stable_value: None, quadratic_elastic: None, dump_batch: None, lazy_flatten_init: None, candidate_shortlist: None, writer_registry: None, native_write_scope: None, dump_options: None, assignment_composition: None, cut_validity: None, source_identity: None, binding_assignment: None, auxiliary_domain: None, activity_report_check: None, evidence_predicates: None, activity_bands: None, encoding_label: None, expression_key: None, canonical_monomial: None, backend_features: None, deferred_materialize: None, quadratic_monomial: None, polarity_encoding: None, tier_classification: None, assumption_binding: None, effectiveness_aggregate: None, bounded_no_good: None, polynomial_lift: None, scalar_function: None, field_resolution: None, boolean_fold: None, master_cut: None, flatten_identity: None, critical_assembly: None, variable_kind_domain: None, sort_by: None, predicate_schema: None, search_budget: None, conditional_semantics: None, big_m_policy: None, number_text: None, combine_status: None, elastic_model: None, native_write: None, input_domain: None, function_usage: None, repository_call: None, unsupported_predicate: None, boolean_scope: None, update_assignment: None, flatten_cache: None, cache_key_address: None, cache_key_identity: None, model_configuration: None, predicate_field_filter: None, predicate_field_resolution: None, analysis_group_aggregate: None, master_cut_encoding: None, q_table: None, heuristic_schedule: None, symbol_core_relations: None, symbol_linearization: None, symbol_condition: None, benders_cut_request: None, benders_metrics: None, cp_checkpoint_request: None, lbb_resume_identity: None, linear_mapping_request: None, checkpoint_request: None, reward_policy: None, hyper_heuristic_config: None, criticality_request: None, multi_target_report: None, relaxation_policy: None, target_spec: None, target_report: None, perturbation_policy: None, fixed_integer_report: None, fixed_integer_config: None, relaxation_request: None, critical_analysis: None, effectiveness_ranking: None, shortlist_limit: None, candidate_funnel: None, conflict_explanation: None, remote_artifact: None, logic_based_benders_plan: None, master_solution: None, benders_quadratic_cuts: None, pipeline_steps: None, repository_query: None, assignments: None, sort_plan: None, predicate: None, benders_cuts: None, relational_query_plan: None, linear_lifecycle: None,  indexed_registration: None, column_batch: None, column_scope: None, heart_beat: None, response_envelope: None, quadratic_orchestration: None, vrp_problem: None, network_resilience_problem: None, allocation_problem: None, network_structure_problem: None, network_design_problem: None, network_generalized_flow_problem: None, network_flow_problem: None, sweep_cases: None,  incumbent_values: None, solution_pool_options: None, gantt_activity_options: None, iis_options: None, bpp3d_options: None, protocol_version: 1, solver: backend.id().into(), model: context.linear_model(), model_kind: Some(ModelKind::Linear), time_limit_ms: Some(10_000),
                tuning_options: None,
                cp_options: None, serial_options: None, parallel_options: None, fixed_recourse_options: None, column_generation_options: None,
                bpp1d_options: None, csp1d_options: None, bpp2d_options: None, bpp_loading_options: None, csp2d_options: None };
            let (status, bytes) = call(&app, Method::POST, "/ospf/v1/jobs", serde_json::to_string(&request).unwrap()).await;
            assert_eq!(status, StatusCode::ACCEPTED, "{}", String::from_utf8_lossy(&bytes));
            let submitted: JobView = serde_json::from_slice(&bytes).unwrap();
            if variant == 2 { assert_eq!(call(&app, Method::POST, &format!("/ospf/v1/jobs/{}/cancel", submitted.id), String::new()).await.0, StatusCode::OK); }
            let deadline = Instant::now() + Duration::from_secs(15);
            let view = loop {
                let (status, bytes) = call(&app, Method::GET, &format!("/ospf/v1/jobs/{}", submitted.id), String::new()).await;
                assert_eq!(status, StatusCode::OK);
                let view: JobView = serde_json::from_slice(&bytes).unwrap(); if view.status != JobStatus::Running { break view; }
                assert!(Instant::now() < deadline); tokio::task::yield_now().await;
            };
            assert_eq!(view.status, JobStatus::Completed, "{:?}", view.error);
            let report = context.read_report(view.report_json.as_ref().unwrap()).unwrap();
            if variant == 0 { assert_eq!(report.solution.as_ref().unwrap().loaded_count, "1"); assert!(view.progress_json.is_some()); }
            if variant == 1 { assert!(report.solution.is_none()); assert_eq!(report.summary.problem_status, "INFEASIBLE"); }
            let (_, bytes) = call(&app, Method::POST, &format!("/ospf/v1/jobs/{}/cancel", submitted.id), String::new()).await;
            assert_eq!(serde_json::from_slice::<JobView>(&bytes).unwrap().report_json, view.report_json);
            assert_eq!(call(&app, Method::DELETE, &format!("/ospf/v1/jobs/{}", submitted.id), String::new()).await.0, StatusCode::NO_CONTENT);
        }
    }
}
