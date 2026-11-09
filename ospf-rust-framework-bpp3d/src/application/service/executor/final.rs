
/// MetaModel 最终 MILP 执行器 / MetaModel final MILP executor
#[derive(Debug, Clone, Default)]
pub struct MetaModelFinalExecutor {
    /// 配置 / Config
    pub config: MetaModelFinalExecutorConfig,
}

impl MetaModelFinalExecutor {
    /// 使用配置创建 / Create with config
    pub fn new(config: MetaModelFinalExecutorConfig) -> Self {
        Self { config }
    }

    fn build_context(
        &self,
        state: &ColumnGenerationApplicationState,
        amounts: Option<&[u64]>,
    ) -> Result<(LayerAssignmentContext<f64, Meter>, PreciseAssignment<f64, Meter>, Vec<Bpp3dDemandEntry>), String> {
        let demand_entries = item_demand_entries(&state.items, amounts)?;
        let bins = if state.bins.is_empty() {
            bins_from_layers(&state.layers)
        } else {
            state.bins.clone()
        };
        let assignment = PreciseAssignment {
            bins,
            layers: state.layers.clone(),
            x: None,
            v: None,
            load_weight_symbols: Vec::new(),
            load_volume_symbols: Vec::new(),
            load_depth_symbols: Vec::new(),
        };
        let aggregation = LayerAssignmentAggregation::final_milp(
            assignment.clone(),
            Load::new(demand_entries.clone()),
            Capacity::new(),
        );
        let context = LayerAssignmentContext::new(aggregation);
        // Limits are added after registration in execute_with_backend
        Ok((context, assignment, demand_entries))
    }

    fn registered_model(
        &self,
        state: &ColumnGenerationApplicationState,
        amounts: Option<&[u64]>,
        extension: Option<&dyn ColumnGenerationFinalModelExtension>,
    ) -> Result<(MetaModel<f64>, LayerAssignmentContext<f64, Meter>, Vec<Bpp3dDemandEntry>, ContinuousRadiusModelRegistration), String> {
        let (mut context, assignment, demand_entries) = self.build_context(state, amounts)?;
        let mut model = MetaModel::<f64>::new(&self.config.model_name);
        context.aggregation_mut().register(&mut model)?;
        let registered = context.aggregation().precise_assignment.clone().unwrap_or(assignment);
        context.add_limit(Box::new(DemandConstraint::precise(demand_entries.clone(), registered.clone())));
        context.add_limit(Box::new(PreciseAssignmentActivationConstraint::new(registered.clone())));
        context.add_limit(Box::new(BinCapacityConstraint::<f64, Meter>::from_bins(
            &registered.bins, final_assignment_indices_by_bin(&registered), final_layer_weights(&registered.layers, &state.items),
            final_layer_volumes(&registered.layers), &Default::default(),
        )));
        context.add_limit(Box::new(BinDepthConstraint::<f64, Meter>::from_bins(
            &registered.bins, final_assignment_indices_by_bin(&registered), layer_depths(&registered.layers), &Default::default(),
        )));
        let v_indices = final_bin_marker_indices(&registered);
        context.add_objective(Box::new(BinVolumeMinimization::from_bins(
            &registered.bins, v_indices.clone(), self.config.final_bin_volume_weight,
        )));
        context.add_objective(Box::new(BinAmountMinimization::new(v_indices.clone(), self.config.final_bin_amount_coefficient)));
        context.add_objective(Box::new(BinTypeCostMinimization::from_bins(
            &registered.bins, v_indices, &self.config.final_bin_type_costs, self.config.final_bin_type_cost_weight,
        )));
        context.register_limits(&mut model)
            .and_then(|_| extension.map(|extension| extension.register(state, &mut model)).unwrap_or(Ok(())))
            .and_then(|_| context.invoke(&model))?;
        let registration = match state.continuous_radius_component.as_ref() {
            Some(component) => component.register_solver_model(&mut model)?,
            None => Default::default(),
        };
        Ok((model, context, demand_entries, registration))
    }

    /// 仅装配带分组需求的原 final 模型，不调用求解器或注册扩展。
    /// Assemble the original grouped-demand final model without calling a solver or registering extensions.
    pub fn model_with_item_amounts(
        &self,
        state: &ColumnGenerationApplicationState,
        amounts: &[u64],
    ) -> Result<MetaModel<f64>, String> {
        self.model_and_assignment_with_item_amounts(state, amounts).map(|(model, _)| model)
    }

