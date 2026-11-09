fn recorded_round_request() -> (CsvMaterializedApplicationRequest, Vec<LayerTracePair>) {
    let input = r#"
# table:items
item_id,name,shape_type,width,height,depth,weight,amount
i0,Cube,cuboid,1,1,1,1,2
# table:bins
bin_id,type_code,width,height,depth,capacity
b1,BIN,2,1,1,2
# table:layers
layer_id,bin_id,depth,from
l0,b1,1,seed
"#;
    let mut request = crate::application::csv::CsvDatasetLoader::load_str(input).unwrap().materialize().unwrap();
    request.items[0].enabled_orientations = vec![Orientation::Upright];
    let trace = LayerPlacementTrace {
        item_index: 0, item_id: "i0".into(), orientation: Orientation::Upright, amount: 1,
        position: MetricPoint3 { x: meters(0.0), y: meters(0.0), z: meters(0.0) },
    };
    (request, vec![(Vec::new(), vec![trace])])
}

#[test]
fn recorded_round_preserves_timeout_cancellation_and_preflight_failure() {
    use ospf_rust_core::solver::TerminationReason;
    let service = ColumnGenerationApplicationService::default();
    for cancel in [false, true] {
        let (request, traces) = recorded_round_request();
        let options = Bpp3dOneRoundOptions::default();
        if cancel { options.linear.cancellation.cancel("pre-cancelled"); }
        let evidence = service.run_csv_materialized_one_generation_round_with_evidence(
            request, traces, LayerGenerationContext::new(), std::sync::Arc::new(TerminalReportSolver), options,
        ).unwrap();
        assert!(evidence.flow.is_err());
        assert!(evidence.final_stage.is_none());
        assert_eq!(evidence.rmp.len(), 1);
        assert_eq!(evidence.rmp[0].amounts.as_deref(), Some(&[2][..]));
        assert_eq!(evidence.rmp[0].state.layers.len(), 1);
        assert_eq!(evidence.rmp[0].execution.info["status"], "solve_failed");
        let report = &evidence.rmp[0].report.as_ref().unwrap().report;
        assert_eq!(report.termination_reason, if cancel { TerminationReason::Cancelled } else { TerminationReason::TimeLimit });
        assert!(report.solution.is_none());
        assert!(report.proof.is_none());
    }
    let (request, _) = recorded_round_request();
    let evidence = service.run_csv_materialized_one_generation_round_with_evidence(
        request, Vec::new(), LayerGenerationContext::new(), std::sync::Arc::new(TerminalReportSolver), Default::default(),
    ).unwrap();
    assert!(evidence.flow.is_err());
    assert!(evidence.rmp.is_empty());
    assert!(evidence.final_stage.is_none());
    for invalid_limit in [0, 100_001] {
        let (request, traces) = recorded_round_request();
        let mut options = Bpp3dOneRoundOptions::default();
        options.validation.max_placements = invalid_limit;
        let error = service.run_csv_materialized_one_generation_round_with_evidence(
            request, traces, LayerGenerationContext::new(), std::sync::Arc::new(TerminalReportSolver), options,
        ).unwrap_err();
        assert!(error.to_string().contains("invalid strict replay placement limit"));
    }
}

#[test]
fn recorded_round_does_not_accept_a_report_with_missing_model_identity() {
    struct UnidentifiedReportSolver;
    impl ospf_rust_core::solver::SolverInfo for UnidentifiedReportSolver {
        fn name(&self) -> &str { "unidentified-report" }
        fn capabilities(&self) -> Vec<ospf_rust_core::solver::SolverCapability> { vec![ospf_rust_core::solver::SolverCapability::Linear] }
    }
    impl ospf_rust_core::solver::LinearSolver for UnidentifiedReportSolver {
        fn solve_linear(&self, _: &ospf_rust_core::model::intermediate::LinearTriadModel) -> ospf_rust_core::Result<ospf_rust_core::solver::SolverOutput> {
            panic!("report path required")
        }
        fn solve_linear_report_with_options(
            &self,
            _: &ospf_rust_core::model::intermediate::LinearTriadModel,
            _: &ospf_rust_core::solver::SolveOptions<'_>,
        ) -> ospf_rust_core::Result<ospf_rust_core::solver::SolveReport<f64>> {
            ospf_rust_core::solver::SolveReport::builder(
                ospf_rust_core::solver::ProblemStatus::Unknown, ospf_rust_core::solver::TerminationReason::TimeLimit,
            ).build()
        }
    }
    let (request, traces) = recorded_round_request();
    let evidence = ColumnGenerationApplicationService::default().run_csv_materialized_one_generation_round_with_evidence(
        request, traces, LayerGenerationContext::new(), std::sync::Arc::new(UnidentifiedReportSolver), Default::default(),
    ).unwrap();
    assert!(evidence.flow.is_err());
    assert_eq!(evidence.rmp.len(), 1);
    assert!(evidence.rmp[0].report.is_none());
    assert!(evidence.rmp[0].execution.info["error"].contains("fingerprint mismatch"));
    assert!(evidence.final_stage.is_none());
}

