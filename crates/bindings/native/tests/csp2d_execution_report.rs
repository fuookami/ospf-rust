//! 真实 CSP2D 求解、共享执行器与源绑定报告验收。 / Acceptance of real CSP2D solving, shared execution and source-bound reports.

#![cfg(any(feature = "scip", feature = "gurobi10", feature = "gurobi11", feature = "gurobi12"))]

use std::sync::Arc;
use std::time::Duration;
use ospf_rust_bridge::*;
use ospf_rust_core::solver::{CancellationOrigin, LinearSolver, SolveHandle};
use ospf_rust_framework_csp2d::application::{Csp2dExecutionOptions, solve_csp2d_column_generation};

fn meters(value: f64) -> PhysicalQuantity { PhysicalQuantity { value, unit: QuantityUnit::Meter } }
fn size(width: f64, height: f64) -> Geometry2dSheet { Geometry2dSheet { width: meters(width), height: meters(height) } }
fn region(x: f64, width: f64) -> Csp2dRegion { Csp2dRegion { origin: Geometry2dPoint { x: meters(x), y: meters(0.0) }, size: size(width, 1.0) } }

fn column(amount: usize) -> Csp2dColumn {
    Csp2dColumn { id: "seed".into(), material_id: "s".into(), region: region(0.0, 2.0),
        placements: (0..amount).map(|index| Geometry2dPlacement { shape: Geometry2dShape::Rectangle { width: meters(1.0), height: meters(1.0) },
            position: Geometry2dPoint { x: meters(index as f64), y: meters(0.0) }, rotated: false, item_id: Some("a".into()) }).collect(), cut_tree: None }
}

fn problem() -> Csp2dProblem {
    Csp2dProblem { products: vec![Csp2dProduct { id: "a".into(), name: "A".into(), geometry: size(1.0, 1.0), allow_rotate: false }],
        materials: vec![Csp2dMaterial { id: "s".into(), name: "Sheet".into(), shape: Csp2dMaterialShape::Sheet { geometry: size(4.0, 2.0) },
            available_batches: "10".into(), process: Csp2dCuttingProcess::Freeform, max_segment_height: None,
            products: vec![Csp2dGenerationProduct { product_id: "a".into(), count: "4".into() }] }],
        demands: vec![Csp2dDemand::Count { product_id: "a".into(), count: "3".into(), upper_bound: None }], initial_columns: Vec::new(),
        kerf: meters(0.0), objective: Default::default() }
}

fn options(backend: NativeBackend) -> Csp2dOptions {
    Csp2dOptions { backend, generators: None, iteration_limit: None, candidates_per_iteration: None, pool_limit: None,
        final_integer_enabled: None, reduced_cost_tolerance: None, report_limit: None }
}