    /// 装配分组需求 final 模型并保留已注册的箱／层变量，不调用求解器或注册扩展。
    /// Assemble the grouped-demand final model and retain registered bin/layer variables, without solving or registering extensions.
    pub fn model_and_assignment_with_item_amounts(
        &self,
        state: &ColumnGenerationApplicationState,
        amounts: &[u64],
    ) -> Result<(MetaModel<f64>, PreciseAssignment<f64, Meter>), String> {
        let (model, context, _, _) = self.registered_model(state, Some(amounts), None)?;
        let assignment = context.aggregation.precise_assignment.ok_or("missing precise assignment after registration")?;
        Ok((model, assignment))
    }
}

impl ColumnGenerationFinalExecutor for MetaModelFinalExecutor {
    fn execute(&self, state: &ColumnGenerationApplicationState) -> ColumnGenerationFinalExecution {
        let backend = NoopMetaModelSolverBackend;
        self.execute_with_backend(state, &backend)
    }
}

/// 物理复验配置，不改变旧 final 默认行为。 / Physical validation settings, without changing legacy final defaults.
#[derive(Debug, Clone)]
pub struct FinalPackingValidation {
    /// 全部展开物品上限，默认 10000。 / Total expanded-item limit, default 10000.
    pub max_placements: usize,
    /// 不可放宽基础几何的附加业务检查。 / Additional business checks that cannot weaken base geometry.
    pub extra_geometry: Option<std::sync::Arc<dyn PackingGeometryContract<f64, Meter>>>,
}

impl Default for FinalPackingValidation {
    fn default() -> Self { Self { max_placements: 10_000, extra_geometry: None } }
}

/// 保留原箱／层分配的 final 结果；物理失败仍可观察原分配，不伪造装箱成功。
/// Final result retaining original bin/layer assignments; physical failures keep the selection observable without fabricating packing success.
#[derive(Debug, Clone)]
pub struct LayerAssignmentFinalExecution {
    /// 既有 application 结果；final_solved 仅在全部物理检查通过后为 true。 / Existing application result; final_solved becomes true only after all physical gates pass.
    pub execution: ColumnGenerationFinalExecution,
    /// 已完整提取的原 x/v；提取之前失败则为 None。 / Completely extracted original x/v, absent on failures before extraction.
    pub selection: Option<PreciseAssignmentSelection>,
}

impl From<ColumnGenerationFinalExecution> for LayerAssignmentFinalExecution {
    fn from(execution: ColumnGenerationFinalExecution) -> Self { Self { execution, selection: None } }
}

/// 显式报告与物理 trace 驱动的 final，可直接注入原 application 编排。
/// Explicit report/physical-trace-driven final, injectable into existing application orchestration.
#[derive(Debug)]
pub struct TraceValidatedMetaModelFinalExecutor {
    /// 与旧执行器共享的建模配置；不读取其中的 no-op 解。 / Shared modelling configuration; its no-op solution is never used.
    pub config: MetaModelFinalExecutorConfig,
    /// 本次领域调用共用的报告后端。 / Shared report backend for this domain invocation.
    pub backend: ReportMetaModelBackend,
    /// 物理展开与扩展门禁。 / Physical expansion and extension gates.
    pub validation: FinalPackingValidation,
}

impl TraceValidatedMetaModelFinalExecutor {
    /// 使用原建模配置与报告后端创建，物理门禁默认开启。 / Create with original modelling settings and report backend, enabling physical gates by default.
    pub fn new(config: MetaModelFinalExecutorConfig, backend: ReportMetaModelBackend) -> Self {
        Self { config, backend, validation: Default::default() }
    }

    /// 保留原分配的入口；扩展继续走原 MetaModel 注册路径。 / Entry retaining original assignments; extensions use the original MetaModel registration path.
    pub fn execute_with_selection(
        &self,
        state: &ColumnGenerationApplicationState,
        extension: Option<&dyn ColumnGenerationFinalModelExtension>,
    ) -> LayerAssignmentFinalExecution {
        MetaModelFinalExecutor::new(self.config.clone()).execute_final(state, &self.backend, extension, Some(&self.validation), None)
    }

    /// 按物品目录顺序使用分组需求，并保留真实箱／层分配。 / Use grouped demand in item-catalog order and retain actual bin/layer assignments.
    pub fn execute_with_selection_and_item_amounts(
        &self,
        state: &ColumnGenerationApplicationState,
        amounts: &[u64],
        extension: Option<&dyn ColumnGenerationFinalModelExtension>,
    ) -> LayerAssignmentFinalExecution {
        MetaModelFinalExecutor::new(self.config.clone()).execute_final(state, &self.backend, extension, Some(&self.validation), Some(amounts))
    }
}

