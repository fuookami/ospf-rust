fn traced_final_state() -> ColumnGenerationApplicationState {
    let mut state = report_backend_state();
    state.bins.push(state.bins[0].clone()); state.layers.push(state.layers[0].clone());
    state.items.push(item("i1"));
    state.layers[1].demand_coverage = vec![Bpp3dLayerDemandCoverage::new(Bpp3dDemandMode::Item, Bpp3dDemandKey::Item { id: "i1".into() }, 1.0)];
    let trace = LayerPlacementTrace { item_index: 0, item_id: state.items[0].id.clone(), orientation: Orientation::Upright, amount: 1,
        position: crate::infrastructure::geometry::MetricPoint3 { x: meters(0.0), y: meters(0.0), z: meters(0.0) } };
    let mut second = trace.clone(); second.item_index = 1; second.item_id = state.items[1].id.clone();
    state.layer_placement_traces = HashMap::from([(0, vec![trace]), (1, vec![second])]); state
}

#[derive(Debug)]
struct FixedTraceGenerator(Vec<LayerGenerationResult<f64, Meter>>);

impl crate::domain::layer_generation::LayerGenerator<f64, Meter> for FixedTraceGenerator {
    fn name(&self) -> &str { "fixed-trace-test" }

    fn generate(&self, _: &LayerGenerationRequest<f64, Meter>) -> Vec<LayerGenerationResult<f64, Meter>> {
        self.0.clone()
    }
}

fn generated_trace_candidate(
    layer: BinLayer<f64, Meter>,
    item_index: usize,
    x: f64,
) -> LayerGenerationResult<f64, Meter> {
    LayerGenerationResult {
        layer, reduced_cost: None, score: None, numeric_score: None, block_traces: Vec::new(),
        placement_traces: vec![LayerPlacementTrace {
            item_index, item_id: format!("i{}", item_index).into(), orientation: Orientation::Upright, amount: 1,
            position: MetricPoint3 { x: meters(x), y: meters(0.0), z: meters(0.0) },
        }],
        diagnostics: Vec::new(), source: "fixed-trace-test".into(),
    }
}

#[derive(Debug, Default)]
struct CaptureGeneratedTraceFinal(std::sync::Mutex<Option<ColumnGenerationApplicationState>>);

impl ColumnGenerationFinalExecutor for CaptureGeneratedTraceFinal {
    fn execute(&self, state: &ColumnGenerationApplicationState) -> ColumnGenerationFinalExecution {
        *self.0.lock().unwrap() = Some(state.clone());
        MockColumnGenerationFinalExecutor.execute(state)
    }
}

#[derive(Debug)]
struct TracedFinalReportSolver;

#[derive(Debug)]
struct OneBinPackingExtension;

impl ColumnGenerationFinalModelExtension for OneBinPackingExtension {
    fn register(&self, _: &ColumnGenerationApplicationState, model: &mut MetaModel<f64>) -> Result<(), String> {
        for (layer, coefficient) in [7.0, 11.0].into_iter().enumerate() {
            let name = format!("x_0_{}", layer);
            let index = model.tokens().iter().find(|token| token.name() == name)
                .ok_or("consumer extension lost original assignment variables")?.solver_index;
            model.add_eq_constraint(&[(index, coefficient)], coefficient, &format!("consumer_one_bin_{}", layer))
                .map_err(|error| error.to_string())?;
        }
        Ok(())
    }
}

#[derive(Debug)]
struct ForbidFirstAssignmentExtension;

impl ColumnGenerationFinalModelExtension for ForbidFirstAssignmentExtension {
    fn register(&self, _: &ColumnGenerationApplicationState, model: &mut MetaModel<f64>) -> Result<(), String> {
        let index = model.tokens().iter().find(|token| token.name() == "x_0_0")
            .ok_or("consumer extension lost first assignment variable")?.solver_index;
        model.add_eq_constraint(&[(index, 1.0)], 0.0, "consumer_forbid_first")
            .map_err(|error| error.to_string())?;
        Ok(())
    }
}

impl ospf_rust_core::solver::SolverInfo for TracedFinalReportSolver {
    fn name(&self) -> &str { "traced-final-contract" }
    fn capabilities(&self) -> Vec<ospf_rust_core::solver::SolverCapability> { vec![ospf_rust_core::solver::SolverCapability::Linear] }
}

impl ospf_rust_core::solver::LinearSolver for TracedFinalReportSolver {
    fn solve_linear(&self, _: &ospf_rust_core::model::intermediate::LinearTriadModel) -> ospf_rust_core::Result<ospf_rust_core::solver::SolverOutput> { panic!("report path required") }

    fn solve_linear_report_with_options(
        &self,
        model: &ospf_rust_core::model::intermediate::LinearTriadModel,
        _: &ospf_rust_core::solver::SolveOptions<'_>,
    ) -> ospf_rust_core::Result<ospf_rust_core::solver::SolveReport<f64>> {
        use ospf_rust_core::solver::{ProblemStatus, SolveFingerprints, SolveReport, SolveSolution, TerminationReason, linear_model_fingerprint, linear_model_mapping};
        let values = model.basic.variables.iter().map(|token| if matches!(token.name(), "x_0_0" | "x_0_1" | "v_0") { 1.0 } else { 0.0 }).collect::<Vec<_>>();
        let mut solution = SolveSolution::vector(values.clone()); solution.objective_value = Some(model.c.iter().zip(values).map(|(coefficient, value)| coefficient * value).sum());
        SolveReport::builder(ProblemStatus::Feasible, TerminationReason::TimeLimit).solution(solution)
            .fingerprints(SolveFingerprints { model: Some(linear_model_fingerprint(model)?), ..Default::default() })
            .model_mapping(Some(linear_model_mapping(model)?)).build()
    }
}

#[test]
fn traced_final_uses_original_model_report_and_preserves_assignments_without_noop_fallback() {
    let backend = ReportMetaModelBackend::new(std::sync::Arc::new(TracedFinalReportSolver), Default::default()).unwrap();
    let executor = TraceValidatedMetaModelFinalExecutor::new(MetaModelFinalExecutorConfig {
        primal_solution: vec![0.0], objective: Some(-999.0), ..Default::default()
    }, backend.clone());
    let state = traced_final_state(); let result = executor.execute_with_selection(&state, Some(&OneBinPackingExtension));
    assert!(result.execution.final_solved, "{:?}", result.execution.info);
    let selection = result.selection.unwrap(); assert_eq!(selection.assignments.len(), 2); assert_eq!(selection.activated_bins, vec![0]);
    assert_eq!(result.execution.layers.len(), 2); assert_eq!(result.execution.packed_bins.len(), 1);
    for bin in &result.execution.packed_bins { assert_eq!(bin.items.len(), 2); assert_eq!(bin.items[1].position.z.value, 4.0); }
    assert_eq!(result.execution.info["status"], "physically_validated"); assert_ne!(result.execution.objective, Some(-999.0));
    let reports = backend.reports(); assert_eq!(reports.len(), 1); assert!(reports[0].report.proof.is_none());
    let model = &reports[0].model.basic;
    for (layer, coefficient) in [7.0, 11.0].into_iter().enumerate() {
        let index = model.variables.iter().position(|token| token.name() == format!("x_0_{}", layer)).unwrap();
        for sign in [1.0, -1.0] {
            assert!(model.A.rows.iter().zip(&model.b).any(|(row, rhs)|
                row.entries == [(index, sign * coefficient)] && *rhs == sign * coefficient),
                "missing consumer constraint for x_0_{layer}, sign {sign}: {:?}", model.A.rows);
        }
    }
    assert_eq!(reports[0].report.termination_reason, ospf_rust_core::solver::TerminationReason::TimeLimit);
    assert_eq!(result.execution.objective, reports[0].report.solution.as_ref().unwrap().objective_value);
}