fn contract(solver: Arc<dyn LinearSolver>, backend: NativeBackend) {
    let mut fixtures = Vec::new();
    for variant in 0..15 {
        let mut problem = problem(); let mut configuration = options(backend); let mut execution = Csp2dExecutionOptions::default();
        match variant {
            1 => { problem.initial_columns = vec![column(1), column(1)]; },
            2 => problem.demands.push(Csp2dDemand::Area { product_id: "a".into(), area: PhysicalQuantity { value: 30_000.0, unit: QuantityUnit::SquareCentimeter }, upper_bound: None }),
            3 => if let Csp2dDemand::Count { upper_bound, .. } = &mut problem.demands[0] { *upper_bound = Some("3".into()); },
            4 => {
                problem.materials[0].shape = Csp2dMaterialShape::Coil { width: meters(4.0), length: Some(meters(2.0)) };
                problem.materials[0].max_segment_height = Some(meters(1.0));
                problem.objective.waste_weight = 2.0; problem.objective.height_weight = 3.0; problem.objective.usage_weight = 5.0;
            },
            5 => {
                problem.materials[0].available_batches = "1".into();
                let mut other = problem.materials[0].clone(); other.id = "expensive".into(); other.available_batches = "10".into(); other.shape = Csp2dMaterialShape::Sheet { geometry: size(5.0, 2.0) };
                problem.materials.push(other); problem.demands = vec![Csp2dDemand::Count { product_id: "a".into(), count: "7".into(), upper_bound: None }];
            },
            6 => problem.demands = vec![Csp2dDemand::Count { product_id: "a".into(), count: "0".into(), upper_bound: None }],
            7 => { let handle = SolveHandle::new(); handle.cancel(CancellationOrigin::External); execution.cancellation = handle; },
            8 => execution.time_limit = Duration::ZERO,
            9 => configuration.report_limit = Some(1),
            10 => configuration.final_integer_enabled = Some(false),
            11 => {
                configuration.iteration_limit = Some(0); problem.kerf = PhysicalQuantity { value: 125.0, unit: QuantityUnit::Millimeter };
                problem.materials[0].process = Csp2dCuttingProcess::GuillotineTwoStage;
                let mut column = column(2); column.region.size.width = meters(2.125); column.placements[1].position.x = meters(1.125);
                column.cut_tree = Some(Csp2dCutTree { nodes: vec![Csp2dCutNode::Leaf { region: region(0.0, 1.0), product_id: Some("a".into()) },
                    Csp2dCutNode::Leaf { region: region(1.125, 1.0), product_id: Some("a".into()) },
                    Csp2dCutNode::Cut { region: region(0.0, 2.125), orientation: Csp2dCutOrientation::Vertical, position: meters(1.0), stage: 1, lower: 0, upper: 1 }] });
                problem.initial_columns.push(column);
            },
            12 => {
                configuration.iteration_limit = Some(0); problem.products[0].geometry = size(2.0, 1.0); problem.products[0].allow_rotate = true;
                let mut column = column(2); column.region.size = size(2.0, 2.0);
                for placement in &mut column.placements { placement.shape = Geometry2dShape::Rectangle { width: meters(1.0), height: meters(2.0) }; placement.rotated = true; }
                problem.initial_columns.push(column);
            },
            13 => if let Csp2dDemand::Count { upper_bound, .. } = &mut problem.demands[0] { *upper_bound = Some("4".into()); },
            14 => {
                let mut other = problem.products[0].clone(); other.id = "b".into(); problem.products.push(other);
                problem.materials[0].products.push(Csp2dGenerationProduct { product_id: "b".into(), count: "4".into() });
                problem.demands.push(Csp2dDemand::Count { product_id: "b".into(), count: "3".into(), upper_bound: None }); problem.initial_columns.push(column(1));
            },
            _ => {},
        }
        let source = encode_csp2d_problem(problem).unwrap();
        let (_, mut input) = decode_csp2d_problem(&source).unwrap(); input.config = configuration.config().unwrap();
        execution.report_limit = configuration.report_limit.unwrap_or(196) as usize;
        let result = solve_csp2d_column_generation(solver.clone(), execution, &input, &configuration.generators().unwrap()).unwrap_or_else(|error| panic!("variant {variant}: {error}"));
        let text = encode_csp2d_execution_report(&source, configuration, &result).unwrap_or_else(|error| panic!("variant {variant}: {error}"));
        let decoded = decode_csp2d_execution_report(&text, &source).unwrap();
        assert!(!decoded.summary.candidate_complete); assert_eq!(decoded.summary.column_count as usize, result.columns.len());
        for (actual, original) in decoded.artifact.stages.iter().zip(&result.stages) { assert_eq!(actual.solver_report_json, serde_json::to_string(&original.result.stage_report.report).unwrap()); }
        match variant {
            3 => { assert!(decoded.production.is_none()); assert!(decoded.summary.integer.as_ref().unwrap().restricted_infeasible); },
            7..=10 => {
                assert!(decoded.production.is_none());
                if variant == 10 { assert!(decoded.summary.integer.is_none()); }
                else { assert_eq!(decoded.summary.stop, match variant { 7 => Csp2dExecutionStop::Cancelled, 8 => Csp2dExecutionStop::TimeLimit, _ => Csp2dExecutionStop::ReportLimit }); }
            },
            _ => {
                let production = decoded.production.as_ref().unwrap_or_else(|| panic!("variant {variant}: {:?}", decoded.summary));
                if variant == 6 { assert_eq!(production.produced_counts["a"], "0"); assert!(production.usages.is_empty()); }
                else { assert!(production.produced_counts["a"].parse::<u128>().unwrap() >= 3); }
                if variant == 4 { assert_eq!(production.coil_heights["s"].value, 1.0); assert_eq!(production.objective_value, 12.0); }
                if variant == 5 { assert!(result.stages.iter().any(|stage| stage.result.shadow_prices.as_ref().is_some_and(|prices| prices.materials["s"] < 0.0))); }
                if variant == 11 { assert_eq!(production.consumed_area.value, 4.25); assert_eq!(production.waste_area.value, 0.25); }
                if variant == 12 { assert_eq!(production.consumed_area.value, 8.0); }
                if variant == 14 { assert!(decoded.stages[0].restricted_infeasible); assert!(production.produced_counts["b"].parse::<u128>().unwrap() >= 3); }
            },
        }
        fixtures.push(serde_json::json!({ "source": source, "reportJson": text, "productionJson": decoded.production.as_ref().map(|value| serde_json::to_string(value).unwrap()) }));
    }
    // 证据仅写调用方指定的 ignored 目录，不作为发布或跨仓构建依赖。 / Evidence goes only to a caller-selected ignored directory, never becoming a release/cross-repository build dependency.
    if let Some(directory) = std::env::var_os("OSPF_CSP2D_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory); std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(directory.join(format!("csp2d-execution-{}.json", backend.id())), serde_json::to_string(&fixtures).unwrap()).unwrap();
    }
}