impl ColumnGenerationFinalExecutor for TraceValidatedMetaModelFinalExecutor {
    fn execute(&self, state: &ColumnGenerationApplicationState) -> ColumnGenerationFinalExecution {
        self.execute_with_selection(state, None).execution
    }

    fn execute_result_with_item_amounts(
        &self,
        state: &ColumnGenerationApplicationState,
        amounts: &[u64],
    ) -> Result<ColumnGenerationFinalExecution, ColumnGenerationExecutionError> {
        final_execution_result(self.execute_with_selection_and_item_amounts(state, amounts, None).execution)
    }
}

/// 求解器驱动的 MetaModel final MILP executor / Solver-backed MetaModel final MILP executor
#[derive(Debug)]
pub struct SolverBackedMetaModelFinalExecutor<B>
where
    B: MetaModelSolverBackend,
{
    /// 配置 / Config
    pub config: MetaModelFinalExecutorConfig,
    /// 求解 backend / Solver backend
    pub backend: B,
}

impl<B> SolverBackedMetaModelFinalExecutor<B>
where
    B: MetaModelSolverBackend,
{
    /// 使用配置和 backend 创建 / Create with config and backend
    pub fn new(config: MetaModelFinalExecutorConfig, backend: B) -> Self {
        Self { config, backend }
    }
}

impl<B> ColumnGenerationFinalExecutor for SolverBackedMetaModelFinalExecutor<B>
where
    B: MetaModelSolverBackend,
{
    fn execute(&self, state: &ColumnGenerationApplicationState) -> ColumnGenerationFinalExecution {
        let executor = MetaModelFinalExecutor::new(self.config.clone());
        executor.execute_with_backend(state, &self.backend)
    }
}

impl MetaModelFinalExecutor {
    fn execute_with_backend(
        &self,
        state: &ColumnGenerationApplicationState,
        backend: &dyn MetaModelSolverBackend,
    ) -> ColumnGenerationFinalExecution {
        self.execute_with_backend_and_extension(state, backend, None)
    }

    /// 使用 backend 和可选模型扩展执行 final / Execute final with backend and optional extension
    pub fn execute_with_backend_and_extension(
        &self,
        state: &ColumnGenerationApplicationState,
        backend: &dyn MetaModelSolverBackend,
        extension: Option<&dyn ColumnGenerationFinalModelExtension>,
    ) -> ColumnGenerationFinalExecution {
        self.execute_final(state, backend, extension, None, None).execution
    }

