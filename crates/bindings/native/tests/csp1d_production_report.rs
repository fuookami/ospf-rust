//! 验收真实源方案、实际报告、领域提取和共享执行入口。
//! Accept actual source plans, reports, domain extraction and the shared execution entry.

#![cfg(any(feature = "scip", feature = "gurobi10", feature = "gurobi11", feature = "gurobi12"))]

use std::sync::Arc;
use std::time::{Duration, Instant};
use ospf_rust_bridge::{Csp1dProblem, Csp1dUsage, decode_csp1d_production_report, encode_csp1d_pattern, encode_csp1d_problem, encode_csp1d_production_report};
use ospf_rust_core::solver::LinearSolver;
use ospf_rust_framework_csp1d::{Csp1dColumnGeneration, Csp1dMilp, Csp1dNativeBackend, Csp1dSolutionStatus, GenerationConstraints, NSameGenerator, ReducedCostPricingGenerator};

fn problem() -> Csp1dProblem {
    serde_json::from_str(r#"{"products":[{"id":"p","name":"product","width":[{"value":30,"unit":"mm"}]}],"materials":[{"id":"m","name":"material","widthRange":{"lowerBound":{"value":0,"unit":"cm"},"upperBound":{"value":10,"unit":"cm"}},"machineId":"mc","availableBatches":"3"}],"machines":[{"id":"mc","name":"machine","widthRange":null,"maxBatchCount":"3"}],"demands":[{"productId":"p","mode":"roll","count":"5"}],"lengthUnit":"cm"}"#).unwrap()
}

fn contract(solver: Arc<dyn LinearSolver>, backend: &str) {
    let problem = problem();
    let mut fixtures = Vec::new();
    for cg in [false, true] {
        for feasible in [false, true] {
            let mut problem = problem.clone();
            if !feasible { problem.materials[0].available_batches = Some("1".into()); }
            let source = encode_csp1d_problem(problem.clone()).unwrap();
            let backend = Arc::new(Csp1dNativeBackend::new(solver.clone(), Default::default()).unwrap());
            let generator = NSameGenerator::with_constraints(GenerationConstraints { max_knife_count: Some(16), ..Default::default() })
                .with_all_amount(true).with_max_plans(128);
            let result = if cg {
                Csp1dColumnGeneration { initial_generator: Box::new(generator.clone()), pricing_generator: Box::new(ReducedCostPricingGenerator::new(generator)),
                    lp_solver: Some(backend.clone()), milp_solver: Some(backend.clone()), ..Default::default() }.solve_with_trace(problem.domain().unwrap(), None)
            } else {
                Csp1dMilp { initial_generator: Box::new(generator), milp_solver: Some(backend.clone()), ..Default::default() }.solve_with_trace(problem.domain().unwrap(), None)
            };
            let records = backend.reports();
            let child = records.last().unwrap();
            let text = encode_csp1d_production_report(&source, &result.solution.generated_plans, child).unwrap();
            let decoded = decode_csp1d_production_report(&text, &source).unwrap();
            assert_eq!(decoded.artifact.child.solver_report_json, serde_json::to_string(&child.report).unwrap());
            assert_eq!(decoded.artifact.child.model.model_fingerprint, child.report.fingerprints.model.clone().unwrap());
            assert!(!decoded.summary.candidate_complete);
            fixtures.push(serde_json::json!({ "source": source, "reportJson": text,
                "productionJson": decoded.production.as_ref().map(|production| serde_json::to_string(production).unwrap()) }));
            if feasible {
                assert_eq!(result.solution.status, Csp1dSolutionStatus::Feasible, "{:?}", result.trace);
                let expected = result.solution.produce.cutting_plans.iter().map(|usage| Csp1dUsage {
                    plan: encode_csp1d_pattern(&usage.plan, problem.length_unit).unwrap(), count: usage.amount.to_string(),
                }).collect::<Vec<_>>();
                assert_eq!(decoded.production.as_ref().unwrap().usages, expected);
                assert_eq!(decoded.production.as_ref().unwrap().batch_count, "2");
                assert_eq!(decoded.production.as_ref().unwrap().produced["p"], "6");
                assert!(decoded.summary.restricted_optimal);
            } else {
                assert_eq!(result.solution.status, Csp1dSolutionStatus::Failed);
                assert!(decoded.production.is_none());
                assert!(decoded.summary.restricted_infeasible);
            }
        }
    }
    // 显式开发联调才输出证据，路径由调用方提供；生成文件不入库。
    // Emit evidence only for explicit development integration at a caller-provided path; generated files stay untracked.
    if let Some(directory) = std::env::var_os("OSPF_CSP1D_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(directory.join(format!("csp1d-{backend}.json")), serde_json::to_string(&fixtures).unwrap()).unwrap();
    }
}

fn execution_contract(backend: ospf_rust_bridge::NativeBackend) {
    use ospf_rust_bridge::{Csp1dAlgorithm, Csp1dBusinessStatus, Csp1dGenerationOptions, Csp1dOptions, JobStatus, ModelKind, SolveRequest, decode_csp1d_execution_report};
    let engine = ospf_rust_native::NativeEngine::new(1);
    let source = encode_csp1d_problem(problem()).unwrap();
    let mut fixtures = Vec::new();
    for algorithm in [Csp1dAlgorithm::Milp, Csp1dAlgorithm::ColumnGeneration] {
        for initial in [None, Some(1)] {
            let configuration = Csp1dOptions { backend, algorithm, generation: initial.map(|limit| Csp1dGenerationOptions { max_plans: Some(limit), ..Default::default() }),
                iteration_limit: Some(8), columns_per_iteration: None, pool_limit: None };
            let request = SolveRequest { schedule_log_probe: None, operator_capability_probe: None, hypervolume_probe: None, acceptance_recovery_probe: None, training_split_probe: None, non_stationary_probe: None, selection_policy_probe: None, solve_value_probe: None, native_lowering_probe: None, native_terminal_proof_probe: None, predicate_scope_probe: None, persistence_schema_probe: None, persistence_sort_probe: None, effectiveness_ranking_probe: None, funnel_ranking: None, sensitivity_probe: None, correction_probe: None, blocking_set: None, fingerprint_probe: None, diagnostics_probe: None, gap_probe: None, proof_probe: None, evidence_probe: None, band_probe: None, group_summary: None, critical_report: None, effectiveness_probe: None, funnel: None, solution_status: None, diversity: None, piecewise: None, archive_probe: None, migration: None, topology: None, ranking_probe: None, acceptance: None, boundary: None, population_config: None, constraint_priority: None, constraint_ranking: None, pareto_archive: None, reference_points: None, dispersion: None, dominance: None, evidence_remap: None, tier_selection: None, activation_set: None, container_surface: None, identity_wrap: None, domain_surface: None, expression_bounds: None, assignment_eval: None, constraint_canon: None, snapshot_gate: None, cp_model_build: None, no_good_oracle: None, subproblem_result: None, cp_support_merge: None, capability_matrix: None, analysis_options: None, model_view: None, polynomial_probe: None, matrix_rows: None, sparse_triad: None, master_assignment: None, stable_value: None, quadratic_elastic: None, dump_batch: None, lazy_flatten_init: None, candidate_shortlist: None, writer_registry: None, native_write_scope: None, dump_options: None, assignment_composition: None, cut_validity: None, source_identity: None, binding_assignment: None, auxiliary_domain: None, activity_report_check: None, evidence_predicates: None, activity_bands: None, encoding_label: None, expression_key: None, canonical_monomial: None, backend_features: None, deferred_materialize: None, quadratic_monomial: None, polarity_encoding: None, tier_classification: None, assumption_binding: None, effectiveness_aggregate: None, bounded_no_good: None, polynomial_lift: None, scalar_function: None, field_resolution: None, boolean_fold: None, master_cut: None, flatten_identity: None, critical_assembly: None, variable_kind_domain: None, sort_by: None, predicate_schema: None, search_budget: None, conditional_semantics: None, big_m_policy: None, number_text: None, combine_status: None, elastic_model: None, native_write: None, input_domain: None, function_usage: None, repository_call: None, unsupported_predicate: None, boolean_scope: None, update_assignment: None, flatten_cache: None, cache_key_address: None, cache_key_identity: None, model_configuration: None, predicate_field_filter: None, predicate_field_resolution: None, analysis_group_aggregate: None, master_cut_encoding: None, q_table: None, heuristic_schedule: None, symbol_core_relations: None, symbol_linearization: None, symbol_condition: None, benders_cut_request: None, benders_metrics: None, cp_checkpoint_request: None, lbb_resume_identity: None, linear_mapping_request: None, checkpoint_request: None, reward_policy: None, hyper_heuristic_config: None, criticality_request: None, multi_target_report: None, relaxation_policy: None, target_spec: None, target_report: None, perturbation_policy: None, fixed_integer_report: None, fixed_integer_config: None, relaxation_request: None, critical_analysis: None, effectiveness_ranking: None, shortlist_limit: None, candidate_funnel: None, conflict_explanation: None, remote_artifact: None, logic_based_benders_plan: None, master_solution: None, benders_quadratic_cuts: None, pipeline_steps: None, repository_query: None, assignments: None, sort_plan: None, predicate: None, benders_cuts: None, relational_query_plan: None, linear_lifecycle: None,  indexed_registration: None, column_batch: None, column_scope: None, heart_beat: None, response_envelope: None, quadratic_orchestration: None, vrp_problem: None, network_resilience_problem: None, allocation_problem: None, network_structure_problem: None, network_design_problem: None, network_generalized_flow_problem: None, network_flow_problem: None, sweep_cases: None,  incumbent_values: None, solution_pool_options: None, gantt_activity_options: None, iis_options: None, bpp3d_options: None, protocol_version: 1, solver: "csp1d".into(), model: source.clone(), model_kind: Some(ModelKind::Csp1d), time_limit_ms: Some(10000),
                tuning_options: None,
                cp_options: None, serial_options: None, parallel_options: None, fixed_recourse_options: None, column_generation_options: None, bpp1d_options: None, bpp2d_options: None, bpp_loading_options: None, csp2d_options: None, csp1d_options: Some(configuration) };
            let job = engine.submit(request).unwrap(); let deadline = Instant::now() + Duration::from_secs(15);
            let view = loop { let view = engine.get(&job.id).unwrap(); if view.status != JobStatus::Running { break view; }
                assert!(Instant::now() < deadline); std::thread::sleep(Duration::from_millis(1)); };
            assert_eq!(view.status, JobStatus::Completed, "{:?}", view.error);
            let report = decode_csp1d_execution_report(view.report_json.as_ref().unwrap(), &source).unwrap();
            assert!(!report.summary.candidate_complete);
            let feasible = initial.is_none() || algorithm == Csp1dAlgorithm::ColumnGeneration;
            assert_eq!(report.summary.business_status, if feasible { Csp1dBusinessStatus::Feasible } else { Csp1dBusinessStatus::Failed }, "{}", report.artifact.trace_json);
            if feasible { assert_eq!(report.production.as_ref().unwrap().batch_count, "2"); }
            if initial.is_some() && algorithm == Csp1dAlgorithm::ColumnGeneration {
                assert!(report.summary.stage_count > 2); assert!(report.summary.column_count > 1);
                assert!(report.summary.integer.as_ref().unwrap().restricted_optimal);
            }
            fixtures.push(serde_json::json!({ "source": source, "reportJson": view.report_json.unwrap(),
                "productionJson": report.production.as_ref().map(|value| serde_json::to_string(value).unwrap()) }));
            engine.release(&job.id).unwrap(); assert_eq!(engine.get(&job.id).unwrap_err().code, "NotFound");
        }
    }
    if let Some(directory) = std::env::var_os("OSPF_CSP1D_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory); std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(directory.join(format!("csp1d-execution-{}.json", backend.id())), serde_json::to_string(&fixtures).unwrap()).unwrap();
    }
    // 初始池缺少 Sheet 产品时，Phase I 必须按各需求单位补短缺并由真实定价恢复。
    // When the initial pool lacks the Sheet product, Phase I must add deficits in each demand's unit and recover through actual pricing.
    let mut mixed = problem(); let mut product = mixed.products[0].clone(); product.id = "q".into(); mixed.products.push(product);
    mixed.demands.push(ospf_rust_bridge::Csp1dDemand { product_id: "q".into(), mode: ospf_rust_bridge::Csp1dDemandMode::Sheet, count: "5".into() });
    mixed.materials[0].available_batches = Some("6".into()); mixed.machines[0].max_batch_count = Some("6".into());
    let source = encode_csp1d_problem(mixed).unwrap();
    let request = SolveRequest { schedule_log_probe: None, operator_capability_probe: None, hypervolume_probe: None, acceptance_recovery_probe: None, training_split_probe: None, non_stationary_probe: None, selection_policy_probe: None, solve_value_probe: None, native_lowering_probe: None, native_terminal_proof_probe: None, predicate_scope_probe: None, persistence_schema_probe: None, persistence_sort_probe: None, effectiveness_ranking_probe: None, funnel_ranking: None, sensitivity_probe: None, correction_probe: None, blocking_set: None, fingerprint_probe: None, diagnostics_probe: None, gap_probe: None, proof_probe: None, evidence_probe: None, band_probe: None, group_summary: None, critical_report: None, effectiveness_probe: None, funnel: None, solution_status: None, diversity: None, piecewise: None, archive_probe: None, migration: None, topology: None, ranking_probe: None, acceptance: None, boundary: None, population_config: None, constraint_priority: None, constraint_ranking: None, pareto_archive: None, reference_points: None, dispersion: None, dominance: None, evidence_remap: None, tier_selection: None, activation_set: None, container_surface: None, identity_wrap: None, domain_surface: None, expression_bounds: None, assignment_eval: None, constraint_canon: None, snapshot_gate: None, cp_model_build: None, no_good_oracle: None, subproblem_result: None, cp_support_merge: None, capability_matrix: None, analysis_options: None, model_view: None, polynomial_probe: None, matrix_rows: None, sparse_triad: None, master_assignment: None, stable_value: None, quadratic_elastic: None, dump_batch: None, lazy_flatten_init: None, candidate_shortlist: None, writer_registry: None, native_write_scope: None, dump_options: None, assignment_composition: None, cut_validity: None, source_identity: None, binding_assignment: None, auxiliary_domain: None, activity_report_check: None, evidence_predicates: None, activity_bands: None, encoding_label: None, expression_key: None, canonical_monomial: None, backend_features: None, deferred_materialize: None, quadratic_monomial: None, polarity_encoding: None, tier_classification: None, assumption_binding: None, effectiveness_aggregate: None, bounded_no_good: None, polynomial_lift: None, scalar_function: None, field_resolution: None, boolean_fold: None, master_cut: None, flatten_identity: None, critical_assembly: None, variable_kind_domain: None, sort_by: None, predicate_schema: None, search_budget: None, conditional_semantics: None, big_m_policy: None, number_text: None, combine_status: None, elastic_model: None, native_write: None, input_domain: None, function_usage: None, repository_call: None, unsupported_predicate: None, boolean_scope: None, update_assignment: None, flatten_cache: None, cache_key_address: None, cache_key_identity: None, model_configuration: None, predicate_field_filter: None, predicate_field_resolution: None, analysis_group_aggregate: None, master_cut_encoding: None, q_table: None, heuristic_schedule: None, symbol_core_relations: None, symbol_linearization: None, symbol_condition: None, benders_cut_request: None, benders_metrics: None, cp_checkpoint_request: None, lbb_resume_identity: None, linear_mapping_request: None, checkpoint_request: None, reward_policy: None, hyper_heuristic_config: None, criticality_request: None, multi_target_report: None, relaxation_policy: None, target_spec: None, target_report: None, perturbation_policy: None, fixed_integer_report: None, fixed_integer_config: None, relaxation_request: None, critical_analysis: None, effectiveness_ranking: None, shortlist_limit: None, candidate_funnel: None, conflict_explanation: None, remote_artifact: None, logic_based_benders_plan: None, master_solution: None, benders_quadratic_cuts: None, pipeline_steps: None, repository_query: None, assignments: None, sort_plan: None, predicate: None, benders_cuts: None, relational_query_plan: None, linear_lifecycle: None,  indexed_registration: None, column_batch: None, column_scope: None, heart_beat: None, response_envelope: None, quadratic_orchestration: None, vrp_problem: None, network_resilience_problem: None, allocation_problem: None, network_structure_problem: None, network_design_problem: None, network_generalized_flow_problem: None, network_flow_problem: None, sweep_cases: None,  incumbent_values: None, solution_pool_options: None, gantt_activity_options: None, iis_options: None, bpp3d_options: None, protocol_version: 1, solver: "csp1d".into(), model: source.clone(), model_kind: Some(ModelKind::Csp1d), time_limit_ms: Some(10000),
        tuning_options: None,
        cp_options: None, serial_options: None, parallel_options: None, fixed_recourse_options: None, column_generation_options: None, bpp1d_options: None,
        bpp2d_options: None, bpp_loading_options: None, csp2d_options: None, csp1d_options: Some(Csp1dOptions { backend, algorithm: Csp1dAlgorithm::ColumnGeneration,
            generation: Some(Csp1dGenerationOptions { max_plans: Some(1), ..Default::default() }), iteration_limit: None, columns_per_iteration: None, pool_limit: None }) };
    let job = engine.submit(request).unwrap(); let deadline = Instant::now() + Duration::from_secs(15);
    let view = loop { let view = engine.get(&job.id).unwrap(); if view.status != JobStatus::Running { break view; }
        assert!(Instant::now() < deadline); std::thread::sleep(Duration::from_millis(1)); };
    assert_eq!(view.status, JobStatus::Completed, "{:?}", view.error);
    let report = decode_csp1d_execution_report(view.report_json.as_ref().unwrap(), &source).unwrap();
    assert_eq!(report.summary.business_status, Csp1dBusinessStatus::Feasible, "{}", report.artifact.trace_json);
    let production = report.production.unwrap(); assert_eq!(production.batch_count, "4");
    assert_eq!(production.produced["p"], "6"); assert_eq!(production.produced["q"], "6"); engine.release(&job.id).unwrap();
}

#[cfg(feature = "scip")]
#[test]
fn real_scip_production_matches_the_actual_integer_child_for_milp_and_column_generation() {
    use ospf_rust_core::solver::solvers::scip::{SCIPConfig, SCIPSolver};
    contract(Arc::new(SCIPSolver::with_config(SCIPConfig::default().with_output(false).with_lp_subproblem_defaults())), "scip");
    execution_contract(ospf_rust_bridge::NativeBackend::Scip);
}

#[cfg(any(feature = "gurobi10", feature = "gurobi11", feature = "gurobi12"))]
#[test]
fn real_gurobi_production_matches_the_actual_integer_child_for_milp_and_column_generation() {
    use ospf_rust_core::solver::solvers::gurobi::{GurobiConfig, GurobiSolver};
    contract(Arc::new(GurobiSolver::with_config(GurobiConfig::default().with_output(false))), "gurobi");
    execution_contract(ospf_rust_bridge::NativeBackend::Gurobi);
}
