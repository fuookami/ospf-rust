    struct TerminalReportSolver;

    impl ospf_rust_core::solver::SolverInfo for TerminalReportSolver {
        fn name(&self) -> &str { "bpp3d-terminal-contract" }
        fn capabilities(&self) -> Vec<ospf_rust_core::solver::SolverCapability> { vec![ospf_rust_core::solver::SolverCapability::Linear] }
    }

    impl ospf_rust_core::solver::LinearSolver for TerminalReportSolver {
        fn solve_linear(&self, _: &ospf_rust_core::model::intermediate::LinearTriadModel) -> ospf_rust_core::Result<ospf_rust_core::solver::SolverOutput> {
            panic!("report path required")
        }

        fn solve_linear_report_with_options(
            &self,
            model: &ospf_rust_core::model::intermediate::LinearTriadModel,
            _: &ospf_rust_core::solver::SolveOptions<'_>,
        ) -> ospf_rust_core::Result<ospf_rust_core::solver::SolveReport<f64>> {
            use ospf_rust_core::solver::{ProblemStatus, SolveFingerprints, SolveReport, TerminationReason, linear_model_fingerprint, linear_model_mapping};
            SolveReport::builder(ProblemStatus::Unknown, TerminationReason::TimeLimit)
                .fingerprints(SolveFingerprints { model: Some(linear_model_fingerprint(model)?), ..Default::default() })
                .model_mapping(Some(linear_model_mapping(model)?)).build()
        }
    }

    fn report_backend_state() -> ColumnGenerationApplicationState {
        let bin = bin_type();
        let layer = BinLayer { iteration: 0, from: "report-seed".into(), bin: Some(bin.clone()), depth: meters(4.0),
            demand_coverage: vec![Bpp3dLayerDemandCoverage::new(Bpp3dDemandMode::Item, Bpp3dDemandKey::Item { id: "i0".into() }, 1.0)] };
        ColumnGenerationApplicationState { items: vec![item("i0")], bins: vec![bin], initial_layers: vec![layer.clone()], layers: vec![layer],
            iteration: 0, layer_block_traces: HashMap::new(), layer_placement_traces: HashMap::new(), continuous_radius_component: None,
            additional_shadow_prices: HashMap::new(), info: HashMap::new() }
    }

    #[test]
    fn report_backend_terminals_never_fall_back_to_noop_vectors_or_generate_selected_layers() {
        use std::sync::Arc;
        use ospf_rust_framework::solver::{LinearExecutionOptions, MetaModelStage, ReportMetaModelBackend};
        let backend = ReportMetaModelBackend::new(Arc::new(TerminalReportSolver), LinearExecutionOptions::default()).unwrap();
        let state = report_backend_state();
        let rmp = SolverBackedMetaModelRmpExecutor::new(MetaModelRmpExecutorConfig {
            primal_solution: vec![1.0], shadow_prices: vec![400.0], objective: Some(400.0), ..Default::default()
        }, backend.clone());
        let final_executor = SolverBackedMetaModelFinalExecutor::new(MetaModelFinalExecutorConfig {
            primal_solution: vec![1.0, 1.0], objective: Some(1000.000001), ..Default::default()
        }, backend.clone());
        assert!(rmp.execute_result(&state).is_err());
        let final_execution = final_executor.execute(&state);
        assert_eq!(final_execution.info["status"], "solve_failed"); assert!(!final_execution.final_solved);
        assert!(final_execution.layers.is_empty()); assert!(final_execution.packed_bins.is_empty()); assert!(final_execution.objective.is_none());
        let reports = backend.reports(); assert_eq!(reports.len(), 2);
        assert_eq!(reports[0].stage, MetaModelStage::RestrictedMaster); assert_eq!(reports[1].stage, MetaModelStage::FinalMilp);
        assert!(reports.iter().all(|record| record.report.solution.is_none() && record.report.proof.is_none()));
        assert!(reports[1].model.has_integer_variables()); assert!(!reports[0].model.has_integer_variables());
    }

    #[test]
    fn report_backend_precancellation_keeps_original_child_terminal_and_stops_application_before_final() {
        use std::sync::Arc;
        use ospf_rust_framework::solver::{LinearExecutionOptions, ReportMetaModelBackend};
        let options = LinearExecutionOptions::default(); options.cancellation.cancel("before-bpp3d-rmp");
        let backend = ReportMetaModelBackend::new(Arc::new(TerminalReportSolver), options).unwrap(); let state = report_backend_state();
        let rmp = SolverBackedMetaModelRmpExecutor::new(Default::default(), backend.clone());
        let final_executor = SolverBackedMetaModelFinalExecutor::new(Default::default(), backend.clone());
        let result = ColumnGenerationApplicationService::default().run_materialized_with_bins(state.items, state.bins, state.layers, &rmp, &final_executor);
        assert!(result.is_err()); let reports = backend.reports(); assert_eq!(reports.len(), 1);
        assert_eq!(reports[0].report.termination_reason, ospf_rust_core::solver::TerminationReason::Cancelled);
        assert!(reports[0].report.solution.is_none()); assert!(reports[0].report.proof.is_none());
    }

    #[cfg(all(feature = "serde", not(feature = "async"), any(feature = "gurobi10", feature = "scip")))]
    fn run_actual_report_backend(solver: std::sync::Arc<dyn ospf_rust_core::solver::LinearSolver>) {
        use ospf_rust_core::solver::{linear_model_fingerprint, linear_model_mapping, require_optimal_lp_certificate_for_linear_model};
        use ospf_rust_framework::solver::{LinearExecutionOptions, MetaModelStage, ReportMetaModelBackend};
        let backend = ReportMetaModelBackend::new(solver, LinearExecutionOptions { time_limit: Duration::from_secs(30), report_limit: 8, ..Default::default() }).unwrap();
        let rmp = SolverBackedMetaModelRmpExecutor::new(Default::default(), backend.clone());
        let final_executor = SolverBackedMetaModelFinalExecutor::new(Default::default(), backend.clone());
        let service = ColumnGenerationApplicationService::new(ColumnGenerationConfig { max_iterations: 1, max_column_amount: 64, max_candidates_per_iteration: 8, ..Default::default() });
        let result = service.run_csv_materialized_one_generation_round(real_solver_smoke_request(),
            ColumnGenerationStandardExecutors::default_layer_generation_context(), &rmp, &final_executor).unwrap();
        assert_eq!(result.rmp.info["backend_kind"], "linear_report"); assert_eq!(result.final_execution.info["backend_kind"], "linear_report");
        let reports = backend.reports(); assert_eq!(reports.len(), 3);
        for record in &reports {
            assert_eq!(record.report.fingerprints.model, Some(linear_model_fingerprint(&record.model).unwrap()));
            assert_eq!(record.report.model_mapping, Some(linear_model_mapping(&record.model).unwrap()));
            assert_eq!(record.report.solution.as_ref().unwrap().values.len(), record.model.num_variables());
        }
        for record in &reports[..2] {
            assert_eq!(record.stage, MetaModelStage::RestrictedMaster);
            require_optimal_lp_certificate_for_linear_model(&record.report, &record.model).unwrap();
        }
        assert_eq!(reports[2].stage, MetaModelStage::FinalMilp); assert!(reports[2].model.has_integer_variables());
        assert_eq!(result.final_execution.objective, reports[2].report.solution.as_ref().unwrap().objective_value.map(|value| value + reports[2].objective_constant));
        assert!(result.final_execution.final_solved); assert!(!result.final_execution.layers.is_empty());
        assert!(!result.result.render_loading_plans.is_empty()); assert!(backend.remaining_time() < Duration::from_secs(30));
    }

    #[cfg(all(feature = "serde", feature = "scip", not(feature = "async")))]
    #[test]
    fn actual_scip_report_backend_preserves_bpp3d_lp_certificates_generation_refresh_and_final_reports() {
        use ospf_rust_core::solver::solvers::scip::{SCIPConfig, SCIPSolver};
        run_actual_report_backend(std::sync::Arc::new(SCIPSolver::with_config(SCIPConfig::default().with_output(false).with_lp_subproblem_defaults())));
    }

    #[cfg(all(feature = "serde", feature = "gurobi10", not(feature = "async")))]
    #[test]
    fn actual_gurobi_report_backend_preserves_bpp3d_lp_certificates_generation_refresh_and_final_reports() {
        use ospf_rust_core::solver::solvers::gurobi::{GurobiConfig, GurobiSolver};
        run_actual_report_backend(std::sync::Arc::new(GurobiSolver::with_config(GurobiConfig::default().with_output(false))));
    }
