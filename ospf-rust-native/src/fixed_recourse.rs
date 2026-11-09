//! 真实固定连续追索 Benders 执行边界 / Actual fixed-continuous-recourse Benders execution boundary.

use ospf_rust_core::solver::SolveOptions;
use ospf_rust_framework::solver::{FixedRecourseAlgorithm, FixedRecourseConfig, LinearBendersPartition};
use ospf_rust_bridge::{BridgeError, JobView, Result, SolveRequest, decode_meta_model, decode_model, encode_fixed_recourse_result};
use crate::{NativeEngine, create_solver};

impl NativeEngine {
    pub(crate) fn start_fixed_recourse(&self, request: SolveRequest) -> Result<JobView> {
        let configuration = request.fixed_recourse_options.clone().ok_or_else(|| BridgeError::new("InvalidInput", "missing fixed-recourse configuration"))?;
        configuration.validate()?;
        let solver = create_solver(configuration.backend.id(), None)?;
        self.start_fixed_recourse_with_solver(request, solver)
    }

    fn start_fixed_recourse_with_solver(&self, request: SolveRequest, solver: std::sync::Arc<dyn ospf_rust_core::solver::LinearSolver>) -> Result<JobView> {
        let configuration = request.fixed_recourse_options.clone().ok_or_else(|| BridgeError::new("InvalidInput", "missing fixed-recourse configuration"))?;
        configuration.validate()?;
        decode_model(&request.model)?;
        let artifact = request.model.clone();
        self.start_encoded(request, move |options| {
            // MetaModel 含非 Send 符号；在线程内部创建，绝不伪造 Send 实现。
            // MetaModel contains non-Send symbols; construct it inside the worker, never invent an unsafe Send implementation.
            let source = decode_meta_model(&artifact)?;
            let partition = LinearBendersPartition::build(&source, configuration.partition_options(&source)?)
                .map_err(|error| BridgeError::new(error.solver_error_class().code(), error))?;
            let defaults = FixedRecourseConfig::default();
            let config = FixedRecourseConfig {
                iteration_limit: configuration.iteration_limit.map_or(defaults.iteration_limit, |value| value as usize),
                time_limit: options.time_limit.unwrap_or(defaults.time_limit),
                tolerance: configuration.tolerance.unwrap_or(defaults.tolerance),
                recover_missing_certificates: configuration.recover_missing_certificates.unwrap_or(defaults.recover_missing_certificates),
            };
            let handle = options.cancellation_handle.ok_or_else(|| BridgeError::new("ExecutionFailure", "missing native cancellation handle"))?;
            let result = FixedRecourseAlgorithm::solve(&source, &partition, config, handle, |model, remaining, handle| {
                solver.solve_linear_report_with_options(model, &SolveOptions {
                    time_limit: Some(remaining), cancellation_handle: Some(handle), progress_reporter: options.progress_reporter,
                    ..Default::default()
                })
            }).map_err(|error| BridgeError::new(error.solver_error_class().code(), error))?;
            encode_fixed_recourse_result(&result, &source, &artifact, config.tolerance)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex, mpsc::{Receiver, SyncSender, sync_channel}};
    use std::time::{Duration, Instant};
    use ospf_rust_core::model::intermediate::LinearTriadModel;
    use ospf_rust_core::solver::{LinearSolver, ProblemStatus, SolveFingerprints, SolveProof, SolveReport, SolveSolution, SolverCapability, SolverInfo, SolverOutput, SolverProvenance, TerminationReason, cancelled_solve_report, linear_model_fingerprint, linear_model_mapping};
    use ospf_rust_bridge::{FixedRecourseOptions, FixedRecourseStop, JobStatus, NativeBackend, decode_fixed_recourse_report, encode_model};

    struct Gated { entered: SyncSender<()>, finish: Mutex<Receiver<()>> }
    impl SolverInfo for Gated {
        fn name(&self) -> &str { "fixed-recourse-fake" }
        fn capabilities(&self) -> Vec<SolverCapability> { vec![] }
    }
    impl LinearSolver for Gated {
        fn solve_linear(&self, _: &LinearTriadModel) -> ospf_rust_core::Result<SolverOutput> { unreachable!("report path required") }
        fn solve_linear_report_with_options(&self, model: &LinearTriadModel, options: &SolveOptions<'_>) -> ospf_rust_core::Result<SolveReport<f64>> {
            self.entered.send(()).unwrap(); self.finish.lock().unwrap().recv_timeout(Duration::from_secs(5)).unwrap();
            assert!(options.time_limit.unwrap() <= Duration::from_secs(1));
            let handle = options.cancellation_handle.unwrap();
            if handle.is_cancelled() { return cancelled_solve_report(SolverProvenance::default(), handle); }
            let mut solution = SolveSolution::vector(vec![0.0; model.num_variables()]); solution.objective_value = Some(0.0);
            SolveReport::builder(ProblemStatus::Feasible, TerminationReason::Completed).solution(solution).proof(SolveProof::optimality())
                .fingerprints(SolveFingerprints { model: Some(linear_model_fingerprint(model)?), ..Default::default() })
                .model_mapping(Some(linear_model_mapping(model)?)).build()
        }
    }

    #[test]
    fn worker_owned_meta_model_reuses_job_capacity_and_actual_cancellation_report() {
        for cancel in [false, true] {
            let model = LinearTriadModel::new("worker-owned-meta");
            let artifact = encode_model(&model).unwrap();
            let configuration = FixedRecourseOptions { backend: NativeBackend::Scip, stages: None, iteration_limit: None, tolerance: None, recover_missing_certificates: None };
            let request = SolveRequest { schedule_log_probe: None, operator_capability_probe: None, hypervolume_probe: None, acceptance_recovery_probe: None, training_split_probe: None, non_stationary_probe: None, selection_policy_probe: None, solve_value_probe: None, native_lowering_probe: None, native_terminal_proof_probe: None, predicate_scope_probe: None, persistence_schema_probe: None, persistence_sort_probe: None, effectiveness_ranking_probe: None, funnel_ranking: None, sensitivity_probe: None, correction_probe: None, blocking_set: None, fingerprint_probe: None, diagnostics_probe: None, gap_probe: None, proof_probe: None, evidence_probe: None, band_probe: None, group_summary: None, critical_report: None, effectiveness_probe: None, funnel: None, solution_status: None, diversity: None, piecewise: None, archive_probe: None, migration: None, topology: None, ranking_probe: None, acceptance: None, boundary: None, population_config: None, constraint_priority: None, constraint_ranking: None, pareto_archive: None, reference_points: None, dispersion: None, dominance: None, evidence_remap: None, tier_selection: None, activation_set: None, container_surface: None, identity_wrap: None, domain_surface: None, expression_bounds: None, assignment_eval: None, constraint_canon: None, snapshot_gate: None, cp_model_build: None, no_good_oracle: None, subproblem_result: None, cp_support_merge: None, capability_matrix: None, analysis_options: None, model_view: None, polynomial_probe: None, matrix_rows: None, sparse_triad: None, master_assignment: None, stable_value: None, quadratic_elastic: None, dump_batch: None, lazy_flatten_init: None, candidate_shortlist: None, writer_registry: None, native_write_scope: None, dump_options: None, assignment_composition: None, cut_validity: None, source_identity: None, binding_assignment: None, auxiliary_domain: None, activity_report_check: None, evidence_predicates: None, activity_bands: None, encoding_label: None, expression_key: None, canonical_monomial: None, backend_features: None, deferred_materialize: None, quadratic_monomial: None, polarity_encoding: None, tier_classification: None, assumption_binding: None, effectiveness_aggregate: None, bounded_no_good: None, polynomial_lift: None, scalar_function: None, field_resolution: None, boolean_fold: None, master_cut: None, flatten_identity: None, critical_assembly: None, variable_kind_domain: None, sort_by: None, predicate_schema: None, search_budget: None, conditional_semantics: None, big_m_policy: None, number_text: None, combine_status: None, elastic_model: None, native_write: None, input_domain: None, function_usage: None, repository_call: None, unsupported_predicate: None, boolean_scope: None, update_assignment: None, flatten_cache: None, cache_key_address: None, cache_key_identity: None, model_configuration: None, predicate_field_filter: None, predicate_field_resolution: None, analysis_group_aggregate: None, master_cut_encoding: None, q_table: None, heuristic_schedule: None, symbol_core_relations: None, symbol_linearization: None, symbol_condition: None, benders_cut_request: None, benders_metrics: None, cp_checkpoint_request: None, lbb_resume_identity: None, linear_mapping_request: None, checkpoint_request: None, reward_policy: None, hyper_heuristic_config: None, criticality_request: None, multi_target_report: None, relaxation_policy: None, target_spec: None, target_report: None, perturbation_policy: None, fixed_integer_report: None, fixed_integer_config: None, relaxation_request: None, critical_analysis: None, effectiveness_ranking: None, shortlist_limit: None, candidate_funnel: None, conflict_explanation: None, remote_artifact: None, logic_based_benders_plan: None, master_solution: None, benders_quadratic_cuts: None, pipeline_steps: None, repository_query: None, assignments: None, sort_plan: None, predicate: None, benders_cuts: None, relational_query_plan: None, linear_lifecycle: None,  indexed_registration: None, column_batch: None, column_scope: None, heart_beat: None, response_envelope: None, quadratic_orchestration: None, vrp_problem: None, network_resilience_problem: None, allocation_problem: None, network_structure_problem: None, network_design_problem: None, network_generalized_flow_problem: None, network_flow_problem: None, sweep_cases: None,  incumbent_values: None, solution_pool_options: None, gantt_activity_options: None, bpp3d_options: None, protocol_version: 1, solver: "fixed-recourse".into(), model: artifact.clone(), model_kind: None,
                tuning_options: None,
                time_limit_ms: Some(1000), cp_options: None, serial_options: None, parallel_options: None, iis_options: None, bpp1d_options: None, bpp2d_options: None, bpp_loading_options: None, csp2d_options: None, csp1d_options: None, column_generation_options: None, fixed_recourse_options: Some(configuration) };
            let engine = NativeEngine::new(1);
            let (entered, ready) = sync_channel(1); let (finish, gate) = sync_channel(1);
            let job = engine.start_fixed_recourse_with_solver(request, Arc::new(Gated { entered, finish: Mutex::new(gate) })).unwrap();
            ready.recv_timeout(Duration::from_secs(5)).unwrap();
            assert_eq!(engine.release(&job.id).unwrap_err().code, "Busy");
            if cancel { engine.cancel(&job.id).unwrap(); }
            finish.send(()).unwrap();
            let deadline = Instant::now() + Duration::from_secs(5);
            while engine.get(&job.id).unwrap().status == JobStatus::Running { assert!(Instant::now() < deadline); std::thread::sleep(Duration::from_millis(1)); }
            let view = engine.get(&job.id).unwrap();
            assert_eq!(view.status, JobStatus::Completed, "{:?}", view.error);
            let report = decode_fixed_recourse_report(view.report_json.as_ref().unwrap(), &artifact).unwrap();
            assert_eq!(report.summary.stop, if cancel { FixedRecourseStop::Cancelled } else { FixedRecourseStop::Converged });
            assert_eq!(report.summary.objective_value, if cancel { None } else { Some(0.0) });
            assert!(report.master_report_json.is_some());
            assert_eq!(engine.cancel(&job.id).unwrap().report_json, view.report_json);
            engine.release(&job.id).unwrap();
        }
    }
}