#[test]
fn grouped_traced_final_retains_actual_selection_and_rejects_invalid_inputs() {
    let backend = ReportMetaModelBackend::new(std::sync::Arc::new(TracedFinalReportSolver), Default::default()).unwrap();
    let executor = TraceValidatedMetaModelFinalExecutor::new(Default::default(), backend.clone());
    let mut state = traced_final_state();
    state.items.truncate(1);
    state.layers[1].demand_coverage = state.layers[0].demand_coverage.clone();
    state.layer_placement_traces.insert(1, state.layer_placement_traces[&0].clone());

    let result = executor.execute_with_selection_and_item_amounts(&state, &[2], Some(&OneBinPackingExtension));
    assert!(result.execution.final_solved, "{:?}", result.execution.info);
    let selection = result.selection.unwrap();
    assert_eq!(selection.activated_bins, vec![0]);
    assert_eq!(selection.assignments.iter().map(|entry| (entry.bin_index, entry.layer_index)).collect::<Vec<_>>(), vec![(0, 0), (0, 1)]);
    assert_eq!(result.execution.packed_bins[0].items.len(), 2);
    assert!(result.execution.packed_bins[0].items.iter().all(|item| item.item.id.as_str() == "i0"));
    assert_eq!(backend.reports().len(), 1);

    state.layer_placement_traces.remove(&1);
    let invalid_trace = executor.execute_with_selection_and_item_amounts(&state, &[2], None);
    assert!(!invalid_trace.execution.final_solved);
    assert_eq!(invalid_trace.execution.info["status"], "physical_validation_failed");
    assert!(invalid_trace.execution.packed_bins.is_empty());
    assert_eq!(invalid_trace.selection.unwrap(), selection);
    assert_eq!(backend.reports().len(), 2);

    let invalid_demand = executor.execute_with_selection_and_item_amounts(&state, &[], None);
    assert!(!invalid_demand.execution.final_solved);
    assert_eq!(invalid_demand.execution.info["status"], "registration_failed");
    assert!(invalid_demand.selection.is_none());
    assert_eq!(backend.reports().len(), 2);
}

#[test]
fn traced_final_rejects_incumbent_that_violates_consumer_extension() {
    let backend = ReportMetaModelBackend::new(std::sync::Arc::new(TracedFinalReportSolver), Default::default()).unwrap();
    let executor = TraceValidatedMetaModelFinalExecutor::new(Default::default(), backend.clone());
    let result = executor.execute_with_selection(&traced_final_state(), Some(&ForbidFirstAssignmentExtension));
    assert!(!result.execution.final_solved);
    assert!(result.selection.is_none());
    assert_eq!(result.execution.info["status"], "solve_failed");
    assert!(result.execution.info["error"].contains("violates exact row"));
    assert!(backend.reports().is_empty());
}

#[test]
fn traced_final_physical_failure_retains_selection_and_report_without_publishing_partial_packing() {
    let backend = ReportMetaModelBackend::new(std::sync::Arc::new(TracedFinalReportSolver), Default::default()).unwrap();
    let executor = TraceValidatedMetaModelFinalExecutor::new(Default::default(), backend.clone());
    let mut state = traced_final_state(); state.layer_placement_traces.remove(&1);
    let result = executor.execute_with_selection(&state, None);
    assert!(!result.execution.final_solved); assert!(result.execution.packed_bins.is_empty()); assert!(result.execution.layers.is_empty());
    assert!(result.execution.objective.is_none()); assert_eq!(result.selection.unwrap().assignments.len(), 2);
    assert_eq!(result.execution.info["status"], "physical_validation_failed"); assert!(result.execution.info["error"].contains("coverage fallback forbidden"));
    assert_eq!(backend.reports().len(), 1); assert!(backend.reports()[0].report.has_incumbent());
}

#[test]
fn application_propagates_traced_final_physical_validation_failure() {
    let backend = ReportMetaModelBackend::new(std::sync::Arc::new(TracedFinalReportSolver), Default::default()).unwrap();
    let executor = TraceValidatedMetaModelFinalExecutor::new(Default::default(), backend.clone());
    let state = traced_final_state();
    let result = ColumnGenerationApplicationService::default().run_materialized_with_bins(
        state.items, state.bins, state.layers, &MockColumnGenerationRmpExecutor::default(), &executor,
    );
    let errors = result.unwrap_err();
    assert!(errors.iter().any(|error| error.contains("FinalMilp") && error.contains("coverage fallback forbidden")));
    assert_eq!(backend.reports().len(), 1);
}

#[test]
fn application_strict_final_replays_each_generated_layer_own_trace() {
    let backend = ReportMetaModelBackend::new(std::sync::Arc::new(TracedFinalReportSolver), Default::default()).unwrap();
    let executor = TraceValidatedMetaModelFinalExecutor::new(Default::default(), backend.clone());
    let mut state = traced_final_state();
    for layer in &mut state.layers { layer.from = "same-source".into(); }
    let candidates = state.layers.iter().cloned().enumerate()
        .map(|(index, layer)| generated_trace_candidate(layer, index, 0.0)).collect();
    let mut generation = LayerGenerationContext::new();
    generation.add_generator(Box::new(FixedTraceGenerator(candidates)));
    let result = ColumnGenerationApplicationService::default().run_materialized_one_generation_round(
        state.items, state.bins, Vec::new(), generation, &MockColumnGenerationRmpExecutor::default(), &executor,
    ).unwrap();
    assert!(result.final_execution.final_solved, "{:?}", result.final_execution.info);
    let packed = &result.final_execution.packed_bins[0].items;
    assert_eq!(packed.len(), 2);
    assert_eq!(packed[0].item.id.as_str(), "i0");
    assert_eq!(packed[1].item.id.as_str(), "i1");
    assert_eq!(packed[1].position.z.value, 4.0);
    assert_eq!(backend.reports().len(), 1);
}

