//! 共享 BPP 原模型经现有原生线性入口的真实求解，不声明完整领域 application 已绑定。
//! Real solves of original shared BPP models through the existing native linear entry, without claiming full domain-application bindings.

#![cfg(any(feature = "scip", feature = "gurobi10", feature = "gurobi11", feature = "gurobi12"))]

use std::time::{Duration, Instant};
use ospf_rust_bridge::*;
use ospf_rust_native::NativeEngine;

fn problem() -> BppLoadingProblem {
    serde_json::from_str(r#"{"items":[{"id":"a","weight":{"value":4000,"unit":"g"},"required":true,"fixedPositionId":null},{"id":"b","weight":{"value":4,"unit":"kg"},"required":true,"fixedPositionId":null}],"positions":[{"id":"p","maxCount":"18446744073709551615","maxWeight":{"value":5,"unit":"kg"},"fixedOccupied":false,"enabled":true},{"id":"q","maxCount":"1","maxWeight":{"value":6,"unit":"kg"},"fixedOccupied":false,"enabled":true},{"id":"r","maxCount":"1","maxWeight":{"value":5,"unit":"kg"},"fixedOccupied":false,"enabled":true}],"allowedAssignments":[{"itemId":"a","positionId":"p"},{"itemId":"a","positionId":"q"},{"itemId":"b","positionId":"r"}],"conflicts":[{"first":"p","second":"q"},{"first":"q","second":"r"}],"layoutGroups":[],"weightUnit":"kg"}"#).unwrap()
}

fn contract(backend: NativeBackend) {
    let engine = NativeEngine::new(1); let mut fixtures = Vec::new();
    for variant in 0..8 {
        let mut problem = problem();
        match variant {
            1 => { problem.items[0].fixed_position_id = Some("p".into()); problem.positions[0].enabled = false; },
            2 => problem.allowed_assignments.retain(|key| key.item_id != "a"),
            3 => problem.layout_groups = vec![BppLoadingLayoutGroup { id: "g".into(), position_ids: vec!["p".into(), "q".into(), "r".into()],
                layouts: vec![BppLoadingLayout { id: "outer".into(), position_ids: vec!["p".into(), "r".into()] }, BppLoadingLayout { id: "middle".into(), position_ids: vec!["q".into()] }] }],
            4 => { problem.items.clear(); problem.allowed_assignments.clear(); problem.positions[0].fixed_occupied = true; },
            6 => { problem.positions[0].max_weight.value = 1.0; problem.positions[1].max_weight.value = 1.0; },
            7 => problem.weight_unit = QuantityUnit::Gram,
            _ => {},
        }
        let context = BppLoadingContext::new(problem).unwrap(); let source = context.artifact(); let model = context.linear_model();
        let request = SolveRequest { schedule_log_probe: None, operator_capability_probe: None, hypervolume_probe: None, acceptance_recovery_probe: None, training_split_probe: None, non_stationary_probe: None, selection_policy_probe: None, solve_value_probe: None, native_lowering_probe: None, native_terminal_proof_probe: None, predicate_scope_probe: None, persistence_schema_probe: None, persistence_sort_probe: None, effectiveness_ranking_probe: None, funnel_ranking: None, sensitivity_probe: None, correction_probe: None, blocking_set: None, fingerprint_probe: None, diagnostics_probe: None, gap_probe: None, proof_probe: None, evidence_probe: None, band_probe: None, group_summary: None, critical_report: None, effectiveness_probe: None, funnel: None, solution_status: None, diversity: None, piecewise: None, archive_probe: None, migration: None, topology: None, ranking_probe: None, acceptance: None, boundary: None, population_config: None, constraint_priority: None, constraint_ranking: None, pareto_archive: None, reference_points: None, dispersion: None, dominance: None, evidence_remap: None, tier_selection: None, activation_set: None, container_surface: None, identity_wrap: None, domain_surface: None, expression_bounds: None, assignment_eval: None, constraint_canon: None, snapshot_gate: None, cp_model_build: None, no_good_oracle: None, subproblem_result: None, cp_support_merge: None, capability_matrix: None, analysis_options: None, model_view: None, polynomial_probe: None, matrix_rows: None, sparse_triad: None, master_assignment: None, stable_value: None, quadratic_elastic: None, dump_batch: None, lazy_flatten_init: None, candidate_shortlist: None, writer_registry: None, native_write_scope: None, dump_options: None, assignment_composition: None, cut_validity: None, source_identity: None, binding_assignment: None, auxiliary_domain: None, activity_report_check: None, evidence_predicates: None, activity_bands: None, encoding_label: None, expression_key: None, canonical_monomial: None, backend_features: None, deferred_materialize: None, quadratic_monomial: None, polarity_encoding: None, tier_classification: None, assumption_binding: None, effectiveness_aggregate: None, bounded_no_good: None, polynomial_lift: None, scalar_function: None, field_resolution: None, boolean_fold: None, master_cut: None, flatten_identity: None, critical_assembly: None, variable_kind_domain: None, sort_by: None, predicate_schema: None, search_budget: None, conditional_semantics: None, big_m_policy: None, number_text: None, combine_status: None, elastic_model: None, native_write: None, input_domain: None, function_usage: None, repository_call: None, unsupported_predicate: None, boolean_scope: None, update_assignment: None, flatten_cache: None, cache_key_address: None, cache_key_identity: None, model_configuration: None, predicate_field_filter: None, predicate_field_resolution: None, analysis_group_aggregate: None, master_cut_encoding: None, q_table: None, heuristic_schedule: None, symbol_core_relations: None, symbol_linearization: None, symbol_condition: None, benders_cut_request: None, benders_metrics: None, cp_checkpoint_request: None, lbb_resume_identity: None, linear_mapping_request: None, checkpoint_request: None, reward_policy: None, hyper_heuristic_config: None, criticality_request: None, multi_target_report: None, relaxation_policy: None, target_spec: None, target_report: None, perturbation_policy: None, fixed_integer_report: None, fixed_integer_config: None, relaxation_request: None, critical_analysis: None, effectiveness_ranking: None, shortlist_limit: None, candidate_funnel: None, conflict_explanation: None, remote_artifact: None, logic_based_benders_plan: None, master_solution: None, benders_quadratic_cuts: None, pipeline_steps: None, repository_query: None, assignments: None, sort_plan: None, predicate: None, benders_cuts: None, relational_query_plan: None, linear_lifecycle: None,  indexed_registration: None, column_batch: None, column_scope: None, heart_beat: None, response_envelope: None, quadratic_orchestration: None, vrp_problem: None, network_resilience_problem: None, allocation_problem: None, network_structure_problem: None, network_design_problem: None, network_generalized_flow_problem: None, network_flow_problem: None, sweep_cases: None,  incumbent_values: None, solution_pool_options: None, gantt_activity_options: None, iis_options: None, bpp3d_options: None, protocol_version: 1, solver: backend.id().into(), model: model.clone(), model_kind: None, time_limit_ms: Some(10_000),
            tuning_options: None,
            cp_options: None, serial_options: None, parallel_options: None, fixed_recourse_options: None, column_generation_options: None,
            bpp1d_options: None, csp1d_options: None, bpp2d_options: None, bpp_loading_options: None, csp2d_options: None };
        let job = engine.submit(request).unwrap(); if variant == 5 { engine.cancel(&job.id).unwrap(); }
        let deadline = Instant::now() + Duration::from_secs(15);
        let view = loop { let view = engine.get(&job.id).unwrap(); if view.status != JobStatus::Running { break view; }
            assert!(Instant::now() < deadline); std::thread::sleep(Duration::from_millis(1)); };
        assert_eq!(view.status, JobStatus::Completed, "variant {variant}: {:?}", view.error);
        let report = context.read_report(view.report_json.as_ref().unwrap()).unwrap_or_else(|error| panic!("variant {variant}: {error}"));
        if variant == 2 || variant == 6 { assert!(report.solution.is_none()); assert_eq!(report.summary.problem_status, "INFEASIBLE"); }
        else if variant != 5 {
            let plan = report.solution.as_ref().unwrap();
            assert_eq!(plan.loaded_count, if variant == 4 { "0" } else { "2" });
            if variant == 7 { assert_eq!(plan.loaded_weight.value, 8000.0); assert_eq!(plan.loaded_weight.unit, QuantityUnit::Gram); }
        }
        assert_eq!(engine.cancel(&job.id).unwrap().report_json, view.report_json); engine.release(&job.id).unwrap();
        fixtures.push(serde_json::json!({ "source": source, "model": model, "reportJson": report.json, "resultJson": report.solution.map(|value| serde_json::to_string(&value).unwrap()) }));
    }
    if let Some(directory) = std::env::var_os("OSPF_BPP_LOADING_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory); std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(directory.join(format!("bpp-loading-{}.json", backend.id())), serde_json::to_string(&fixtures).unwrap()).unwrap();
    }
}

#[cfg(feature = "scip")]
#[test]
fn real_scip_solves_original_shared_loading_models_and_preserves_actual_reports() { contract(NativeBackend::Scip); }

#[cfg(any(feature = "gurobi10", feature = "gurobi11", feature = "gurobi12"))]
#[test]
fn real_gurobi_solves_original_shared_loading_models_and_preserves_actual_reports() { contract(NativeBackend::Gurobi); }

fn application_contract(backend: NativeBackend) {
    let engine = NativeEngine::new(1); let mut fixtures = Vec::new();
    for variant in 0..7 {
        let mut problem = problem(); problem.conflicts.clear(); problem.items[0].required = false; problem.items[1].required = false;
        let mut cargo = problem.items[1].clone(); cargo.id = "c".into(); problem.items.push(cargo);
        problem.positions[2].enabled = false;
        problem.allowed_assignments = ["a", "b", "c"].into_iter().flat_map(|item| ["p", "q"].into_iter().map(move |position| BppLoadingAssignment { item_id: item.into(), position_id: position.into() })).collect();
        let objective: BppLoadingObjective = serde_json::from_str(r#"{"itemCosts":{"a":-10,"b":-3,"c":-7},"positionCosts":{"p":1,"q":1},"layoutCosts":[]}"#).unwrap();
        match variant {
            1 => { for item in &mut problem.items { item.required = true; } },
            3 => { problem.items.clear(); problem.allowed_assignments.clear(); problem.positions[0].fixed_occupied = true; },
            4 => { problem.items[0].fixed_position_id = Some("p".into()); problem.positions[0].enabled = false; },
            5 => problem.weight_unit = QuantityUnit::Gram,
            _ => {},
        }
        let mut objective = objective; if variant == 3 { objective.item_costs.clear(); }
        let context = BppLoadingContext::with_objective(problem, objective).unwrap(); let source = context.artifact();
        let request: SolveRequest = serde_json::from_value(serde_json::json!({"protocolVersion":1,"solver":"bpp-loading","modelKind":"bppLoading",
            "model":source,"timeLimitMs":if variant == 6 { 1 } else { 10_000 },"bppLoadingOptions":{"backend":backend}})).unwrap();
        let job = engine.submit(request).unwrap(); if variant == 2 { engine.cancel(&job.id).unwrap(); }
        let deadline = Instant::now() + Duration::from_secs(15);
        let view = loop { let view = engine.get(&job.id).unwrap(); if view.status != JobStatus::Running { break view; }
            assert!(Instant::now() < deadline); std::thread::sleep(Duration::from_millis(1)); };
        assert_eq!(view.status, JobStatus::Completed, "variant {variant}: {:?}", view.error);
        let json = view.report_json.as_ref().unwrap(); let report = decode_bpp_loading_application(json, &source).unwrap();
        if variant == 1 { assert!(report.solution.is_none()); assert_eq!(report.summary.problem_status, "INFEASIBLE"); }
        else if variant != 2 && variant != 6 {
            assert!(report.envelope.registration_present); assert_eq!(report.envelope.objective, Some(if variant == 3 { 1.0 } else { -15.0 }));
            assert_eq!(report.solution.as_ref().unwrap().loaded_count, if variant == 3 { "0" } else { "2" });
            if variant == 5 { assert_eq!(report.solution.as_ref().unwrap().loaded_weight.unit, QuantityUnit::Gram); }
        }
        assert_eq!(engine.cancel(&job.id).unwrap().report_json, view.report_json); engine.release(&job.id).unwrap();
        fixtures.push(serde_json::json!({"source":source,"model":context.application_model().unwrap(),"applicationReportJson":json,"resultJson":report.solution.map(|value| serde_json::to_string(&value).unwrap())}));
    }
    if let Some(directory) = std::env::var_os("OSPF_BPP_LOADING_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory); std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(directory.join(format!("bpp-application-{}.json", backend.id())), serde_json::to_string(&fixtures).unwrap()).unwrap();
    }
}

#[cfg(feature = "scip")]
#[test]
fn real_scip_executes_original_loading_application_with_business_objectives() { application_contract(NativeBackend::Scip); }

#[cfg(any(feature = "gurobi10", feature = "gurobi11", feature = "gurobi12"))]
#[test]
fn real_gurobi_executes_original_loading_application_with_business_objectives() { application_contract(NativeBackend::Gurobi); }

fn resource_application_contract(backend: NativeBackend) {
    let engine = NativeEngine::new(1); let mut fixtures = Vec::new();
    for variant in 0..5 {
        let mut problem: BppLoadingProblem = serde_json::from_str(r#"{"items":[{"id":"a","weight":{"value":1000,"unit":"g"},"required":false,"fixedPositionId":null}],"positions":[{"id":"p","maxCount":"18446744073709551615","maxWeight":{"value":2,"unit":"kg"},"fixedOccupied":false,"enabled":true}],"allowedAssignments":[{"itemId":"a","positionId":"p"}],"conflicts":[],"layoutGroups":[],"weightUnit":"kg"}"#).unwrap();
        let objective = serde_json::from_str(r#"{"itemCosts":{"a":-5},"positionCosts":{"p":1},"layoutCosts":[]}"#).unwrap();
        let mut resource: BppLoadingResource = serde_json::from_str(r#"{"id":"ballast","positionId":"p","minimumWeight":{"value":0,"unit":"kg"},"maximumWeight":{"value":1000,"unit":"g"},"minimumPerLoadedWeight":0.5,"costPerWeight":2}"#).unwrap();
        if variant == 1 { problem.items[0].required = true; problem.positions[0].max_weight.value = 1.4; }
        if variant == 2 { resource.cost_per_weight = -2.0; }
        if variant == 3 { problem.weight_unit = QuantityUnit::Gram; resource.cost_per_weight = 0.002; }
        let context = BppLoadingContext::with_resources(problem, Some(objective), vec![resource]).unwrap(); let source = context.artifact();
        let request = serde_json::from_value(serde_json::json!({"protocolVersion":1,"solver":"bpp-loading","modelKind":"bppLoading","model":source,
            "timeLimitMs":10_000,"bppLoadingOptions":{"backend":backend}})).unwrap();
        let job = engine.submit(request).unwrap(); if variant == 4 { engine.cancel(&job.id).unwrap(); }
        let deadline = Instant::now() + Duration::from_secs(15);
        let view = loop { let view = engine.get(&job.id).unwrap(); if view.status != JobStatus::Running { break view; }
            assert!(Instant::now() < deadline); std::thread::sleep(Duration::from_millis(1)); };
        assert_eq!(view.status, JobStatus::Completed, "variant {variant}: {:?}", view.error);
        let json = view.report_json.as_ref().unwrap(); let report = decode_bpp_loading_application(json, &source).unwrap();
        if variant == 1 { assert!(report.solution.is_none()); assert_eq!(report.summary.problem_status, "INFEASIBLE"); }
        else if variant != 4 {
            assert!((report.envelope.objective.unwrap() - if variant == 2 { -6.0 } else { -3.0 }).abs() < 1e-7);
            let result = report.solution.as_ref().unwrap(); assert_eq!(result.loaded_count, "1");
            let resources = result.resources.as_ref().unwrap();
            assert!((resources.weights["ballast"].value - if variant == 2 { 1.0 } else if variant == 3 { 500.0 } else { 0.5 }).abs() < 1e-7);
            assert!((resources.total_weight.value - if variant == 2 { 2.0 } else if variant == 3 { 1500.0 } else { 1.5 }).abs() < 1e-7);
        }
        assert_eq!(engine.cancel(&job.id).unwrap().report_json, view.report_json); engine.release(&job.id).unwrap();
        fixtures.push(serde_json::json!({"source":source,"model":context.application_model().unwrap(),"applicationReportJson":json,
            "resultJson":report.solution.map(|value| serde_json::to_string(&value).unwrap())}));
    }
    if let Some(directory) = std::env::var_os("OSPF_BPP_LOADING_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory); std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(directory.join(format!("bpp-application-resources-{}.json", backend.id())), serde_json::to_string(&fixtures).unwrap()).unwrap();
    }
}

#[cfg(feature = "scip")]
#[test]
fn real_scip_executes_loading_application_with_coupled_continuous_resources() { resource_application_contract(NativeBackend::Scip); }

#[cfg(any(feature = "gurobi10", feature = "gurobi11", feature = "gurobi12"))]
#[test]
fn real_gurobi_executes_loading_application_with_coupled_continuous_resources() { resource_application_contract(NativeBackend::Gurobi); }

fn benders_contract(backend: NativeBackend, finishing: bool) {
    let engine = NativeEngine::new(1); let mut fixtures = Vec::new();
    for variant in 0..7 {
        let mut problem: BppLoadingProblem = serde_json::from_str(r#"{"items":[{"id":"a","weight":{"value":1000,"unit":"g"},"required":false,"fixedPositionId":null}],"positions":[{"id":"p","maxCount":"1","maxWeight":{"value":2,"unit":"kg"},"fixedOccupied":false,"enabled":true}],"allowedAssignments":[{"itemId":"a","positionId":"p"}],"conflicts":[],"layoutGroups":[],"weightUnit":"kg"}"#).unwrap();
        let objective = serde_json::from_str(r#"{"itemCosts":{"a":-5},"positionCosts":{"p":1},"layoutCosts":[]}"#).unwrap();
        let mut resource: BppLoadingResource = serde_json::from_str(r#"{"id":"ballast","positionId":"p","minimumWeight":{"value":0,"unit":"kg"},"maximumWeight":{"value":1000,"unit":"g"},"minimumPerLoadedWeight":0.5,"costPerWeight":2}"#).unwrap();
        if variant == 1 { problem.items[0].required = true; problem.positions[0].max_weight.value = 1.4; }
        if variant == 2 { resource.cost_per_weight = -2.0; }
        if variant == 3 { problem.weight_unit = QuantityUnit::Gram; resource.cost_per_weight = 0.002; }
        let context = BppLoadingContext::with_resources(problem, Some(objective), vec![resource]).unwrap(); let source = context.artifact();
        let mut benders = serde_json::json!({"recourseResources":["ballast"],"iterationLimit":if variant == 4 || variant == 6 { 1 } else { 128 },"fallbackToMilp":variant == 4});
        if finishing { benders["sourceMilpFinishing"] = serde_json::json!({"reserveTimeMs":3000}); }
        let request = serde_json::from_value(serde_json::json!({"protocolVersion":1,"solver":"bpp-loading","modelKind":"bppLoading","model":source,
            "timeLimitMs":10_000,"bppLoadingOptions":{"backend":backend,"benders":benders}})).unwrap();
        let job = engine.submit(request).unwrap(); if variant == 5 { engine.cancel(&job.id).unwrap(); }
        let deadline = Instant::now() + Duration::from_secs(15);
        let view = loop { let view = engine.get(&job.id).unwrap(); if view.status != JobStatus::Running { break view; }
            assert!(Instant::now() < deadline); std::thread::sleep(Duration::from_millis(1)); };
        assert_eq!(view.status, JobStatus::Completed, "Benders variant {variant}: {:?}", view.error);
        let json = view.report_json.as_ref().unwrap(); let report = decode_bpp_loading_benders(json, &source).unwrap();
        let core: ospf_rust_core::solver::SolveReport<f64, u64> = serde_json::from_str(&report.application.envelope.report_json).unwrap();
        if finishing {
            let evidence = report.envelope.certification.as_ref().unwrap();
            let acceleration = decode_bpp_loading_application(&evidence.previous_application_report_json, &source).unwrap();
            if variant == 4 { assert!(evidence.report_json.is_none(), "already certified fallback cannot trigger another backend"); }
            else if variant != 5 {
                assert!(evidence.report_json.is_some(), "full-source finishing must actually invoke the backend");
                assert!(serde_json::from_str::<ospf_rust_core::solver::SolveReport<f64, u64>>(&acceleration.envelope.report_json).unwrap().proof.is_none());
                if variant != 1 { assert!(core.is_optimal()); }
            }
        } else if variant != 4 { assert!(core.proof.is_none(), "merged decomposition cannot become a source proof"); }
        if variant == 1 {
            let summary = &report.decomposition.as_ref().unwrap().summary; assert!(summary.feasibility_cuts > 0);
            assert_eq!(summary.terminal_problem_status.as_deref(), Some("INFEASIBLE"));
            assert!(report.application.solution.is_none()); assert_eq!(report.application.summary.problem_status, if finishing { "INFEASIBLE" } else { "UNKNOWN" });
        } else if variant != 5 {
            let decomposition = report.decomposition.as_ref().unwrap(); let result = report.application.solution.as_ref().unwrap();
            assert!((report.application.envelope.objective.unwrap() - if variant == 2 { -6.0 } else { -3.0 }).abs() < 1e-7);
            assert_eq!(result.loaded_count, "1"); assert_eq!(result.resources.as_ref().unwrap().total_weight.unit, if variant == 3 { QuantityUnit::Gram } else { QuantityUnit::Kilogram });
            if variant == 0 || variant == 3 { assert_eq!(decomposition.summary.stop, FixedRecourseStop::Converged); assert!(decomposition.summary.optimality_cuts > 0); }
            if variant == 4 { assert!(report.envelope.fallback_used); assert!(core.is_optimal()); assert_eq!(decomposition.summary.stop, FixedRecourseStop::IterationLimit); }
            if variant == 6 { assert_eq!(decomposition.summary.stop, FixedRecourseStop::IterationLimit); assert!(!report.envelope.fallback_used); }
        }
        assert_eq!(engine.cancel(&job.id).unwrap().report_json, view.report_json); engine.release(&job.id).unwrap();
        fixtures.push(serde_json::json!({"source":source,"model":context.application_model().unwrap(),"bendersReportJson":json,
            "resultJson":report.application.solution.map(|value| serde_json::to_string(&value).unwrap())}));
    }
    if let Some(directory) = std::env::var_os("OSPF_BPP_LOADING_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory); std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(directory.join(format!("bpp-{}-{}.json", if finishing { "finishing" } else { "benders" }, backend.id())), serde_json::to_string(&fixtures).unwrap()).unwrap();
    }
}

#[cfg(feature = "scip")]
#[test]
fn real_scip_runs_original_loading_benders_cuts_and_fresh_milp_fallback() { benders_contract(NativeBackend::Scip, false); }

#[cfg(any(feature = "gurobi10", feature = "gurobi11", feature = "gurobi12"))]
#[test]
fn real_gurobi_runs_original_loading_benders_cuts_and_fresh_milp_fallback() { benders_contract(NativeBackend::Gurobi, false); }

#[cfg(feature = "scip")]
#[test]
fn real_scip_finishes_benders_on_fresh_complete_source_and_preserves_existing_proofs() { benders_contract(NativeBackend::Scip, true); }

#[cfg(any(feature = "gurobi10", feature = "gurobi11", feature = "gurobi12"))]
#[test]
fn real_gurobi_finishes_benders_on_fresh_complete_source_and_preserves_existing_proofs() { benders_contract(NativeBackend::Gurobi, true); }
