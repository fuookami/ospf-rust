//! 通过真实回环 TCP 和 SCIP 验收宿主合同。 / Verify the host contract over real loopback TCP and SCIP.

#![cfg(any(feature = "scip", feature = "scip-bundled", feature = "scip-from-source"))]

use std::time::Duration;
use axum::http::{HeaderValue, StatusCode};
use ospf_rust_bridge::{HostCapabilities, JobStatus, JobView, ModelKind, SolveRequest, encode_model, summarize_report};
use ospf_rust_core::model::{MetaModel, ObjectiveCategory};
use ospf_rust_core::variable::{Continuous, VariableRange};
use ospf_rust_host::{HostState, router};
use ospf_rust_native::NativeEngine;
use tokio::sync::oneshot;

const TOKEN: &str = "tcp-loopback-scip-integration-test-token";
const ORIGIN: &str = "http://localhost:3000";

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn authenticated_tcp_solve_cancel_progress_and_shutdown() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}/ospf/v1", listener.local_addr().unwrap());
    let app = router(HostState { engine: NativeEngine::new(1), token: TOKEN.into() }, vec![HeaderValue::from_static(ORIGIN)]).unwrap();
    let (shutdown, stopped) = oneshot::channel::<()>();
    let server = tokio::spawn(async move {
        axum::serve(listener, app).with_graceful_shutdown(async { let _ = stopped.await; }).await.unwrap();
    });
    let client = reqwest::Client::builder().no_proxy().pool_max_idle_per_host(0).timeout(Duration::from_secs(5)).build().unwrap();
    assert_eq!(client.get(format!("{base}/capabilities")).send().await.unwrap().status(), StatusCode::UNAUTHORIZED);
    let preflight = client.request(reqwest::Method::OPTIONS, format!("{base}/jobs"))
        .header("Origin", ORIGIN).header("Access-Control-Request-Method", "POST")
        .header("Access-Control-Request-Headers", "authorization,content-type").send().await.unwrap();
    assert_eq!(preflight.headers().get("access-control-allow-origin").unwrap(), ORIGIN);
    let capabilities: HostCapabilities = client.get(format!("{base}/capabilities")).bearer_auth(TOKEN).send().await.unwrap().json().await.unwrap();
    assert!(capabilities.backends.iter().any(|backend| backend.id == "scip" && backend.compiled));

    let mut source = MetaModel::<f64>::new("tcp-scip");
    source.register_auto_variable_with_range::<Continuous>("x", VariableRange::bounded(0.0, 4.0)).unwrap();
    source.set_linear_objective(vec![1.0], ObjectiveCategory::Maximum);
    let model = encode_model(&source.try_to_linear_triad_model().unwrap()).unwrap();
    for cancel_early in [false, true] {
        let request = SolveRequest { schedule_log_probe: None, operator_capability_probe: None, hypervolume_probe: None, acceptance_recovery_probe: None, training_split_probe: None, non_stationary_probe: None, selection_policy_probe: None, solve_value_probe: None, native_lowering_probe: None, native_terminal_proof_probe: None, predicate_scope_probe: None, persistence_schema_probe: None, persistence_sort_probe: None, effectiveness_ranking_probe: None, funnel_ranking: None, sensitivity_probe: None, correction_probe: None, blocking_set: None, fingerprint_probe: None, diagnostics_probe: None, gap_probe: None, proof_probe: None, evidence_probe: None, band_probe: None, group_summary: None, critical_report: None, effectiveness_probe: None, funnel: None, solution_status: None, diversity: None, piecewise: None, archive_probe: None, migration: None, topology: None, ranking_probe: None, acceptance: None, boundary: None, population_config: None, constraint_priority: None, constraint_ranking: None, pareto_archive: None, reference_points: None, dispersion: None, dominance: None, evidence_remap: None, tier_selection: None, activation_set: None, container_surface: None, identity_wrap: None, domain_surface: None, expression_bounds: None, assignment_eval: None, constraint_canon: None, snapshot_gate: None, cp_model_build: None, no_good_oracle: None, subproblem_result: None, cp_support_merge: None, capability_matrix: None, analysis_options: None, model_view: None, polynomial_probe: None, matrix_rows: None, sparse_triad: None, master_assignment: None, stable_value: None, quadratic_elastic: None, dump_batch: None, lazy_flatten_init: None, candidate_shortlist: None, writer_registry: None, native_write_scope: None, dump_options: None, assignment_composition: None, cut_validity: None, source_identity: None, binding_assignment: None, auxiliary_domain: None, activity_report_check: None, evidence_predicates: None, activity_bands: None, encoding_label: None, expression_key: None, canonical_monomial: None, backend_features: None, deferred_materialize: None, quadratic_monomial: None, polarity_encoding: None, tier_classification: None, assumption_binding: None, effectiveness_aggregate: None, bounded_no_good: None, polynomial_lift: None, scalar_function: None, field_resolution: None, boolean_fold: None, master_cut: None, flatten_identity: None, critical_assembly: None, variable_kind_domain: None, sort_by: None, predicate_schema: None, search_budget: None, conditional_semantics: None, big_m_policy: None, number_text: None, combine_status: None, elastic_model: None, native_write: None, input_domain: None, function_usage: None, repository_call: None, unsupported_predicate: None, boolean_scope: None, update_assignment: None, flatten_cache: None, cache_key_address: None, cache_key_identity: None, model_configuration: None, predicate_field_filter: None, predicate_field_resolution: None, analysis_group_aggregate: None, master_cut_encoding: None, q_table: None, heuristic_schedule: None, symbol_core_relations: None, symbol_linearization: None, symbol_condition: None, benders_cut_request: None, benders_metrics: None, cp_checkpoint_request: None, lbb_resume_identity: None, linear_mapping_request: None, checkpoint_request: None, reward_policy: None, hyper_heuristic_config: None, criticality_request: None, multi_target_report: None, relaxation_policy: None, target_spec: None, target_report: None, perturbation_policy: None, fixed_integer_report: None, fixed_integer_config: None, relaxation_request: None, critical_analysis: None, effectiveness_ranking: None, shortlist_limit: None, candidate_funnel: None, conflict_explanation: None, remote_artifact: None, logic_based_benders_plan: None, master_solution: None, benders_quadratic_cuts: None, pipeline_steps: None, repository_query: None, assignments: None, sort_plan: None, predicate: None, benders_cuts: None, relational_query_plan: None, linear_lifecycle: None,  indexed_registration: None, column_batch: None, column_scope: None, heart_beat: None, response_envelope: None, quadratic_orchestration: None, vrp_problem: None, network_resilience_problem: None, allocation_problem: None, network_structure_problem: None, network_design_problem: None, network_generalized_flow_problem: None, network_flow_problem: None, sweep_cases: None,  incumbent_values: None, solution_pool_options: None, gantt_activity_options: None, iis_options: None, bpp3d_options: None,
            tuning_options: None,
            protocol_version: 1, solver: "scip".into(), model: model.clone(), model_kind: Some(ModelKind::Linear), time_limit_ms: Some(10_000),
            cp_options: None, serial_options: None, parallel_options: None, fixed_recourse_options: None, column_generation_options: None,
            bpp1d_options: None, csp1d_options: None, bpp2d_options: None, bpp_loading_options: None, csp2d_options: None,
        };
        let response = client.post(format!("{base}/jobs")).bearer_auth(TOKEN).json(&request).send().await.unwrap();
        assert_eq!(response.status(), StatusCode::ACCEPTED);
        let submitted: JobView = response.json().await.unwrap();
        assert_eq!(submitted.status, JobStatus::Running);
        let job = format!("{base}/jobs/{}", submitted.id);
        if cancel_early {
            assert_eq!(client.post(format!("{job}/cancel")).bearer_auth(TOKEN).send().await.unwrap().status(), StatusCode::OK);
        }
        let terminal = tokio::time::timeout(Duration::from_secs(15), async {
            loop {
                let view: JobView = client.get(&job).bearer_auth(TOKEN).send().await.unwrap().json().await.unwrap();
                if view.status != JobStatus::Running { break view; }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        }).await.unwrap();
        assert_eq!(terminal.status, JobStatus::Completed, "{:?}", terminal.error);
        assert_eq!(terminal.fingerprint, model.fingerprint);
        let report = summarize_report(terminal.report_json.as_ref().unwrap()).unwrap();
        if cancel_early {
            assert!(["COMPLETED", "CANCELLED", "INTERRUPTED"].contains(&report.termination_reason.as_str()));
        } else {
            assert_eq!(report.objective_value, Some(4.0));
            assert_eq!(report.values, vec![4.0]);
        }
        let progress: serde_json::Value = serde_json::from_str(terminal.progress_json.as_ref().unwrap()).unwrap();
        assert_eq!(progress["terminal"], true);
        let late: JobView = client.post(format!("{job}/cancel")).bearer_auth(TOKEN).send().await.unwrap().json().await.unwrap();
        assert_eq!(late.report_json, terminal.report_json);
        let result: JobView = client.get(format!("{job}/result")).bearer_auth(TOKEN).send().await.unwrap().json().await.unwrap();
        assert_eq!(result.report_json, terminal.report_json);
        assert_eq!(client.delete(&job).bearer_auth(TOKEN).send().await.unwrap().status(), StatusCode::NO_CONTENT);
        assert_eq!(client.get(&job).bearer_auth(TOKEN).send().await.unwrap().status(), StatusCode::NOT_FOUND);
    }
    shutdown.send(()).unwrap();
    tokio::time::timeout(Duration::from_secs(5), server).await.unwrap().unwrap();
    assert!(client.get(format!("{base}/capabilities")).bearer_auth(TOKEN).send().await.is_err());
}