#[test]
fn application_strict_final_accepts_traced_initial_layers_and_keeps_them_after_generation() {
    for generate in [false, true] {
        let backend = ReportMetaModelBackend::new(std::sync::Arc::new(TracedFinalReportSolver), Default::default()).unwrap();
        let executor = TraceValidatedMetaModelFinalExecutor::new(Default::default(), backend.clone());
        let state = traced_final_state();
        let first = generated_trace_candidate(state.layers[0].clone(), 0, 0.0);
        let second = generated_trace_candidate(state.layers[1].clone(), 1, 0.0);
        let service = ColumnGenerationApplicationService::default();
        let result = if generate {
            let mut generation = LayerGenerationContext::new();
            generation.add_generator(Box::new(FixedTraceGenerator(vec![second])));
            service.run_materialized_one_generation_round_with_traced_initial_layers(
                state.items, state.bins, vec![first], generation, &MockColumnGenerationRmpExecutor::default(), &executor,
            )
        } else {
            service.run_materialized_with_traced_initial_layers(
                state.items, state.bins, vec![first, second], &MockColumnGenerationRmpExecutor::default(), &executor,
            )
        }.unwrap();
        assert!(result.final_execution.final_solved, "{:?}", result.final_execution.info);
        let packed = &result.final_execution.packed_bins[0].items;
        assert_eq!(packed.len(), 2);
        assert_eq!(packed[0].item.id.as_str(), "i0");
        assert_eq!(packed[1].item.id.as_str(), "i1");
        assert_eq!(packed[1].position.z.value, 4.0);
        assert_eq!(backend.reports().len(), 1);
    }
}

#[cfg(feature = "serde")]
fn traced_csv_request() -> CsvMaterializedApplicationRequest {
    let input = r#"
# table:items
item_id,name,shape_type,width,height,depth,weight,amount
i0,First,cuboid,2,3,4,1,1
i1,Second,cuboid,2,3,4,1,1
# table:bins
bin_id,type_code,width,height,depth,capacity
b1,BIN,10,10,10,1000
# table:layers
layer_id,bin_id,depth,from
l0,b1,4,first
l1,b1,4,second
"#;
    crate::application::csv::CsvDatasetLoader::load_str(input).unwrap().materialize().unwrap()
}

#[cfg(feature = "serde")]
fn traced_csv_initial_traces(request: &CsvMaterializedApplicationRequest) -> Vec<LayerTracePair> {
    request.initial_layers.iter().cloned().enumerate().map(|(index, layer)| {
        let result = generated_trace_candidate(layer, index, 0.0);
        (result.block_traces, result.placement_traces)
    }).collect()
}

#[cfg(feature = "serde")]
fn grouped_traced_csv_request() -> (CsvMaterializedApplicationRequest, Vec<LayerTracePair>) {
    let mut request = traced_csv_request();
    request.items.truncate(1);
    request.item_amounts.truncate(1);
    request.item_amounts[0].1 = 2;
    request.initial_layers.truncate(1);
    let mut traces = traced_csv_initial_traces(&request);
    let mut second = traces[0].1[0].clone();
    second.position.x = meters(2.0);
    traces[0].1.push(second);
    (request, traces)
}

#[cfg(feature = "serde")]
#[test]
fn csv_traced_round_preserves_grouped_item_demand_through_generation_and_final() {
    #[derive(Debug)]
    struct DemandProbe(std::sync::Arc<std::sync::Mutex<Vec<f64>>>);

    impl crate::domain::layer_generation::LayerGenerator<f64, Meter> for DemandProbe {
        fn name(&self) -> &str { "grouped-demand-probe" }
        fn generate(&self, request: &LayerGenerationRequest<f64, Meter>) -> Vec<LayerGenerationResult<f64, Meter>> {
            *self.0.lock().unwrap() = request.demand_entries.iter().map(|entry| entry.demand).collect();
            Vec::new()
        }
    }

    let (request, traces) = grouped_traced_csv_request();
    let observed = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let mut generation = LayerGenerationContext::new();
    generation.add_generator(Box::new(DemandProbe(observed.clone())));
    let backend = ReportMetaModelBackend::new(std::sync::Arc::new(TracedFinalReportSolver), Default::default()).unwrap();
    let final_executor = TraceValidatedMetaModelFinalExecutor::new(Default::default(), backend.clone());
    let rmp = MetaModelRmpExecutor::new(MetaModelRmpExecutorConfig {
        shadow_prices: vec![1.0], primal_solution: vec![1.0], ..Default::default()
    });
    let result = ColumnGenerationApplicationService::default().run_csv_materialized_one_generation_round_with_traced_initial_layers(
        request, traces, generation, &rmp, &final_executor,
    ).unwrap();
    assert_eq!(*observed.lock().unwrap(), vec![2.0]);
    assert!(result.final_execution.final_solved);
    let packed = &result.final_execution.packed_bins[0].items;
    assert_eq!(packed.len(), 2);
    assert_eq!(packed[0].item.id.as_str(), "i0");
    assert_eq!(packed[1].item.id.as_str(), "i0");
    let reports = backend.reports();
    let model = &reports[0].model.basic;
    let index = model.variables.iter().position(|token| token.name() == "x_0_0").unwrap();
    for sign in [1.0, -1.0] {
        assert!(model.A.rows.iter().zip(&model.b).any(|(row, rhs)|
            row.entries == [(index, sign * 2.0)] && *rhs == sign * 2.0));
    }
}

#[cfg(feature = "serde")]
#[test]
fn csv_traced_round_matches_grouped_amounts_by_id_not_input_order() {
    let mut request = traced_csv_request();
    request.item_amounts[0].1 = 2;
    request.item_amounts.reverse();
    let mut traces = traced_csv_initial_traces(&request);
    let mut second = traces[0].1[0].clone();
    second.position.x = meters(2.0);
    traces[0].1.push(second);
    let backend = ReportMetaModelBackend::new(std::sync::Arc::new(TracedFinalReportSolver), Default::default()).unwrap();
    let final_executor = TraceValidatedMetaModelFinalExecutor::new(Default::default(), backend);
    let rmp = MetaModelRmpExecutor::new(MetaModelRmpExecutorConfig {
        shadow_prices: vec![1.0, 1.0], primal_solution: vec![1.0, 1.0], ..Default::default()
    });
    let result = ColumnGenerationApplicationService::default().run_csv_materialized_one_generation_round_with_traced_initial_layers(
        request, traces, LayerGenerationContext::new(), &rmp, &final_executor,
    ).unwrap();
    let packed = &result.final_execution.packed_bins[0].items;
    assert_eq!(packed.iter().map(|item| item.item.id.as_str()).collect::<Vec<_>>(), vec!["i0", "i0", "i1"]);
}

#[cfg(feature = "serde")]
#[test]
fn csv_strict_final_derives_each_initial_layer_coverage_from_its_trace() {
    let request = traced_csv_request();
    let traces = traced_csv_initial_traces(&request);
    let backend = ReportMetaModelBackend::new(std::sync::Arc::new(TracedFinalReportSolver), Default::default()).unwrap();
    let executor = TraceValidatedMetaModelFinalExecutor::new(Default::default(), backend.clone());
    let result = ColumnGenerationApplicationService::default().run_csv_materialized_one_generation_round_with_traced_initial_layers(
        request, traces, LayerGenerationContext::new(), &MockColumnGenerationRmpExecutor::default(), &executor,
    ).unwrap();
    assert!(result.final_execution.final_solved, "{:?}", result.final_execution.info);
    let packed = &result.final_execution.packed_bins[0].items;
    assert_eq!(packed.len(), 2);
    assert_eq!(packed[0].item.id.as_str(), "i0");
    assert_eq!(packed[1].item.id.as_str(), "i1");
    assert_eq!(packed[1].position.z.value, 4.0);
    assert_eq!(backend.reports().len(), 1);
}