fn engine_contract(backend: NativeBackend) {
    use std::time::Instant;
    let engine = ospf_rust_native::NativeEngine::new(1);
    let source = encode_csp2d_problem(problem()).unwrap();
    for variant in 0..4 {
        let mut options = options(backend);
        if variant == 1 { options.report_limit = Some(1); }
        if variant == 2 { options.final_integer_enabled = Some(false); }
        let request = SolveRequest { schedule_log_probe: None, operator_capability_probe: None, hypervolume_probe: None, acceptance_recovery_probe: None, training_split_probe: None, non_stationary_probe: None, selection_policy_probe: None, solve_value_probe: None, native_lowering_probe: None, native_terminal_proof_probe: None, predicate_scope_probe: None, persistence_schema_probe: None, persistence_sort_probe: None, effectiveness_ranking_probe: None, funnel_ranking: None, sensitivity_probe: None, correction_probe: None, blocking_set: None, fingerprint_probe: None, diagnostics_probe: None, gap_probe: None, proof_probe: None, evidence_probe: None, band_probe: None, group_summary: None, critical_report: None, effectiveness_probe: None, funnel: None, solution_status: None, diversity: None, piecewise: None, archive_probe: None, migration: None, topology: None, ranking_probe: None, acceptance: None, boundary: None, population_config: None, constraint_priority: None, constraint_ranking: None, pareto_archive: None, reference_points: None, dispersion: None, dominance: None, evidence_remap: None, tier_selection: None, activation_set: None, container_surface: None, identity_wrap: None, domain_surface: None, expression_bounds: None, assignment_eval: None, constraint_canon: None, snapshot_gate: None, cp_model_build: None, no_good_oracle: None, subproblem_result: None, cp_support_merge: None, capability_matrix: None, analysis_options: None, model_view: None, polynomial_probe: None, matrix_rows: None, sparse_triad: None, master_assignment: None, stable_value: None, quadratic_elastic: None, dump_batch: None, lazy_flatten_init: None, candidate_shortlist: None, writer_registry: None, native_write_scope: None, dump_options: None, assignment_composition: None, cut_validity: None, source_identity: None, binding_assignment: None, auxiliary_domain: None, activity_report_check: None, evidence_predicates: None, activity_bands: None, encoding_label: None, expression_key: None, canonical_monomial: None, backend_features: None, deferred_materialize: None, quadratic_monomial: None, polarity_encoding: None, tier_classification: None, assumption_binding: None, effectiveness_aggregate: None, bounded_no_good: None, polynomial_lift: None, scalar_function: None, field_resolution: None, boolean_fold: None, master_cut: None, flatten_identity: None, critical_assembly: None, variable_kind_domain: None, sort_by: None, predicate_schema: None, search_budget: None, conditional_semantics: None, big_m_policy: None, number_text: None, combine_status: None, elastic_model: None, native_write: None, input_domain: None, function_usage: None, repository_call: None, unsupported_predicate: None, boolean_scope: None, update_assignment: None, flatten_cache: None, cache_key_address: None, cache_key_identity: None, model_configuration: None, predicate_field_filter: None, predicate_field_resolution: None, analysis_group_aggregate: None, master_cut_encoding: None, q_table: None, heuristic_schedule: None, symbol_core_relations: None, symbol_linearization: None, symbol_condition: None, benders_cut_request: None, benders_metrics: None, cp_checkpoint_request: None, lbb_resume_identity: None, linear_mapping_request: None, checkpoint_request: None, reward_policy: None, hyper_heuristic_config: None, criticality_request: None, multi_target_report: None, relaxation_policy: None, target_spec: None, target_report: None, perturbation_policy: None, fixed_integer_report: None, fixed_integer_config: None, relaxation_request: None, critical_analysis: None, effectiveness_ranking: None, shortlist_limit: None, candidate_funnel: None, conflict_explanation: None, remote_artifact: None, logic_based_benders_plan: None, master_solution: None, benders_quadratic_cuts: None, pipeline_steps: None, repository_query: None, assignments: None, sort_plan: None, predicate: None, benders_cuts: None, relational_query_plan: None, linear_lifecycle: None,  indexed_registration: None, column_batch: None, column_scope: None, heart_beat: None, response_envelope: None, quadratic_orchestration: None, vrp_problem: None, network_resilience_problem: None, allocation_problem: None, network_structure_problem: None, network_design_problem: None, network_generalized_flow_problem: None, network_flow_problem: None, sweep_cases: None,  incumbent_values: None, solution_pool_options: None, gantt_activity_options: None, iis_options: None, bpp3d_options: None, protocol_version: 1, solver: "csp2d".into(), model: source.clone(), model_kind: Some(ModelKind::Csp2d), time_limit_ms: Some(10_000),
            tuning_options: None,
            cp_options: None, serial_options: None, parallel_options: None, fixed_recourse_options: None, column_generation_options: None, bpp1d_options: None, csp1d_options: None,
            bpp2d_options: None, bpp_loading_options: None, csp2d_options: Some(options) };
        let submitted = engine.submit(request).unwrap();
        if variant == 3 { engine.cancel(&submitted.id).unwrap(); }
        let deadline = Instant::now() + Duration::from_secs(15);
        let view = loop { let view = engine.get(&submitted.id).unwrap(); if view.status != JobStatus::Running { break view; }
            assert!(Instant::now() < deadline); std::thread::sleep(Duration::from_millis(1)); };
        assert_eq!(view.status, JobStatus::Completed, "{:?}", view.error);
        let report = decode_csp2d_execution_report(view.report_json.as_ref().unwrap(), &source).unwrap();
        assert!(!report.summary.candidate_complete);
        if variant == 0 { assert!(report.production.is_some()); assert!(view.progress_json.is_some()); }
        if variant == 1 { assert_eq!(report.summary.stop, Csp2dExecutionStop::ReportLimit); assert!(report.production.is_none()); }
        if variant == 2 { assert!(report.summary.integer.is_none()); assert!(report.production.is_none()); }
        let retained = engine.cancel(&submitted.id).unwrap(); assert_eq!(retained.report_json, view.report_json);
        engine.release(&submitted.id).unwrap(); assert_eq!(engine.get(&submitted.id).unwrap_err().code, "NotFound");
    }
}

#[cfg(feature = "scip")]
#[test]
fn real_scip_source_bound_execution_report_and_production() {
    use ospf_rust_core_plugin_scip::{SCIPConfig, SCIPSolver};
    contract(Arc::new(SCIPSolver::with_config(SCIPConfig::default().with_output(false).with_lp_subproblem_defaults())), NativeBackend::Scip);
    engine_contract(NativeBackend::Scip);
}

#[cfg(any(feature = "gurobi10", feature = "gurobi11", feature = "gurobi12"))]
#[test]
fn real_gurobi_source_bound_execution_report_and_production() {
    use ospf_rust_core_plugin_gurobi::{GurobiConfig, GurobiSolver};
    contract(Arc::new(GurobiSolver::with_config(GurobiConfig::default().with_output(false))), NativeBackend::Gurobi);
    engine_contract(NativeBackend::Gurobi);
}