#[cfg(all(not(feature = "async"), any(feature = "scip", feature = "gurobi10")))]
fn recorded_actual_round(solver: std::sync::Arc<dyn ospf_rust_core::solver::LinearSolver>) {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use ospf_rust_core::solver::{linear_model_fingerprint, linear_model_mapping, require_optimal_lp_certificate_for_linear_model};

    struct CountingSolver {
        inner: std::sync::Arc<dyn ospf_rust_core::solver::LinearSolver>,
        calls: std::sync::Arc<AtomicUsize>,
        cancel_after: Option<(usize, ospf_rust_core::solver::SolveHandle)>,
    }
    impl ospf_rust_core::solver::SolverInfo for CountingSolver {
        fn name(&self) -> &str { self.inner.name() }
        fn capabilities(&self) -> Vec<ospf_rust_core::solver::SolverCapability> { self.inner.capabilities() }
    }
    impl ospf_rust_core::solver::LinearSolver for CountingSolver {
        fn solve_linear(&self, model: &LinearTriadModel) -> ospf_rust_core::Result<ospf_rust_core::solver::SolverOutput> {
            self.inner.solve_linear(model)
        }
        fn solve_linear_report_with_options(
            &self,
            model: &LinearTriadModel,
            options: &ospf_rust_core::solver::SolveOptions<'_>,
        ) -> ospf_rust_core::Result<ospf_rust_core::solver::SolveReport<f64>> {
            let index = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
            let report = self.inner.solve_linear_report_with_options(model, options);
            if let Some((_, handle)) = self.cancel_after.as_ref().filter(|(number, _)| *number == index) {
                handle.cancel("after-child-solve");
            }
            report
        }
    }
    let calls = std::sync::Arc::new(AtomicUsize::new(0));
    let solver: std::sync::Arc<dyn ospf_rust_core::solver::LinearSolver> = std::sync::Arc::new(CountingSolver {
        inner: solver, calls: calls.clone(), cancel_after: None,
    });
    let service = ColumnGenerationApplicationService::default();
    let (request, traces) = recorded_round_request();
    let mut generation = LayerGenerationContext::new();
    generation.add_generator(Box::new(BlockLayerGenerator::new()));
    let evidence = service.run_csv_materialized_one_generation_round_with_evidence(
        request, traces, generation, solver.clone(), Default::default(),
    ).unwrap();
    let flow = evidence.flow.as_ref().unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 3);
    assert_eq!(evidence.rmp.len(), 2);
    assert_eq!(evidence.rmp[0].state.layers.len(), 1);
    assert!(evidence.rmp[1].state.layers.len() > 1);
    assert_eq!(evidence.rmp[0].state.layers[0].identity_key(), evidence.rmp[1].state.layers[0].identity_key());
    assert!(evidence.rmp.iter().all(|stage| stage.amounts.as_deref() == Some(&[2][..])));
    for stage in &evidence.rmp {
        let record = stage.report.as_ref().unwrap();
        assert_eq!(record.stage, MetaModelStage::RestrictedMaster);
        require_optimal_lp_certificate_for_linear_model(&record.report, &record.model).unwrap();
        assert_eq!(record.report.fingerprints.model, Some(linear_model_fingerprint(&record.model).unwrap()));
        assert_eq!(record.report.model_mapping, Some(linear_model_mapping(&record.model).unwrap()));
    }
    let final_stage = evidence.final_stage.as_ref().unwrap();
    assert_eq!(final_stage.amounts.as_deref(), Some(&[2][..]));
    assert_eq!(
        final_stage.state.layers.iter().map(BinLayer::identity_key).collect::<Vec<_>>(),
        evidence.rmp[1].state.layers.iter().map(BinLayer::identity_key).collect::<Vec<_>>(),
    );
    let selection = final_stage.execution.selection.as_ref().unwrap();
    assert_eq!(selection.assignments.len(), 1);
    let layer_index = selection.assignments[0].layer_index;
    assert!(layer_index >= final_stage.state.initial_layers.len());
    assert_eq!(final_stage.state.layers[layer_index].from, "block_layer_generator");
    let blocks = &final_stage.state.layer_block_traces[&layer_index];
    assert_eq!(blocks.len(), 1);
    assert_eq!((blocks[0].nx, blocks[0].ny, blocks[0].nz, blocks[0].item_count), (2, 1, 1, 2));
    assert_eq!(final_stage.state.layer_placement_traces[&layer_index][0].amount, 2);
    assert_eq!(flow.final_execution.packed_bins[0].items.len(), 2);
    let mut altered = final_stage.state.clone();
    altered.layer_block_traces.get_mut(&layer_index).unwrap()[0].nx = 1;
    assert!(replay_strict_grouped_packing(&altered, selection, &[2], 10_000).is_err());
    let report = final_stage.report.as_ref().unwrap();
    assert_eq!(report.stage, MetaModelStage::FinalMilp);
    assert!(report.model.has_integer_variables() && report.report.has_incumbent());
    assert_eq!(report.report.fingerprints.model, Some(linear_model_fingerprint(&report.model).unwrap()));
    assert_eq!(report.report.model_mapping, Some(linear_model_mapping(&report.model).unwrap()));
    assert_eq!(flow.final_execution.objective, report.report.solution.as_ref().unwrap().objective_value.map(|value| value + report.objective_constant));

    let (request, mut traces) = recorded_round_request();
    let duplicate = traces[0].1[0].clone();
    traces[0].1.push(duplicate);
    let failed = service.run_csv_materialized_one_generation_round_with_evidence(
        request, traces, LayerGenerationContext::new(), solver.clone(), Default::default(),
    ).unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 6);
    assert!(failed.flow.is_err());
    assert_eq!(failed.rmp.len(), 2);
    let final_stage = failed.final_stage.unwrap();
    assert_eq!(final_stage.execution.execution.info["status"], "physical_validation_failed");
    assert!(!final_stage.execution.execution.final_solved);
    assert!(final_stage.execution.execution.packed_bins.is_empty());
    assert_eq!(final_stage.execution.selection.unwrap().assignments.len(), 1);
    assert!(final_stage.report.unwrap().report.has_incumbent());

    let (request, traces) = recorded_round_request();
    let infeasible = service.run_csv_materialized_one_generation_round_with_evidence(
        request, traces, LayerGenerationContext::new(), solver.clone(), Default::default(),
    ).unwrap();
    assert!(infeasible.flow.is_err());
    assert_eq!(infeasible.rmp.len(), 2);
    let final_stage = infeasible.final_stage.unwrap();
    assert!(final_stage.execution.selection.is_none());
    let report = final_stage.report.unwrap();
    assert_eq!(report.report.problem_status, ospf_rust_core::solver::ProblemStatus::Infeasible);
    assert!(report.report.solution.is_none());
    ospf_rust_core::solver::require_infeasibility_certificate_for_model(
        &report.report, &linear_model_fingerprint(&report.model).unwrap(),
    ).unwrap();

    for cancel_after in [1, 2] {
        let child_calls = std::sync::Arc::new(AtomicUsize::new(0));
        let options = Bpp3dOneRoundOptions::default();
        let cancelling_solver = std::sync::Arc::new(CountingSolver {
            inner: solver.clone(), calls: child_calls.clone(),
            cancel_after: Some((cancel_after, options.linear.cancellation.clone())),
        });
        let (request, traces) = recorded_round_request();
        let evidence = service.run_csv_materialized_one_generation_round_with_evidence(
            request, traces, LayerGenerationContext::new(), cancelling_solver, options,
        ).unwrap();
        assert_eq!(child_calls.load(Ordering::SeqCst), cancel_after);
        assert!(evidence.flow.is_err());
        assert_eq!(evidence.rmp.len(), 2);
        assert!(evidence.rmp[0].report.as_ref().unwrap().report.has_incumbent());
        let cancelled = if cancel_after == 1 {
            assert!(evidence.final_stage.is_none());
            &evidence.rmp[1].report.as_ref().unwrap().report
        } else {
            assert!(evidence.rmp[1].report.as_ref().unwrap().report.has_incumbent());
            let final_stage = evidence.final_stage.as_ref().unwrap();
            assert!(final_stage.execution.selection.is_none());
            &final_stage.report.as_ref().unwrap().report
        };
        assert_eq!(cancelled.termination_reason, ospf_rust_core::solver::TerminationReason::Cancelled);
        assert!(cancelled.solution.is_none());
    }

    let (request, traces) = recorded_round_request();
    let mut options = Bpp3dOneRoundOptions::default();
    options.linear.report_limit = 1;
    let limited = service.run_csv_materialized_one_generation_round_with_evidence(
        request, traces, LayerGenerationContext::new(), solver, options,
    ).unwrap();
    assert!(limited.flow.is_err());
    assert_eq!(limited.rmp.len(), 2);
    assert!(limited.rmp[0].report.is_some());
    assert!(limited.rmp[1].report.is_none());
    assert!(limited.rmp[1].execution.info["error"].contains("report limit"));
    assert!(limited.final_stage.is_none());
}

#[cfg(all(feature = "scip", not(feature = "async")))]
#[test]
fn recorded_scip_round_keeps_actual_generated_block_and_failure_evidence() {
    use ospf_rust_core_plugin_scip::{SCIPConfig, SCIPSolver};
    recorded_actual_round(std::sync::Arc::new(SCIPSolver::with_config(
        SCIPConfig::default().with_output(false).with_lp_subproblem_defaults(),
    )));
}

#[cfg(all(feature = "gurobi10", not(feature = "async")))]
#[test]
fn recorded_gurobi_round_keeps_actual_generated_block_and_failure_evidence() {
    use ospf_rust_core::solver::solvers::gurobi::{GurobiConfig, GurobiSolver};
    recorded_actual_round(std::sync::Arc::new(GurobiSolver::with_config(GurobiConfig::default().with_output(false))));
}