#[cfg(feature = "serde")]
#[test]
fn csv_traced_initial_layers_reject_invalid_demands_and_trace_inputs_before_solving() {
    #[derive(Debug)]
    struct NoSolve;
    impl ColumnGenerationRmpExecutor for NoSolve {
        fn execute(&self, _: &ColumnGenerationApplicationState) -> ColumnGenerationRmpExecution { panic!("invalid input reached RMP") }
    }
    for case in 0..12 {
        let mut request = traced_csv_request();
        let mut traces = traced_csv_initial_traces(&request);
        match case {
            0 => request.item_amounts[0].1 = 2,
            1 => request.item_amounts[0].1 = 0,
            2 => request.item_amounts[1].0 = request.item_amounts[0].0.clone(),
            3 => { request.item_amounts.pop(); },
            4 => { traces.pop(); },
            5 => traces[0].1[0].item_index = request.items.len(),
            6 => traces[0].1[0].item_id = "wrong".into(),
            7 => traces[0].1[0].amount = 0,
            8 => traces[0].1[0].amount = 9_007_199_254_740_993,
            9 => request.item_amounts[0].1 = 9_007_199_254_740_993,
            10 => request.item_amounts[0].1 = 100_001,
            _ => request.items[1].id = request.items[0].id.clone(),
        }
        let result = ColumnGenerationApplicationService::default().run_csv_materialized_one_generation_round_with_traced_initial_layers(
            request, traces, LayerGenerationContext::new(), &NoSolve, &MockColumnGenerationFinalExecutor,
        );
        assert!(result.is_err(), "case {case}");
    }
}

#[test]
fn grouped_executor_boundary_rejects_invalid_or_unsupported_demands() {
    let mut state = report_backend_state();
    let rmp = MetaModelRmpExecutor::default();
    let backend = ReportMetaModelBackend::new(std::sync::Arc::new(TracedFinalReportSolver), Default::default()).unwrap();
    let strict_final = TraceValidatedMetaModelFinalExecutor::new(Default::default(), backend.clone());
    for amounts in [&[][..], &[0][..], &[9_007_199_254_740_993][..]] {
        assert!(rmp.execute_result_with_item_amounts(&state, amounts).is_err());
        assert!(strict_final.execute_result_with_item_amounts(&state, amounts).is_err());
    }
    assert!(backend.reports().is_empty());
    let mock_rmp = MockColumnGenerationRmpExecutor::default();
    let mock_final = MockColumnGenerationFinalExecutor;
    assert!(mock_rmp.execute_result_with_item_amounts(&state, &[2]).unwrap_err().message.contains("does not support"));
    assert!(mock_final.execute_result_with_item_amounts(&state, &[2]).unwrap_err().message.contains("does not support"));
    assert!(mock_rmp.execute_result_with_item_amounts(&state, &[1]).is_ok());
    assert!(mock_final.execute_result_with_item_amounts(&state, &[1]).is_ok());
    state.items.push(state.items[0].clone());
    assert!(rmp.execute_result_with_item_amounts(&state, &[1, 1]).is_err());
    assert!(strict_final.execute_result_with_item_amounts(&state, &[1, 1]).is_err());
}

#[cfg(feature = "serde")]
#[test]
fn csv_traced_round_keeps_package_and_continuous_radius_context() {
    #[derive(Debug, Default)]
    struct RadiusProbe(std::sync::Mutex<Vec<bool>>);
    impl ColumnGenerationRmpExecutor for RadiusProbe {
        fn execute(&self, state: &ColumnGenerationApplicationState) -> ColumnGenerationRmpExecution {
            self.0.lock().unwrap().push(state.continuous_radius_component.is_some());
            MockColumnGenerationRmpExecutor::default().execute(state)
        }
    }
    #[derive(Debug)]
    struct PackageProbe;
    impl crate::domain::layer_generation::LayerGenerator<f64, Meter> for PackageProbe {
        fn name(&self) -> &str { "package-probe" }
        fn generate(&self, request: &LayerGenerationRequest<f64, Meter>) -> Vec<LayerGenerationResult<f64, Meter>> {
            assert_eq!(request.package_attributes["c1"].allow_mixed_loading, Some(false));
            Vec::new()
        }
    }
    let input = r#"
# table:items
item_id,name,shape_type,width,height,depth,weight,amount,radius_min,radius_max,radius_step,radius_weight_function_key,axis,allow_mixed_loading
c1,Cylinder,cylinder,6,5,6,1,1,1,3,1,cylinder_weight,Y,false
# table:bins
bin_id,type_code,width,height,depth,capacity
b1,BIN,10,10,10,1000
# table:layers
layer_id,bin_id,depth
l1,b1,5
"#;
    let request = crate::application::csv::CsvDatasetLoader::load_str(input).unwrap().materialize().unwrap();
    let traces = vec![(Vec::new(), vec![LayerPlacementTrace {
        item_index: 0, item_id: "c1".into(), orientation: Orientation::Upright, amount: 1,
        position: MetricPoint3 { x: meters(0.0), y: meters(0.0), z: meters(0.0) },
    }])];
    let mut generation = LayerGenerationContext::new(); generation.add_generator(Box::new(PackageProbe));
    let rmp = RadiusProbe::default();
    let result = ColumnGenerationApplicationService::default().run_csv_materialized_one_generation_round_with_traced_initial_layers(
        request, traces, generation, &rmp, &MockColumnGenerationFinalExecutor,
    ).unwrap();
    assert_eq!(*rmp.0.lock().unwrap(), vec![true, true]);
    assert_eq!(result.result.info["continuous_radius_prototype_count"], "1");
}

#[cfg(feature = "serde")]
#[test]
fn csv_traced_initial_layers_preserve_explicit_coverage_and_require_selected_trace() {
    let mut request = traced_csv_request();
    let mut traces = traced_csv_initial_traces(&request);
    request.initial_layers[0].demand_coverage = vec![Bpp3dLayerDemandCoverage::new(
        Bpp3dDemandMode::Item, Bpp3dDemandKey::Item { id: "i1".into() }, 1.0,
    )];
    prepare_csv_traced_initial_layers(&mut request, &traces).unwrap();
    assert_eq!(request.initial_layers[0].demand_coverage[0].key, Bpp3dDemandKey::Item { id: "i1".into() });
    assert_eq!(request.initial_layers[1].demand_coverage[0].key, Bpp3dDemandKey::Item { id: "i1".into() });

    let mut request = traced_csv_request();
    traces[0].1.clear();
    request.initial_layers[0].demand_coverage = vec![Bpp3dLayerDemandCoverage::new(
        Bpp3dDemandMode::Item, Bpp3dDemandKey::Item { id: "i0".into() }, 1.0,
    )];
    let backend = ReportMetaModelBackend::new(std::sync::Arc::new(TracedFinalReportSolver), Default::default()).unwrap();
    let executor = TraceValidatedMetaModelFinalExecutor::new(Default::default(), backend);
    let errors = ColumnGenerationApplicationService::default().run_csv_materialized_one_generation_round_with_traced_initial_layers(
        request, traces, LayerGenerationContext::new(), &MockColumnGenerationRmpExecutor::default(), &executor,
    ).unwrap_err();
    assert!(errors.iter().any(|error| error.contains("FinalMilp") && error.contains("coverage fallback forbidden")), "{errors:?}");
}