    fn execute_final(
        &self,
        state: &ColumnGenerationApplicationState,
        backend: &dyn MetaModelSolverBackend,
        extension: Option<&dyn ColumnGenerationFinalModelExtension>,
        validation: Option<&FinalPackingValidation>,
        amounts: Option<&[u64]>,
    ) -> LayerAssignmentFinalExecution {
        let (mut model, context, demand_entries, continuous_radius_registration) = match self.registered_model(state, amounts, extension) {
            Ok(result) => result,
            Err(error) => return ColumnGenerationFinalExecution {
                info: HashMap::from([
                    ("executor".into(), "meta_model_final".into()),
                    ("status".into(), "registration_failed".into()),
                    ("error".into(), error),
                ]),
                ..Default::default()
            }.into(),
        };
        let diagnostics = MetaModelExecutionDiagnostics::from_model(
            self.config.model_name.clone(),
            &model,
            demand_entries.len(),
            state.layers.len(),
            context
                .aggregation
                .precise_assignment
                .as_ref()
                .map(|assignment| assignment.bins.len())
                .unwrap_or(0),
        );
        let solve = match backend.solve_final(&model, &diagnostics) {
            Ok(solve) if validation.is_some() => solve,
            Ok(solve) => merge_noop_solve_result(
                solve,
                self.config.objective,
                self.config.primal_solution.clone(),
                Vec::new(),
            ),
            Err(error) => {
                return ColumnGenerationFinalExecution {
                    layers: Vec::new(),
                    packed_bins: Vec::new(),
                    objective: None,
                    final_solved: false,
                    diagnostics: Some(diagnostics),
                    info: HashMap::from([
                        ("executor".to_string(), "meta_model_final".to_string()),
                        ("backend".to_string(), backend.name().to_string()),
                        ("status".to_string(), "solve_failed".to_string()),
                        ("error".to_string(), error),
                    ]),
                }.into();
            }
        };
        let mut lifecycle = DynamicModelLifecycle::new();
        lifecycle.set_solution_to_model(&mut model, solve.primal_solution.clone());
        let (continuous_radius_solutions, mut continuous_radius_diagnostics) =
            match state.continuous_radius_component.as_ref() {
                Some(component) => component.selected_solutions_from_primal(
                    &continuous_radius_registration,
                    &solve.primal_solution,
                ),
                None => (Vec::new(), Vec::new()),
            };
        continuous_radius_diagnostics.extend(continuous_radius_registration.diagnostics.clone());
        let Some(assignment) = context.aggregation.precise_assignment.as_ref() else {
            return ColumnGenerationFinalExecution {
                layers: Vec::new(),
                packed_bins: Vec::new(),
                objective: None,
                final_solved: false,
                diagnostics: None,
                info: HashMap::from([
                    ("executor".to_string(), "meta_model_final".to_string()),
                    ("status".to_string(), "registration_failed".to_string()),
                    ("error".to_string(), "missing precise assignment after registration".to_string()),
                ]),
            }.into();
        };
        if let Some(validation) = validation {
            let selected = assignment.extract_selection(&solve.primal_solution, 1e-7);
            let selection = selected.as_ref().ok().cloned();
            let packed = selected.and_then(|selection| {
                let mut items = state.items.clone();
                if let Some(component) = state.continuous_radius_component.as_ref() {
                    if !continuous_radius_diagnostics.is_empty() || continuous_radius_solutions.len() != component.prototypes.len() {
                        return Err("continuous-radius solution is incomplete or inconsistent".into());
                    }
                    let diagnostics = component.apply_solutions_to_items(&mut items, &continuous_radius_solutions);
                    if !diagnostics.is_empty() { return Err(diagnostics.join("; ")); }
                }
                let packed = LayerTraceReplayAdapter::new().replay_assignments(LayerAssignmentTraceReplayRequest {
                    selection: &selection, layers: &assignment.layers, bins: &assignment.bins, items: &items,
                    block_traces: &state.layer_block_traces, placement_traces: &state.layer_placement_traces,
                    max_placements: validation.max_placements, extra_geometry: validation.extra_geometry.as_deref(),
                }).map_err(|errors| errors.join("; "))?;
                validate_strict_final_packing(state, &selection, &items, &packed, amounts)?;
                Ok((selection, packed))
            });
            let mut info = solve.info;
            info.insert("executor".into(), "trace_validated_meta_model_final".into());
            info.insert("backend".into(), backend.name().into());
            diagnostics.write_bpp3d_info(&mut info);
            let execution = match packed {
                Ok((validated_selection, packed_bins)) => {
                    let layers = validated_selection.assignments.iter()
                        .map(|entry| assignment.layers[entry.layer_index].clone()).collect::<Vec<_>>();
                    info.insert("status".into(), "physically_validated".into());
                    info.insert("selected_assignment_count".into(), layers.len().to_string());
                    info.insert("packed_bin_count".into(), packed_bins.len().to_string());
                    for solution in &continuous_radius_solutions {
                        info.insert(ContinuousRadiusModelComponent::solution_info_key(&solution.variable_name), solution.radius.to_string());
                        if let Some(value) = solution.radius_squared { info.insert(ContinuousRadiusModelComponent::radius_squared_info_key(&solution.variable_name), value.to_string()); }
                        if let Some(value) = solution.segment_index { info.insert(ContinuousRadiusModelComponent::segment_info_key(&solution.variable_name), value.to_string()); }
                    }
                    ColumnGenerationFinalExecution { layers, packed_bins, objective: solve.objective,
                        final_solved: true, diagnostics: Some(diagnostics), info }
                }
                Err(error) => {
                    info.insert("status".into(), "physical_validation_failed".into()); info.insert("error".into(), error);
                    ColumnGenerationFinalExecution { layers: Vec::new(), packed_bins: Vec::new(), objective: None,
                        final_solved: false, diagnostics: Some(diagnostics), info }
                }
            };
            return LayerAssignmentFinalExecution { execution, selection };
        }
        let selected_assignments = assignment.x.as_ref().map(|x| {
            SolutionExtractor::extract_indexed_binary_2(&solve.primal_solution, x)
        }).unwrap_or_default();
        let selected_layer_indices = state.layers
            .iter()
            .enumerate()
            .filter_map(|(layer_index, _)| {
                let selected = assignment.bins
                    .iter()
                    .enumerate()
                    .any(|(bin_index, _)| {
                        selected_assignments
                            .get(&(bin_index, layer_index))
                            .copied()
                            .unwrap_or(false)
                    });
                selected.then_some(layer_index)
            })
            .collect::<Vec<_>>();
        let layers = selected_layer_indices
            .iter()
            .filter_map(|index| state.layers.get(*index).cloned())
            .collect::<Vec<_>>();
        let output_layers = if solve.primal_solution.is_empty() {
            state.layers.clone()
        } else {
            layers
        };
        let output_layer_indices = if solve.primal_solution.is_empty() {
            (0..state.layers.len()).collect::<Vec<_>>()
        } else {
            selected_layer_indices
        };
        let trace_layer_count = output_layers
            .iter()
            .filter(|layer| !layer.demand_coverage.is_empty())
            .count();
        let (packed_bins, mut packing_diagnostics) = packed_bins_from_selected_layers(
            &output_layers,
            &output_layer_indices,
            state,
        );
        if state.bins.is_empty() && bins_from_layers(&state.layers).is_empty() {
            packing_diagnostics.push("no available bin type for final packing".to_string());
        }
        let mut info = HashMap::from([
            ("executor".to_string(), "meta_model_final".to_string()),
            ("backend".to_string(), backend.name().to_string()),
            ("status".to_string(), "registered".to_string()),
            ("selected_assignment_count".to_string(), selected_assignments.len().to_string()),
            ("selected_layer_count".to_string(), output_layers.len().to_string()),
            ("selected_layer_with_coverage_count".to_string(), trace_layer_count.to_string()),
            ("packed_bin_count".to_string(), packed_bins.len().to_string()),
            (
                "framework_lifecycle_solution_len".to_string(),
                lifecycle.solution().map(|solution| solution.len()).unwrap_or(0).to_string(),
            ),
            (
                "framework_lifecycle_flush_count".to_string(),
                lifecycle.flush_count().to_string(),
            ),
            (
                "continuous_radius_model_variable_count".to_string(),
                continuous_radius_registration.variable_count().to_string(),
            ),
            (
                "continuous_radius_model_constraint_count".to_string(),
                continuous_radius_registration.constraint_count.to_string(),
            ),
            (
                "continuous_radius_model_objective_term_count".to_string(),
                continuous_radius_registration.objective_term_count.to_string(),
            ),
            (
                "continuous_radius_selected_count".to_string(),
                continuous_radius_solutions.len().to_string(),
            ),
        ]);
        for solution in &continuous_radius_solutions {
            info.insert(
                ContinuousRadiusModelComponent::solution_info_key(&solution.variable_name),
                solution.radius.to_string(),
            );
            if let Some(radius_squared) = solution.radius_squared {
                info.insert(
                    ContinuousRadiusModelComponent::radius_squared_info_key(&solution.variable_name),
                    radius_squared.to_string(),
                );
            }
            if let Some(segment_index) = solution.segment_index {
                info.insert(
                    ContinuousRadiusModelComponent::segment_info_key(&solution.variable_name),
                    segment_index.to_string(),
                );
            }
        }
        if !continuous_radius_diagnostics.is_empty() {
            info.insert(
                "continuous_radius_diagnostics".to_string(),
                continuous_radius_diagnostics.join("; "),
            );
        }
        if !packing_diagnostics.is_empty() {
            info.insert(
                "packing_diagnostics".to_string(),
                packing_diagnostics.join("; "),
            );
        }
        if !packed_bins.is_empty() {
            info.insert(
                "final_diagnostics".to_string(),
                "selected_layers_renderable".to_string(),
            );
        } else if output_layers.is_empty() {
            info.insert(
                "final_diagnostics".to_string(),
                "no_selected_layer_assignment".to_string(),
            );
        } else if trace_layer_count == 0 {
            info.insert(
                "final_diagnostics".to_string(),
                "selected_layers_without_coverage_trace".to_string(),
            );
        } else {
            info.insert(
                "final_diagnostics".to_string(),
                "selected_layers_have_coverage_trace".to_string(),
            );
        }
        info.extend(solve.info);
        diagnostics.write_bpp3d_info(&mut info);
        ColumnGenerationFinalExecution {
            layers: output_layers,
            packed_bins,
            objective: solve.objective,
            final_solved: true,
            diagnostics: Some(diagnostics),
            info,
        }.into()
    }
}
