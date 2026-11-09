// ColumnGenerationApplicationService - 列生成应用服务 / Application service
// ============================================================================

/// 列生成应用服务 / Column generation application service
///
/// 提供轻量应用入口，组合配置、层生成和装箱分析。
/// Provides a lightweight application entry point composing config, layer
/// generation, and packing analysis.
#[derive(Debug, Clone)]
pub struct ColumnGenerationApplicationService<G = PackingGeometryGuard> {
    /// 配置 / Config
    pub config: ColumnGenerationConfig,
    /// 装箱分析器 / Packing analyzer
    pub packing_analyzer: ColumnGenerationPackingAnalyzer<G>,
}

impl Default for ColumnGenerationApplicationService<PackingGeometryGuard> {
    fn default() -> Self {
        Self::new(ColumnGenerationConfig::default())
    }
}

impl ColumnGenerationApplicationService<PackingGeometryGuard> {
    /// 创建应用服务 / Create application service
    pub fn new(config: ColumnGenerationConfig) -> Self {
        Self {
            config,
            packing_analyzer: ColumnGenerationPackingAnalyzer::new(),
        }
    }
}

impl<G> ColumnGenerationApplicationService<G> {
    /// 使用自定义几何校验策略创建应用服务 / Create application service with custom geometry validation strategy
    pub fn with_geometry_guard(config: ColumnGenerationConfig, geometry_guard: G) -> Self {
        Self {
            config,
            packing_analyzer: ColumnGenerationPackingAnalyzer::with_geometry_guard(
                geometry_guard,
            ),
        }
    }
}