#[cfg(feature = "serde")]
#[test]
fn csv_traced_initial_layers_use_block_counts_and_reject_conflicting_summaries() {
    for conflicting in [false, true] {
        let request = traced_csv_request();
        let mut traces = traced_csv_initial_traces(&request);
        traces[0].0.push(LayerBlockTrace {
            block_index: 0, item_index: 0, item_id: "i0".into(), orientation: Orientation::Upright,
            nx: 1, ny: 1, nz: 1, item_count: 1,
            size: MetricSize3 { width: meters(2.0), height: meters(3.0), depth: meters(4.0) },
            origin: MetricPoint3 { x: meters(0.0), y: meters(0.0), z: meters(0.0) },
        });
        if conflicting { traces[0].1[0].item_id = "i1".into(); }
        let backend = ReportMetaModelBackend::new(std::sync::Arc::new(TracedFinalReportSolver), Default::default()).unwrap();
        let executor = TraceValidatedMetaModelFinalExecutor::new(Default::default(), backend.clone());
        let result = ColumnGenerationApplicationService::default().run_csv_materialized_one_generation_round_with_traced_initial_layers(
            request, traces, LayerGenerationContext::new(), &MockColumnGenerationRmpExecutor::default(), &executor,
        );
        if conflicting {
            assert!(result.unwrap_err().iter().any(|error| error.contains("placement summaries disagree")));
        } else {
            assert_eq!(result.unwrap().final_execution.packed_bins[0].items.len(), 2);
        }
        assert_eq!(backend.reports().len(), 1);
    }
}

#[test]
fn initial_layer_traces_follow_first_accepted_candidate_after_coverage_fallback() {
    for first_has_trace in [false, true] {
        let state = traced_final_state();
        let mut layer = state.layers[0].clone();
        layer.demand_coverage.clear();
        let mut first = generated_trace_candidate(layer.clone(), 0, 1.0);
        if !first_has_trace { first.placement_traces.clear(); }
        layer.demand_coverage = default_item_coverage(&state.items);
        let duplicate = generated_trace_candidate(layer, 1, 2.0);
        let capture = CaptureGeneratedTraceFinal::default();
        ColumnGenerationApplicationService::default().run_materialized_with_traced_initial_layers(
            state.items, state.bins, vec![first, duplicate], &MockColumnGenerationRmpExecutor::default(), &capture,
        ).unwrap();
        let recorded = capture.0.lock().unwrap();
        let recorded = recorded.as_ref().unwrap();
        assert_eq!(recorded.layers.len(), 1);
        if first_has_trace {
            assert_eq!(recorded.layer_placement_traces[&0][0].item_id.as_str(), "i0");
            assert_eq!(recorded.layer_placement_traces[&0][0].position.x.value, 1.0);
        } else {
            assert!(!recorded.layer_placement_traces.contains_key(&0));
        }
    }
}

#[test]
fn generated_traces_keep_bin_coverage_and_source_pairs_separate_from_initial_layers() {
    let mut state = traced_final_state();
    state.items.push(item("i2"));
    state.bins[1].type_code = "other".into();
    for layer in &mut state.layers { layer.from = "same-source".into(); }
    let mut seed = state.layers[0].clone();
    seed.demand_coverage = vec![Bpp3dLayerDemandCoverage::new(
        Bpp3dDemandMode::Item, Bpp3dDemandKey::Item { id: "i2".into() }, 1.0,
    )];
    let mut first = generated_trace_candidate(state.layers[0].clone(), 0, 0.0);
    first.block_traces.push(LayerBlockTrace {
        block_index: 0, item_index: 0, item_id: "i0".into(), orientation: Orientation::Upright,
        nx: 1, ny: 1, nz: 1, item_count: 1,
        size: MetricSize3 { width: meters(2.0), height: meters(3.0), depth: meters(4.0) },
        origin: first.placement_traces[0].position.clone(),
    });
    let second = generated_trace_candidate(state.layers[1].clone(), 1, 1.0);
    let mut other_bin_layer = state.layers[0].clone();
    other_bin_layer.bin = Some(state.bins[1].clone());
    let third = generated_trace_candidate(other_bin_layer, 0, 2.0);
    let mut generation = LayerGenerationContext::new();
    generation.add_generator(Box::new(FixedTraceGenerator(vec![first, second, third])));
    let capture = CaptureGeneratedTraceFinal::default();
    ColumnGenerationApplicationService::default().run_materialized_one_generation_round(
        state.items, state.bins, vec![seed], generation, &MockColumnGenerationRmpExecutor::default(), &capture,
    ).unwrap();
    let recorded = capture.0.lock().unwrap();
    let recorded = recorded.as_ref().unwrap();
    assert_eq!(recorded.layers.len(), 4);
    assert!(!recorded.layer_placement_traces.contains_key(&0));
    assert!(!recorded.layer_block_traces.contains_key(&0));
    assert_eq!(recorded.layer_placement_traces[&1][0].item_id.as_str(), "i0");
    assert_eq!(recorded.layer_block_traces[&1][0].item_id.as_str(), "i0");
    assert_eq!(recorded.layer_placement_traces[&2][0].item_id.as_str(), "i1");
    assert!(!recorded.layer_block_traces.contains_key(&2));
    assert_eq!(recorded.layer_placement_traces[&3][0].position.x.value, 2.0);
    assert_eq!(recorded.layers[3].bin.as_ref().unwrap().type_code.as_str(), "other");
}

#[test]
fn generated_traces_follow_first_accepted_candidate_after_coverage_fallback() {
    for first_has_trace in [false, true] {
        let state = traced_final_state();
        let mut seed = state.layers[0].clone();
        seed.from = "seed".into();
        seed.demand_coverage.clear();
        let rejected_seed = generated_trace_candidate(seed.clone(), 0, 0.0);
        let mut added = seed.clone();
        added.from = "added".into();
        let mut accepted = generated_trace_candidate(added.clone(), 0, 1.0);
        if !first_has_trace { accepted.placement_traces.clear(); }
        added.demand_coverage = default_item_coverage(&state.items);
        let mut duplicate = generated_trace_candidate(added, 1, 2.0);
        duplicate.block_traces.push(LayerBlockTrace {
            block_index: 0, item_index: 1, item_id: "i1".into(), orientation: Orientation::Upright,
            nx: 1, ny: 1, nz: 1, item_count: 1,
            size: MetricSize3 { width: meters(2.0), height: meters(3.0), depth: meters(4.0) },
            origin: duplicate.placement_traces[0].position.clone(),
        });
        let mut generation = LayerGenerationContext::new();
        generation.add_generator(Box::new(FixedTraceGenerator(vec![rejected_seed, accepted, duplicate])));
        let capture = CaptureGeneratedTraceFinal::default();
        ColumnGenerationApplicationService::default().run_materialized_one_generation_round(
            state.items, state.bins, vec![seed], generation, &MockColumnGenerationRmpExecutor::default(), &capture,
        ).unwrap();
        let recorded = capture.0.lock().unwrap();
        let recorded = recorded.as_ref().unwrap();
        assert_eq!(recorded.layers.len(), 2);
        assert!(!recorded.layer_placement_traces.contains_key(&0));
        if first_has_trace {
            assert_eq!(recorded.layer_placement_traces[&1][0].item_id.as_str(), "i0");
            assert_eq!(recorded.layer_placement_traces[&1][0].position.x.value, 1.0);
        } else {
            assert!(!recorded.layer_placement_traces.contains_key(&1));
        }
        assert!(!recorded.layer_block_traces.contains_key(&1));
    }
}

#[test]
fn traced_final_rejects_forged_or_ambiguous_item_coverage() {
    for duplicate in [false, true] {
        let backend = ReportMetaModelBackend::new(std::sync::Arc::new(TracedFinalReportSolver), Default::default()).unwrap();
        let executor = TraceValidatedMetaModelFinalExecutor::new(Default::default(), backend.clone());
        let mut state = traced_final_state();
        if duplicate {
            let coverage = state.layers[0].demand_coverage[0].clone();
            state.layers[0].demand_coverage.push(coverage);
        } else {
            let first = state.layers[0].demand_coverage.clone();
            state.layers[0].demand_coverage = state.layers[1].demand_coverage.clone();
            state.layers[1].demand_coverage = first;
        }
        let result = executor.execute_with_selection(&state, None);
        assert_eq!(result.execution.info["status"], "physical_validation_failed", "{:?}", result.execution.info);
        assert!(result.execution.info["error"].contains("coverage"));
        assert!(result.execution.packed_bins.is_empty());
        assert!(result.selection.is_some());
        assert_eq!(backend.reports().len(), 1);
    }
}

#[test]
fn traced_final_rechecks_actual_weight_when_model_coverage_understates_it() {
    let backend = ReportMetaModelBackend::new(std::sync::Arc::new(TracedFinalReportSolver), Default::default()).unwrap();
    let executor = TraceValidatedMetaModelFinalExecutor::new(Default::default(), backend.clone());
    let mut state = traced_final_state();
    for bin in &mut state.bins { bin.capacity = meters(1.0); }
    for (index, layer) in state.layers.iter_mut().enumerate() {
        layer.demand_coverage.push(Bpp3dLayerDemandCoverage::new(
            Bpp3dDemandMode::ItemWeight, Bpp3dDemandKey::Item { id: state.items[index].id.clone() }, -0.5,
        ));
    }
    let result = executor.execute_with_selection(&state, None);
    assert_eq!(result.execution.info["status"], "physical_validation_failed", "{:?}", result.execution.info);
    assert!(result.execution.info["error"].contains("weight capacity"));
    assert!(result.execution.packed_bins.is_empty());
    assert_eq!(backend.reports().len(), 1);
}

#[test]
fn strict_final_validation_rejects_duplicate_ids_and_unsatisfied_item_demand() {
    let mut state = traced_final_state();
    let selection = PreciseAssignmentSelection {
        activated_bins: vec![0],
        assignments: vec![
            crate::domain::layer_assignment::LayerAssignment { bin_index: 0, layer_index: 0 },
            crate::domain::layer_assignment::LayerAssignment { bin_index: 0, layer_index: 1 },
        ],
    };
    state.layers[1].demand_coverage = state.layers[0].demand_coverage.clone();
    let trace = state.layer_placement_traces[&0].clone();
    state.layer_placement_traces.insert(1, trace);
    let packed = LayerTraceReplayAdapter::new().replay_assignments(LayerAssignmentTraceReplayRequest {
        selection: &selection, layers: &state.layers, bins: &state.bins, items: &state.items,
        block_traces: &state.layer_block_traces, placement_traces: &state.layer_placement_traces,
        max_placements: 100, extra_geometry: None,
    }).unwrap();
    assert!(validate_strict_final_packing(&state, &selection, &state.items, &packed, None).unwrap_err().contains("exactly once"));
    assert!(validate_strict_final_packing(&state, &selection, &state.items, &packed, Some(&[2, 1])).unwrap_err().contains("exactly"));
    state.items[1].id = state.items[0].id.clone();
    assert!(validate_strict_final_packing(&state, &selection, &state.items, &packed, None).unwrap_err().contains("duplicate item ID"));
}

#[test]
fn traced_final_precancellation_cannot_use_configured_noop_solution() {
    let options = ospf_rust_framework::solver::LinearExecutionOptions::default(); options.cancellation.cancel("before-traced-final");
    let backend = ReportMetaModelBackend::new(std::sync::Arc::new(TracedFinalReportSolver), options).unwrap();
    let executor = TraceValidatedMetaModelFinalExecutor::new(MetaModelFinalExecutorConfig { primal_solution: vec![1.0; 6], objective: Some(1.0), ..Default::default() }, backend.clone());
    let result = executor.execute_with_selection(&traced_final_state(), None);
    assert!(!result.execution.final_solved); assert!(result.selection.is_none()); assert!(result.execution.packed_bins.is_empty());
    assert_eq!(backend.reports()[0].report.termination_reason, ospf_rust_core::solver::TerminationReason::Cancelled);
    let grouped = executor.execute_with_selection_and_item_amounts(&traced_final_state(), &[1, 1], None);
    assert!(!grouped.execution.final_solved); assert!(grouped.selection.is_none()); assert!(grouped.execution.packed_bins.is_empty());
    assert_eq!(backend.reports()[1].report.termination_reason, ospf_rust_core::solver::TerminationReason::Cancelled);
}

#[cfg(all(not(feature = "async"), any(feature = "scip", feature = "gurobi10")))]
fn run_actual_traced_final(solver: std::sync::Arc<dyn ospf_rust_core::solver::LinearSolver>) {
    let backend = ReportMetaModelBackend::new(solver, Default::default()).unwrap();
    let executor = TraceValidatedMetaModelFinalExecutor::new(Default::default(), backend.clone());
    let mut state = traced_final_state(); let result = executor.execute_with_selection(&state, Some(&OneBinPackingExtension));
    assert!(result.execution.final_solved, "{:?}", result.execution.info);
    let selection = result.selection.unwrap();
    assert_eq!(selection.assignments.len(), result.execution.layers.len()); assert!(!selection.assignments.is_empty());
    assert_eq!(selection.activated_bins.len(), result.execution.packed_bins.len());
    assert_eq!(result.execution.packed_bins.len(), 1); assert_eq!(result.execution.packed_bins[0].items.len(), 2);
    assert_eq!(result.execution.packed_bins[0].items[1].position.z.value, 4.0);
    assert_eq!(result.execution.packed_bins.iter().map(|bin| bin.items.len()).sum::<usize>(), selection.assignments.len());
    assert_eq!(backend.reports().len(), 1); assert!(backend.reports()[0].model.has_integer_variables());
    state.layer_placement_traces.clear(); let invalid = executor.execute_with_selection(&state, None);
    assert!(!invalid.execution.final_solved); assert!(invalid.selection.is_some()); assert_eq!(backend.reports().len(), 2);
    assert!(backend.reports()[1].report.has_incumbent());
}