impl<G> ColumnGenerationApplicationService<G>
where
    G: PackingGeometryContract<f64, Meter>,
{
    /// 创建列生成算法 / Create column generation algorithm
    pub fn create_algorithm<V, U>(&self) -> ColumnGenerationAlgorithm<V, U>
    where
        V: Field + Clone + Debug + Send + Sync + PartialEq + num_traits::FloatConst,
        U: UnitTrait + Debug + Clone + Send + Sync,
    {
        self.create_algorithm_with(|config| ColumnGenerationAlgorithm::new(config))
    }

    /// 使用工厂创建列生成算法 / Create column generation algorithm with a factory
    pub fn create_algorithm_with<V, U, F>(&self, factory: F) -> ColumnGenerationAlgorithm<V, U>
    where
        V: Field + Clone + Debug + Send + Sync + PartialEq + num_traits::FloatConst,
        U: UnitTrait + Debug + Clone + Send + Sync,
        F: FnOnce(ColumnGenerationConfig) -> ColumnGenerationAlgorithm<V, U>,
    {
        factory(self.config.clone())
    }

    /// 分析装箱结果 / Analyze packing result
    pub fn analyze_packing<V, U>(
        &self,
        packed_bins: Vec<PackedBin<V, U>>,
    ) -> Result<ColumnGenerationPackingAnalysis<V, U>, Vec<String>>
    where
        V: num_traits::Float + Field + Clone + Debug + Send + Sync + PartialOrd + num_traits::FloatConst + Into<f64>,
        U: CTUnit + Default + Clone + Debug + Send + Sync,
        G: PackingGeometryContract<V, U>,
    {
        self.packing_analyzer.analyze(packed_bins)
    }

    /// 运行 materialized request / Run materialized request
    ///
    /// 该入口只编排 executor，不直接注册 MetaModel。
    /// This entry only orchestrates executors and does not register MetaModel directly.
    pub fn run_materialized(
        &self,
        items: Vec<ActualItem<f64, Meter>>,
        initial_layers: Vec<BinLayer<f64, Meter>>,
        rmp_executor: &dyn ColumnGenerationRmpExecutor,
        final_executor: &dyn ColumnGenerationFinalExecutor,
    ) -> Result<ColumnGenerationApplicationFlowResult, Vec<String>> {
        self.run_materialized_with_bins(
            items,
            Vec::new(),
            initial_layers,
            rmp_executor,
            final_executor,
        )
    }

    /// 运行带箱型的 materialized request / Run materialized request with bins
    ///
    /// 该入口只传递完整应用状态给 executor，不直接注册 MetaModel。
    /// This entry only passes complete application state to executors and does not register MetaModel directly.
    pub fn run_materialized_with_bins(
        &self,
        items: Vec<ActualItem<f64, Meter>>,
        bins: Vec<BinType<f64, Meter>>,
        initial_layers: Vec<BinLayer<f64, Meter>>,
        rmp_executor: &dyn ColumnGenerationRmpExecutor,
        final_executor: &dyn ColumnGenerationFinalExecutor,
    ) -> Result<ColumnGenerationApplicationFlowResult, Vec<String>> {
        self.run_materialized_with_bins_and_continuous_radius(
            items,
            bins,
            initial_layers,
            Vec::new(),
            None,
            rmp_executor,
            final_executor,
        )
    }

    /// 使用生成结果中的层和原始 trace 作为初始列；评分等生成元数据不参与此入口。
    /// Use layers and original traces from generation results as initial columns; scoring and other generation metadata are not used by this entry.
    pub fn run_materialized_with_traced_initial_layers(
        &self,
        items: Vec<ActualItem<f64, Meter>>,
        bins: Vec<BinType<f64, Meter>>,
        initial_results: Vec<LayerGenerationResult<f64, Meter>>,
        rmp_executor: &dyn ColumnGenerationRmpExecutor,
        final_executor: &dyn ColumnGenerationFinalExecutor,
    ) -> Result<ColumnGenerationApplicationFlowResult, Vec<String>> {
        let (initial_layers, traces) = split_traced_initial_layers(initial_results);
        self.run_materialized_with_bins_and_continuous_radius(
            items,
            bins,
            initial_layers,
            traces,
            None,
            rmp_executor,
            final_executor,
        )
    }

    fn run_materialized_with_bins_and_continuous_radius(
        &self,
        items: Vec<ActualItem<f64, Meter>>,
        bins: Vec<BinType<f64, Meter>>,
        initial_layers: Vec<BinLayer<f64, Meter>>,
        initial_traces: Vec<LayerTracePair>,
        continuous_radius_component: Option<ContinuousRadiusModelComponent>,
        rmp_executor: &dyn ColumnGenerationRmpExecutor,
        final_executor: &dyn ColumnGenerationFinalExecutor,
    ) -> Result<ColumnGenerationApplicationFlowResult, Vec<String>> {
        let mut algorithm = self.create_algorithm::<f64, Meter>();
        let initial_layers = ensure_layer_demand_coverage(initial_layers, &items);
        let (block_traces, placement_traces) =
            add_initial_layers_with_traces(&mut algorithm, &initial_layers, initial_traces);
        let mut state = application_state_from_algorithm_with_continuous_radius(
            &algorithm,
            items,
            bins,
            initial_layers,
            block_traces,
            placement_traces,
            continuous_radius_component,
        );
        let rmp = rmp_executor
            .execute_result(&state)
            .map_err(|error| vec![format!("{:?}: {}", error.stage, error.message)])?;
        algorithm.state.additional_shadow_prices = rmp.additional_shadow_prices.clone();
        state.additional_shadow_prices = algorithm.state.additional_shadow_prices.clone();
        if let Some(objective) = rmp.objective {
            algorithm.state.observe_objective(
                objective,
                self.config.objective_sense,
                self.config.reduced_cost_tolerance,
            );
        }
        let final_execution = final_executor
            .execute_result(&state)
            .map_err(|error| vec![format!("{:?}: {}", error.stage, error.message)])?;
        let packing_analysis = if final_execution.packed_bins.is_empty() {
            None
        } else {
            Some(self.analyze_packing(final_execution.packed_bins.clone())?)
        };
        let mut info = HashMap::new();
        info.extend(rmp.info.iter().map(|(k, v)| (format!("rmp_{}", k), v.clone())));
        info.extend(rmp.additional_shadow_prices.iter().map(|(k, v)| {
            (format!("rmp_additional_shadow_price_{}", k), v.to_string())
        }));
        info.extend(final_execution.info.iter().map(|(k, v)| (format!("final_{}", k), v.clone())));
        let result = ColumnGenerationResult {
            state: algorithm.state,
            layers: final_execution.layers.clone(),
            packing_result: packing_analysis.as_ref().map(|analysis| analysis.packing_result.clone()),
            render_loading_plans: packing_analysis
                .map(|analysis| analysis.render_loading_plans)
                .unwrap_or_default(),
            info,
        };
        Ok(ColumnGenerationApplicationFlowResult {
            result,
            rmp,
            final_execution,
            selected_radius_solutions: Vec::new(),
        })
    }

    /// 运行一轮 RMP -> 生成列 -> RMP refresh -> final / Run one RMP -> generate columns -> RMP refresh -> final round
    ///
    /// 该入口验证 shadow price 能穿过 application 编排进入 layer generation。
    /// This entry verifies shadow prices pass through application orchestration into layer generation.
    pub fn run_materialized_one_generation_round(
        &self,
        items: Vec<ActualItem<f64, Meter>>,
        bins: Vec<BinType<f64, Meter>>,
        initial_layers: Vec<BinLayer<f64, Meter>>,
        layer_generation: LayerGenerationContext<f64, Meter>,
        rmp_executor: &dyn ColumnGenerationRmpExecutor,
        final_executor: &dyn ColumnGenerationFinalExecutor,
    ) -> Result<ColumnGenerationApplicationFlowResult, Vec<String>> {
        self.run_materialized_one_generation_round_with_package_attributes(
            items,
            bins,
            initial_layers,
            Vec::new(),
            None,
            HashMap::new(),
            None,
            layer_generation,
            rmp_executor,
            final_executor,
        )
    }

    /// 使用含原始 trace 的初始列执行一轮生成；后续新列保留各自的 trace。
    /// Run one generation round with original traces on initial columns; subsequent columns retain their own traces.
    pub fn run_materialized_one_generation_round_with_traced_initial_layers(
        &self,
        items: Vec<ActualItem<f64, Meter>>,
        bins: Vec<BinType<f64, Meter>>,
        initial_results: Vec<LayerGenerationResult<f64, Meter>>,
        layer_generation: LayerGenerationContext<f64, Meter>,
        rmp_executor: &dyn ColumnGenerationRmpExecutor,
        final_executor: &dyn ColumnGenerationFinalExecutor,
    ) -> Result<ColumnGenerationApplicationFlowResult, Vec<String>> {
        let (initial_layers, traces) = split_traced_initial_layers(initial_results);
        self.run_materialized_one_generation_round_with_package_attributes(
            items,
            bins,
            initial_layers,
            traces,
            None,
            HashMap::new(),
            None,
            layer_generation,
            rmp_executor,
            final_executor,
        )
    }

    fn run_materialized_one_generation_round_with_package_attributes(
        &self,
        items: Vec<ActualItem<f64, Meter>>,
        bins: Vec<BinType<f64, Meter>>,
        initial_layers: Vec<BinLayer<f64, Meter>>,
        initial_traces: Vec<LayerTracePair>,
        continuous_radius_component: Option<ContinuousRadiusModelComponent>,
        package_attributes: HashMap<String, PackageAttribute>,
        item_amounts: Option<&[u64]>,
        layer_generation: LayerGenerationContext<f64, Meter>,
        rmp_executor: &dyn ColumnGenerationRmpExecutor,
        final_executor: &dyn ColumnGenerationFinalExecutor,
    ) -> Result<ColumnGenerationApplicationFlowResult, Vec<String>> {
        let mut algorithm = ColumnGenerationAlgorithm::with_layer_generation(
            self.config.clone(),
            layer_generation,
        );
        let initial_layers = ensure_layer_demand_coverage(initial_layers, &items);
        let (mut layer_block_traces, mut layer_placement_traces) =
            add_initial_layers_with_traces(&mut algorithm, &initial_layers, initial_traces);
        let initial_state = application_state_from_algorithm_with_continuous_radius(
            &algorithm,
            items.clone(),
            bins.clone(),
            initial_layers.clone(),
            layer_block_traces.clone(),
            layer_placement_traces.clone(),
            continuous_radius_component.clone(),
        );
        let first_rmp = match item_amounts {
            Some(amounts) => rmp_executor.execute_result_with_item_amounts(&initial_state, amounts),
            None => rmp_executor.execute_result(&initial_state),
        }
            .map_err(|error| vec![format!("{:?}: {}", error.stage, error.message)])?;
        algorithm.state.additional_shadow_prices = first_rmp.additional_shadow_prices.clone();
        if let Some(objective) = first_rmp.objective {
            algorithm.state.observe_objective(
                objective,
                self.config.objective_sense,
                self.config.reduced_cost_tolerance,
            );
        }

        let demand_entries = layer_generation_demand_entries(&items, &first_rmp.shadow_price_summary, item_amounts);
        let shadow_prices = layer_generation_shadow_prices(&first_rmp.shadow_price_summary);
        let mut request = LayerGenerationRequest::new(algorithm.state.iteration as i64, items.clone())
            .with_demand_entries(demand_entries)
            .with_shadow_prices(shadow_prices)
            .with_package_attributes(package_attributes)
            .with_max_candidates(self.config.max_candidates_per_iteration);
        request.bin = bins.first().cloned();
        // 多箱型：把全部候选箱型交给层生成，使每种箱型都能产出候选列；列通过 `BinLayer.bin` 携带箱型。
        // Multiple bin types: hand every candidate bin type to layer generation so each type produces
        // candidates; each column carries its type through `BinLayer.bin`.
        request.bins = layer_generation_candidate_bins(&bins);
        request.existing_layers = algorithm.active_layers();
        let generated = algorithm.layer_generation.generate(&request);
        let generated_layers = ensure_layer_demand_coverage(
            generated.iter().map(|result| result.layer.clone()).collect(),
            &items,
        );
        let mut traces_by_key = HashMap::new();
        for (layer, result) in generated_layers.iter().zip(generated) {
            traces_by_key.entry(layer.identity_key())
                .or_insert((result.block_traces, result.placement_traces));
        }
        let added = algorithm.add_columns(algorithm.state.iteration, generated_layers);
        let added_keys = added.iter()
            .map(BinLayer::identity_key)
            .collect::<std::collections::HashSet<_>>();
        for (index, layer) in algorithm.active_layers().iter().enumerate() {
            let key = layer.identity_key();
            if !added_keys.contains(&key) { continue; }
            if let Some((blocks, placements)) = traces_by_key.remove(&key) {
                if !blocks.is_empty() { layer_block_traces.insert(index, blocks); }
                if !placements.is_empty() { layer_placement_traces.insert(index, placements); }
            }
        }
        algorithm.state.advance_iteration();

        let mut refreshed_state = application_state_from_algorithm_with_continuous_radius(
            &algorithm,
            items,
            bins,
            initial_layers,
            layer_block_traces,
            layer_placement_traces,
            continuous_radius_component,
        );
        let rmp = match item_amounts {
            Some(amounts) => rmp_executor.execute_result_with_item_amounts(&refreshed_state, amounts),
            None => rmp_executor.execute_result(&refreshed_state),
        }
            .map_err(|error| vec![format!("{:?}: {}", error.stage, error.message)])?;
        algorithm.state.additional_shadow_prices = rmp.additional_shadow_prices.clone();
        refreshed_state.additional_shadow_prices = algorithm.state.additional_shadow_prices.clone();
        if let Some(objective) = rmp.objective {
            algorithm.state.observe_objective(
                objective,
                self.config.objective_sense,
                self.config.reduced_cost_tolerance,
            );
        }
        let final_execution = match item_amounts {
            Some(amounts) => final_executor.execute_result_with_item_amounts(&refreshed_state, amounts),
            None => final_executor.execute_result(&refreshed_state),
        }
            .map_err(|error| vec![format!("{:?}: {}", error.stage, error.message)])?;
        let packing_analysis = if final_execution.packed_bins.is_empty() {
            None
        } else {
            Some(self.analyze_packing(final_execution.packed_bins.clone())?)
        };
        let mut info = HashMap::from([
            ("first_rmp_objective".to_string(), format!("{:?}", first_rmp.objective)),
            ("generated_layer_count".to_string(), (algorithm.state.total_columns.saturating_sub(refreshed_state.initial_layers.len())).to_string()),
        ]);
        info.extend(rmp.info.iter().map(|(k, v)| (format!("rmp_{}", k), v.clone())));
        info.extend(rmp.additional_shadow_prices.iter().map(|(k, v)| {
            (format!("rmp_additional_shadow_price_{}", k), v.to_string())
        }));
        info.extend(final_execution.info.iter().map(|(k, v)| (format!("final_{}", k), v.clone())));
        let result = ColumnGenerationResult {
            state: algorithm.state,
            layers: final_execution.layers.clone(),
            packing_result: packing_analysis.as_ref().map(|analysis| analysis.packing_result.clone()),
            render_loading_plans: packing_analysis
                .map(|analysis| analysis.render_loading_plans)
                .unwrap_or_default(),
            info,
        };
        Ok(ColumnGenerationApplicationFlowResult {
            result,
            rmp,
            final_execution,
            selected_radius_solutions: Vec::new(),
        })
    }

    /// 运行 CSV materialized request / Run CSV materialized request
    #[cfg(feature = "serde")]
    pub fn run_csv_materialized(
        &self,
        request: CsvMaterializedApplicationRequest,
        rmp_executor: &dyn ColumnGenerationRmpExecutor,
        final_executor: &dyn ColumnGenerationFinalExecutor,
    ) -> Result<ColumnGenerationApplicationFlowResult, Vec<String>> {
        reject_grouped_csv_demand(&request, "run_csv_materialized")?;
        let continuous_radius_component = request.continuous_radius_component.clone();
        let mut flow = self.run_materialized_with_bins_and_continuous_radius(
            request.items,
            Vec::new(),
            request.initial_layers,
            Vec::new(),
            continuous_radius_component.clone(),
            rmp_executor,
            final_executor,
        )?;
        apply_continuous_radius_solutions(self, &mut flow, continuous_radius_component.as_ref())?;
        append_continuous_radius_info(&mut flow.result.info, continuous_radius_component.as_ref());
        Ok(flow)
    }

    /// 运行带箱型的 CSV materialized request / Run CSV materialized request with bins
    #[cfg(feature = "serde")]
    pub fn run_csv_materialized_with_bins(
        &self,
        request: CsvMaterializedApplicationRequest,
        rmp_executor: &dyn ColumnGenerationRmpExecutor,
        final_executor: &dyn ColumnGenerationFinalExecutor,
    ) -> Result<ColumnGenerationApplicationFlowResult, Vec<String>> {
        reject_grouped_csv_demand(&request, "run_csv_materialized_with_bins")?;
        let continuous_radius_component = request.continuous_radius_component.clone();
        let mut flow = self.run_materialized_with_bins_and_continuous_radius(
            request.items,
            request.bins,
            request.initial_layers,
            Vec::new(),
            continuous_radius_component.clone(),
            rmp_executor,
            final_executor,
        )?;
        apply_continuous_radius_solutions(self, &mut flow, continuous_radius_component.as_ref())?;
        append_continuous_radius_info(&mut flow.result.info, continuous_radius_component.as_ref());
        Ok(flow)
    }

    /// 运行一轮 CSV materialized request / Run one-round CSV materialized request
    #[cfg(feature = "serde")]
    pub fn run_csv_materialized_one_generation_round(
        &self,
        request: CsvMaterializedApplicationRequest,
        layer_generation: LayerGenerationContext<f64, Meter>,
        rmp_executor: &dyn ColumnGenerationRmpExecutor,
        final_executor: &dyn ColumnGenerationFinalExecutor,
    ) -> Result<ColumnGenerationApplicationFlowResult, Vec<String>> {
        let item_amounts = ordered_csv_item_amounts(&request)?;
        let continuous_radius_component = request.continuous_radius_component.clone();
        let package_attributes = request
            .package_attributes
            .into_iter()
            .collect::<HashMap<_, _>>();
        let mut flow = self.run_materialized_one_generation_round_with_package_attributes(
            request.items,
            request.bins,
            request.initial_layers,
            Vec::new(),
            continuous_radius_component.clone(),
            package_attributes,
            Some(&item_amounts),
            layer_generation,
            rmp_executor,
            final_executor,
        )?;
        apply_continuous_radius_solutions(self, &mut flow, continuous_radius_component.as_ref())?;
        append_continuous_radius_info(&mut flow.result.info, continuous_radius_component.as_ref());
        Ok(flow)
    }

    /// 一轮 CSV 流程携带按初始层顺序提供的物理 trace；空 coverage 从实际 trace 生成。
    /// Run one CSV round with physical traces in initial-layer order; derive empty coverage from actual traces.
    ///
    /// 分组数量通过支持数量的执行器传递；调用方须选择严格 final 来验证物理与需求可行性。
    /// Grouped quantities require quantity-aware executors; callers must select strict final to validate physical and demand feasibility.
    #[cfg(feature = "serde")]
    pub fn run_csv_materialized_one_generation_round_with_traced_initial_layers(
        &self,
        mut request: CsvMaterializedApplicationRequest,
        initial_traces: Vec<LayerTracePair>,
        layer_generation: LayerGenerationContext<f64, Meter>,
        rmp_executor: &dyn ColumnGenerationRmpExecutor,
        final_executor: &dyn ColumnGenerationFinalExecutor,
    ) -> Result<ColumnGenerationApplicationFlowResult, Vec<String>> {
        let item_amounts = prepare_csv_traced_initial_layers(&mut request, &initial_traces)?;
        let continuous_radius_component = request.continuous_radius_component.clone();
        let package_attributes = request.package_attributes.into_iter().collect();
        let mut flow = self.run_materialized_one_generation_round_with_package_attributes(
            request.items,
            request.bins,
            request.initial_layers,
            initial_traces,
            continuous_radius_component.clone(),
            package_attributes,
            Some(&item_amounts),
            layer_generation,
            rmp_executor,
            final_executor,
        )?;
        apply_continuous_radius_solutions(self, &mut flow, continuous_radius_component.as_ref())?;
        append_continuous_radius_info(&mut flow.result.info, continuous_radius_component.as_ref());
        Ok(flow)
    }
}