#[cfg(all(not(feature = "async"), any(feature = "scip", feature = "gurobi10")))]
fn run_actual_trace_backed_application(solver: std::sync::Arc<dyn ospf_rust_core::solver::LinearSolver>) {
    use ospf_rust_core::solver::require_optimal_lp_certificate_for_linear_model;
    let backend = ReportMetaModelBackend::new(solver, Default::default()).unwrap();
    let rmp = SolverBackedMetaModelRmpExecutor::new(Default::default(), backend.clone());
    let final_executor = TraceValidatedMetaModelFinalExecutor::new(Default::default(), backend.clone());
    let state = traced_final_state();
    let mut seed_layer = state.layers[0].clone();
    seed_layer.demand_coverage.extend(state.layers[1].demand_coverage.clone());
    let mut seed = generated_trace_candidate(seed_layer, 0, 0.0);
    seed.placement_traces.push(generated_trace_candidate(state.layers[1].clone(), 1, 2.0).placement_traces.remove(0));
    let generated = generated_trace_candidate(state.layers[0].clone(), 0, 0.0);
    let mut generation = LayerGenerationContext::new();
    generation.add_generator(Box::new(FixedTraceGenerator(vec![generated])));
    let result = ColumnGenerationApplicationService::default().run_materialized_one_generation_round_with_traced_initial_layers(
        state.items, state.bins, vec![seed], generation, &rmp, &final_executor,
    ).unwrap();
    assert!(result.final_execution.final_solved, "{:?}", result.final_execution.info);
    assert_eq!(result.result.info["generated_layer_count"], "1");
    assert_eq!(result.final_execution.packed_bins.len(), 1);
    assert_eq!(result.final_execution.packed_bins[0].items.len(), 2);
    let reports = backend.reports();
    assert_eq!(reports.len(), 3);
    assert!(reports[1].model.num_variables() > reports[0].model.num_variables());
    for record in &reports[..2] {
        require_optimal_lp_certificate_for_linear_model(&record.report, &record.model).unwrap();
    }
    assert!(reports[2].model.has_integer_variables());
    assert!(reports[2].report.has_incumbent());
}

#[cfg(all(feature = "serde", not(feature = "async"), any(feature = "scip", feature = "gurobi10")))]
fn run_actual_traced_csv_application(solver: std::sync::Arc<dyn ospf_rust_core::solver::LinearSolver>) {
    use ospf_rust_core::solver::require_optimal_lp_certificate_for_linear_model;
    let request = traced_csv_request();
    let traces = traced_csv_initial_traces(&request);
    let mut layer = request.initial_layers[0].clone();
    layer.from = "generated-both".into();
    layer.demand_coverage = request.items.iter().map(|item| Bpp3dLayerDemandCoverage::new(
        Bpp3dDemandMode::Item, Bpp3dDemandKey::Item { id: item.id.clone() }, 1.0,
    )).collect();
    let mut candidate = generated_trace_candidate(layer.clone(), 0, 0.0);
    candidate.placement_traces.push(generated_trace_candidate(layer, 1, 2.0).placement_traces.remove(0));
    let mut generation = LayerGenerationContext::new();
    generation.add_generator(Box::new(FixedTraceGenerator(vec![candidate])));
    let backend = ReportMetaModelBackend::new(solver, Default::default()).unwrap();
    let rmp = SolverBackedMetaModelRmpExecutor::new(Default::default(), backend.clone());
    let final_executor = TraceValidatedMetaModelFinalExecutor::new(Default::default(), backend.clone());
    let result = ColumnGenerationApplicationService::default().run_csv_materialized_one_generation_round_with_traced_initial_layers(
        request, traces, generation, &rmp, &final_executor,
    ).unwrap();
    assert!(result.final_execution.final_solved, "{:?}", result.final_execution.info);
    assert_eq!(result.result.info["generated_layer_count"], "1");
    assert_eq!(result.final_execution.packed_bins.len(), 1);
    assert_eq!(result.final_execution.packed_bins[0].items.len(), 2);
    let reports = backend.reports();
    assert_eq!(reports.len(), 3);
    assert!(reports[1].model.num_variables() > reports[0].model.num_variables());
    for record in &reports[..2] {
        require_optimal_lp_certificate_for_linear_model(&record.report, &record.model).unwrap();
    }
    assert!(reports[2].model.has_integer_variables());
    assert!(reports[2].report.has_incumbent());
}

#[cfg(all(feature = "serde", not(feature = "async"), any(feature = "scip", feature = "gurobi10")))]
fn run_actual_grouped_traced_csv_application(solver: std::sync::Arc<dyn ospf_rust_core::solver::LinearSolver>) {
    use ospf_rust_core::solver::require_optimal_lp_certificate_for_linear_model;
    let (request, traces) = grouped_traced_csv_request();
    let backend = ReportMetaModelBackend::new(solver, Default::default()).unwrap();
    let rmp = SolverBackedMetaModelRmpExecutor::new(Default::default(), backend.clone());
    let final_executor = TraceValidatedMetaModelFinalExecutor::new(Default::default(), backend.clone());
    let result = ColumnGenerationApplicationService::default().run_csv_materialized_one_generation_round_with_traced_initial_layers(
        request, traces, LayerGenerationContext::new(), &rmp, &final_executor,
    ).unwrap();
    assert!(result.final_execution.final_solved, "{:?}", result.final_execution.info);
    let packed = &result.final_execution.packed_bins[0].items;
    assert_eq!(packed.len(), 2);
    assert!(packed.iter().all(|item| item.item.id.as_str() == "i0"));
    let reports = backend.reports();
    assert_eq!(reports.len(), 3);
    for (index, record) in reports.iter().enumerate() {
        if index < 2 { require_optimal_lp_certificate_for_linear_model(&record.report, &record.model).unwrap(); }
        else { assert!(record.model.has_integer_variables() && record.report.has_incumbent()); }
        let basic = &record.model.basic;
        let variable = basic.variables.iter().position(|token| token.name() == if index < 2 { "x_0" } else { "x_0_0" }).unwrap();
        for sign in [1.0, -1.0] {
            assert!(basic.A.rows.iter().zip(&basic.b).any(|(row, rhs)|
                row.entries == [(variable, sign * 2.0)] && *rhs == sign * 2.0));
        }
    }
}

#[cfg(all(feature = "serde", not(feature = "async"), any(feature = "scip", feature = "gurobi10")))]
fn run_actual_grouped_traced_csv_multiple_bins(solver: std::sync::Arc<dyn ospf_rust_core::solver::LinearSolver>) {
    use ospf_rust_core::solver::require_optimal_lp_certificate_for_linear_model;
    let (mut request, mut traces) = grouped_traced_csv_request();
    traces[0].1.pop();
    request.bins.push(request.bins[0].clone());
    let mut direct_request = request.clone();
    let amounts = prepare_csv_traced_initial_layers(&mut direct_request, &traces).unwrap();
    let direct_state = ColumnGenerationApplicationState {
        items: direct_request.items, bins: direct_request.bins, initial_layers: direct_request.initial_layers.clone(),
        layers: direct_request.initial_layers, iteration: 0, layer_block_traces: HashMap::new(),
        layer_placement_traces: HashMap::from([(0, traces[0].1.clone())]), continuous_radius_component: None,
        additional_shadow_prices: HashMap::new(), info: HashMap::new(),
    };
    let backend = ReportMetaModelBackend::new(solver, Default::default()).unwrap();
    let rmp = SolverBackedMetaModelRmpExecutor::new(Default::default(), backend.clone());
    let final_executor = TraceValidatedMetaModelFinalExecutor::new(Default::default(), backend.clone());
    let result = ColumnGenerationApplicationService::default().run_csv_materialized_one_generation_round_with_traced_initial_layers(
        request, traces, LayerGenerationContext::new(), &rmp, &final_executor,
    ).unwrap();
    assert!(result.final_execution.final_solved, "{:?}", result.final_execution.info);
    assert_eq!(result.final_execution.packed_bins.len(), 2);
    assert!(result.final_execution.packed_bins.iter().all(|bin| bin.items.len() == 1 && bin.items[0].item.id.as_str() == "i0"));
    let reports = backend.reports();
    assert_eq!(reports.len(), 3);
    for record in &reports[..2] { require_optimal_lp_certificate_for_linear_model(&record.report, &record.model).unwrap(); }
    let basic = &reports[2].model.basic;
    let indices = ["x_0_0", "x_1_0"].map(|name| basic.variables.iter().position(|token| token.name() == name).unwrap());
    for sign in [1.0, -1.0] {
        assert!(basic.A.rows.iter().zip(&basic.b).any(|(row, rhs)|
            row.entries == [(indices[0], sign), (indices[1], sign)] && *rhs == sign * 2.0));
    }
    let direct = final_executor.execute_with_selection_and_item_amounts(&direct_state, &amounts, None);
    assert!(direct.execution.final_solved, "{:?}", direct.execution.info);
    let selection = direct.selection.unwrap();
    assert_eq!(selection.activated_bins, vec![0, 1]);
    assert_eq!(selection.assignments.iter().map(|entry| (entry.bin_index, entry.layer_index)).collect::<Vec<_>>(), vec![(0, 0), (1, 0)]);
    assert_eq!(direct.execution.packed_bins.len(), 2);
    assert!(direct.execution.packed_bins.iter().all(|bin| bin.items.len() == 1 && bin.items[0].item.id.as_str() == "i0"));
    assert_eq!(backend.reports().len(), 4);
}

#[cfg(all(feature = "serde", feature = "scip", not(feature = "async")))]
#[test]
fn actual_scip_grouped_traced_csv_preserves_demand_in_both_rmp_reports_and_final() {
    use ospf_rust_core_plugin_scip::{SCIPConfig, SCIPSolver};
    run_actual_grouped_traced_csv_application(std::sync::Arc::new(SCIPSolver::with_config(
        SCIPConfig::default().with_output(false).with_lp_subproblem_defaults(),
    )));
}

#[cfg(all(feature = "serde", feature = "scip", not(feature = "async")))]
#[test]
fn actual_scip_grouped_traced_csv_reuses_a_layer_across_distinct_bins() {
    use ospf_rust_core_plugin_scip::{SCIPConfig, SCIPSolver};
    run_actual_grouped_traced_csv_multiple_bins(std::sync::Arc::new(SCIPSolver::with_config(
        SCIPConfig::default().with_output(false).with_lp_subproblem_defaults(),
    )));
}

#[cfg(all(feature = "serde", feature = "gurobi10", not(feature = "async")))]
#[test]
fn actual_gurobi_grouped_traced_csv_preserves_demand_in_both_rmp_reports_and_final() {
    use ospf_rust_core::solver::solvers::gurobi::{GurobiConfig, GurobiSolver};
    run_actual_grouped_traced_csv_application(std::sync::Arc::new(GurobiSolver::with_config(
        GurobiConfig::default().with_output(false),
    )));
}

#[cfg(all(feature = "serde", feature = "gurobi10", not(feature = "async")))]
#[test]
fn actual_gurobi_grouped_traced_csv_reuses_a_layer_across_distinct_bins() {
    use ospf_rust_core::solver::solvers::gurobi::{GurobiConfig, GurobiSolver};
    run_actual_grouped_traced_csv_multiple_bins(std::sync::Arc::new(GurobiSolver::with_config(
        GurobiConfig::default().with_output(false),
    )));
}

#[cfg(all(feature = "serde", feature = "scip", not(feature = "async")))]
#[test]
fn actual_scip_traced_csv_application_uses_original_reports_and_physical_evidence() {
    use ospf_rust_core_plugin_scip::{SCIPConfig, SCIPSolver};
    run_actual_traced_csv_application(std::sync::Arc::new(SCIPSolver::with_config(
        SCIPConfig::default().with_output(false).with_lp_subproblem_defaults(),
    )));
}

#[cfg(all(feature = "serde", feature = "gurobi10", not(feature = "async")))]
#[test]
fn actual_gurobi_traced_csv_application_uses_original_reports_and_physical_evidence() {
    use ospf_rust_core::solver::solvers::gurobi::{GurobiConfig, GurobiSolver};
    run_actual_traced_csv_application(std::sync::Arc::new(GurobiSolver::with_config(
        GurobiConfig::default().with_output(false),
    )));
}

#[cfg(all(feature = "scip", not(feature = "async")))]
#[test]
fn actual_scip_traced_final_retains_actual_report_and_requires_physical_evidence() {
    use ospf_rust_core_plugin_scip::{SCIPConfig, SCIPSolver};
    let solver = std::sync::Arc::new(SCIPSolver::with_config(
        SCIPConfig::default().with_output(false).with_lp_subproblem_defaults(),
    ));
    run_actual_traced_final(solver.clone());
    run_actual_trace_backed_application(solver);
}

#[cfg(all(feature = "gurobi10", not(feature = "async")))]
#[test]
fn actual_gurobi_traced_final_retains_actual_report_and_requires_physical_evidence() {
    use ospf_rust_core::solver::solvers::gurobi::{GurobiConfig, GurobiSolver};
    let solver = std::sync::Arc::new(GurobiSolver::with_config(GurobiConfig::default().with_output(false)));
    run_actual_traced_final(solver.clone());
    run_actual_trace_backed_application(solver);
}
