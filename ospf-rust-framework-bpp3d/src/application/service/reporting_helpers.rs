// ============================================================================
// 候选箱型排序 / Candidate bin ordering
// ============================================================================

/// 排序列出参与层生成的候选箱型 / Order the candidate bin types taking part in layer generation
///
/// 主箱型排在最前，其余按传入顺序去重，使每种箱型都能产出候选列。
/// The main bin type comes first and the rest keep their input order, deduplicated, so that every bin type
/// can produce candidate columns.
fn layer_generation_candidate_bins(bins: &[BinType<f64, Meter>]) -> Vec<BinType<f64, Meter>> {
    let mut ordered: Vec<BinType<f64, Meter>> = Vec::new();
    if let Some(main_bin) = bins.iter().find(|bin| bin.is_main) {
        ordered.push(main_bin.clone());
    }
    for bin in bins {
        if !ordered
            .iter()
            .any(|existing| existing.type_code == bin.type_code)
        {
            ordered.push(bin.clone());
        }
    }
    ordered
}

fn packed_bins_from_selected_layers(
    layers: &[BinLayer<f64, Meter>],
    layer_indices: &[usize],
    state: &ColumnGenerationApplicationState,
) -> (Vec<PackedBin<f64, Meter>>, Vec<String>) {
    let replay = LayerTraceReplayAdapter::new().replay_selected_layers(
        layers,
        layer_indices,
        &state.items,
        &state.bins,
        &state.layer_block_traces,
        &state.layer_placement_traces,
    );
    (replay.packed_bins, replay.diagnostics)
}

fn merge_noop_solve_result(
    mut solve: MetaModelExecutorSolveResult,
    fallback_objective: Option<f64>,
    fallback_primal_solution: Vec<f64>,
    fallback_dual_solution: Vec<f64>,
) -> MetaModelExecutorSolveResult {
    if solve.objective.is_none() {
        solve.objective = fallback_objective;
    }
    if solve.primal_solution.is_empty() {
        solve.primal_solution = fallback_primal_solution;
    }
    if solve.dual_solution.is_empty() {
        solve.dual_solution = fallback_dual_solution;
    }
    solve
}

fn validate_item_amounts(items: &[ActualItem<f64, Meter>], amounts: &[u64]) -> Result<(), String> {
    if items.len() != amounts.len() {
        return Err("item demand count does not match item catalog".into());
    }
    let mut ids = std::collections::HashSet::new();
    for (item, amount) in items.iter().zip(amounts) {
        if !ids.insert(item.id.as_str()) || *amount == 0 || *amount > 9_007_199_254_740_992 {
            return Err("item demand requires unique IDs and positive exactly representable amounts".into());
        }
    }
    Ok(())
}

fn item_demand_entries(
    items: &[ActualItem<f64, Meter>],
    amounts: Option<&[u64]>,
) -> Result<Vec<Bpp3dDemandEntry>, String> {
    if let Some(amounts) = amounts { validate_item_amounts(items, amounts)?; }
    Ok(items
        .iter()
        .enumerate()
        .map(|(index, item)| Bpp3dDemandEntry {
            mode: Bpp3dDemandMode::Item,
            key: Bpp3dDemandKey::Item { id: item.id.clone() },
            demand: amounts.map(|amounts| amounts[index] as f64).unwrap_or(1.0),
        })
        .collect())
}

/// 严格回放分组需求方案并检查实际层覆盖、箱载重和精确件数；不证明求解器身份或最优性。
/// Strictly replay a grouped-demand plan and check actual layer coverage, bin loads and exact counts; this proves neither solver provenance nor optimality.
pub fn replay_strict_grouped_packing(
    state: &ColumnGenerationApplicationState,
    selection: &PreciseAssignmentSelection,
    amounts: &[u64],
    max_placements: usize,
) -> Result<Vec<PackedBin<f64, Meter>>, String> {
    validate_item_amounts(&state.items, amounts)?;
    let packed = LayerTraceReplayAdapter::new().replay_assignments(LayerAssignmentTraceReplayRequest {
        selection, layers: &state.layers, bins: &state.bins, items: &state.items,
        block_traces: &state.layer_block_traces, placement_traces: &state.layer_placement_traces,
        max_placements, extra_geometry: None,
    }).map_err(|errors| errors.join("; "))?;
    validate_strict_final_packing(state, selection, &state.items, &packed, Some(amounts))?;
    Ok(packed)
}

fn validate_strict_final_packing(
    state: &ColumnGenerationApplicationState,
    selection: &PreciseAssignmentSelection,
    items: &[ActualItem<f64, Meter>],
    packed_bins: &[PackedBin<f64, Meter>],
    amounts: Option<&[u64]>,
) -> Result<(), String> {
    if let Some(amounts) = amounts { validate_item_amounts(items, amounts)?; }
    let mut indices = HashMap::new();
    for (index, item) in items.iter().enumerate() {
        if indices.insert(item.id.as_str(), index).is_some() {
            return Err("duplicate item ID in final demand".into());
        }
        if !item.weight.value.is_finite() || item.weight.value < 0.0 {
            return Err("invalid item weight in final demand".into());
        }
    }
    let mut checked_layers = std::collections::HashSet::new();
    for assignment in &selection.assignments {
        let index = assignment.layer_index;
        if !checked_layers.insert(index) { continue; }
        let layer = state.layers.get(index).ok_or("unknown selected layer")?;
        let mut actual = HashMap::<usize, u64>::new();
        let mut add_count = |item_index: usize, count: u64| -> Result<(), String> {
            let entry = actual.entry(item_index).or_default();
            *entry = entry.checked_add(count).ok_or("physical item count overflows")?;
            Ok(())
        };
        match state.layer_block_traces.get(&index).filter(|blocks| !blocks.is_empty()) {
            Some(blocks) => for block in blocks { add_count(block.item_index, block.item_count)?; },
            None => for trace in state.layer_placement_traces.get(&index).into_iter().flatten() {
                add_count(trace.item_index, trace.amount)?;
            },
        }
        let mut declared = HashMap::new();
        for coverage in layer.demand_coverage.iter().filter(|coverage| coverage.mode == Bpp3dDemandMode::Item) {
            let Bpp3dDemandKey::Item { id } = &coverage.key else {
                return Err("item demand coverage has a non-item key".into());
            };
            let item_index = *indices.get(id.as_str()).ok_or("item demand coverage references an unknown ID")?;
            if !coverage.coefficient.is_finite() || coverage.coefficient < 0.0
                || declared.insert(item_index, coverage.coefficient).is_some()
            {
                return Err("invalid or duplicate item demand coverage".into());
            }
        }
        for (item_index, count) in actual {
            let coefficient = declared.remove(&item_index).unwrap_or(0.0);
            if (coefficient - count as f64).abs() > 1e-7 {
                return Err("physical trace disagrees with item demand coverage".into());
            }
        }
        if declared.values().any(|coefficient| coefficient.abs() > 1e-7) {
            return Err("item demand coverage has no matching physical trace".into());
        }
    }
    let mut counts = vec![0_u64; items.len()];
    for bin in packed_bins {
        let capacity = bin.bin_type.capacity.value;
        if !capacity.is_finite() || capacity < 0.0 { return Err("invalid bin weight capacity".into()); }
        let mut weight = 0.0;
        for packed in &bin.items {
            let count = counts.get_mut(packed.item_index).ok_or("packed item index is unknown")?;
            *count = count.checked_add(1).ok_or("packed item count overflows")?;
            weight += items[packed.item_index].weight.value;
        }
        if !weight.is_finite() || weight > capacity + 1e-7 {
            return Err("physical packing exceeds bin weight capacity".into());
        }
    }
    if counts.iter().enumerate().any(|(index, count)| *count != amounts.map(|amounts| amounts[index]).unwrap_or(1)) {
        return Err(if amounts.is_some() {
            "physical packing does not satisfy each item demand exactly".into()
        } else {
            "physical packing does not satisfy each item demand exactly once".into()
        });
    }
    Ok(())
}

fn bins_from_layers(layers: &[BinLayer<f64, Meter>]) -> Vec<BinType<f64, Meter>> {
    layers
        .iter()
        .filter_map(|layer| layer.bin.clone())
        .collect()
}

fn demand_key_name(entry: &Bpp3dDemandEntry) -> String {
    match &entry.key {
        Bpp3dDemandKey::Item { id } => format!("item:{}", id),
        Bpp3dDemandKey::Material { no } => format!("material:{}", no),
    }
}

#[cfg(feature = "serde")]
fn demand_mode_name(mode: Bpp3dDemandMode) -> &'static str {
    match mode {
        Bpp3dDemandMode::Item => "item",
        Bpp3dDemandMode::Material => "material",
        Bpp3dDemandMode::ItemAmount => "item_amount",
        Bpp3dDemandMode::ItemWeight => "item_weight",
        Bpp3dDemandMode::ItemMaterialAmount => "item_material_amount",
        Bpp3dDemandMode::ItemMaterialWeight => "item_material_weight",
    }
}

#[cfg(feature = "serde")]
fn demand_key_report_name(key: &Bpp3dDemandKey) -> String {
    match key {
        Bpp3dDemandKey::Item { id } => format!("item:{}", id),
        Bpp3dDemandKey::Material { no } => format!("material:{}", no),
    }
}

#[cfg(feature = "serde")]
fn selected_layer_reports(
    layers: &[BinLayer<f64, Meter>],
) -> Vec<super::report::Bpp3dSelectedLayerReport> {
    layers
        .iter()
        .enumerate()
        .map(|(index, layer)| super::report::Bpp3dSelectedLayerReport {
            index,
            iteration: layer.iteration,
            from: layer.from.clone(),
            bin_type: layer.bin.as_ref().map(|bin| bin.type_code.to_string()),
            depth: layer.depth.value,
            coverage: layer
                .demand_coverage
                .iter()
                .map(|coverage| super::report::Bpp3dDemandCoverageReport {
                    mode: demand_mode_name(coverage.mode).to_string(),
                    key: demand_key_report_name(&coverage.key),
                    coefficient: coverage.coefficient,
                })
                .collect(),
        })
        .collect()
}

#[cfg(feature = "serde")]
fn packed_bin_reports(
    packed_bins: &[PackedBin<f64, Meter>],
) -> Vec<super::report::Bpp3dPackedBinReport> {
    packed_bins
        .iter()
        .map(|bin| {
            let mut loading_orders = bin
                .items
                .iter()
                .map(|item| item.loading_order)
                .collect::<Vec<_>>();
            loading_orders.sort_unstable();
            super::report::Bpp3dPackedBinReport {
                name: bin.name.clone(),
                bin_type: bin.bin_type.type_code.to_string(),
                item_count: bin.items.len(),
                batch_no: bin.batch_no.clone(),
                loading_orders,
            }
        })
        .collect()
}

fn framework_shadow_price_summary(
    demand_constraint: &DemandConstraint<f64, Meter>,
    demand_entries: &[Bpp3dDemandEntry],
    model: &MetaModel<f64>,
    dual_solution: &[f64],
) -> HashMap<String, f64> {
    let mut shadow_price_map = BasicShadowPriceMap::<DemandShadowPriceKey>::new();
    if extract_shadow_price::<
        DemandShadowPriceKey,
        MetaModel<f64>,
        BasicShadowPriceMap<DemandShadowPriceKey>,
        DemandConstraint<f64, Meter>,
    >(
        &mut shadow_price_map,
        std::slice::from_ref(demand_constraint),
        model,
        dual_solution,
    )
    .is_err()
    {
        return demand_entries
            .iter()
            .enumerate()
            .map(|(index, entry)| {
                (
                    demand_key_name(entry),
                    dual_solution.get(index).copied().unwrap_or(0.0),
                )
            })
            .collect();
    }
    demand_entries
        .iter()
        .map(|entry| {
            let key = DemandConstraint::<f64, Meter>::shadow_price_key(entry);
            let price_key = ShadowPriceKey::named::<DemandShadowPriceKey>(format!("{:?}", key));
            (
                demand_key_name(entry),
                shadow_price_map
                    .get(&price_key)
                    .map(|price| price.price)
                    .unwrap_or(0.0),
            )
        })
        .collect()
}

fn rmp_column_upper_bounds(
    layers: &[BinLayer<f64, Meter>],
    demand_entries: &[Bpp3dDemandEntry],
) -> Vec<Option<f64>> {
    layers
        .iter()
        .map(|layer| {
            let mut upper_bound: Option<f64> = None;
            for entry in demand_entries {
                let coefficient = layer.demand_coverage_coefficient(entry.mode, &entry.key);
                if coefficient > 0.0 {
                    let candidate = entry.demand / coefficient;
                    upper_bound = Some(match upper_bound {
                        Some(current) => current.min(candidate),
                        None => candidate,
                    });
                }
            }
            upper_bound.map(|bound| bound.max(0.0))
        })
        .collect()
}

fn rmp_layer_volume_terms(
    assignment: &ImpreciseAssignment<f64, Meter>,
) -> Vec<(usize, f64)> {
    assignment
        .layers
        .iter()
        .enumerate()
        .filter_map(|(layer_idx, layer)| {
            let model_idx = assignment.x.as_ref()
                .and_then(|x| x.model_index(&layer_idx))?;
            let volume = layer
                .bin
                .as_ref()
                .map(|bin| bin.width.value * bin.height.value * layer.depth.value)
                .unwrap_or(layer.depth.value);
            Some((model_idx, volume))
        })
        .collect()
}

fn final_assignment_indices_by_bin(
    assignment: &PreciseAssignment<f64, Meter>,
) -> Vec<Vec<(usize, usize)>> {
    assignment.bins
        .iter()
        .enumerate()
        .map(|(bin_idx, _)| {
            assignment.layers
                .iter()
                .enumerate()
                .filter_map(|(layer_idx, _)| {
                    let model_idx = assignment.x.as_ref()
                        .and_then(|x| x.model_index(&bin_idx, &layer_idx))?;
                    Some((layer_idx, model_idx))
                })
                .collect()
        })
        .collect()
}

fn final_bin_marker_indices(
    assignment: &PreciseAssignment<f64, Meter>,
) -> Vec<usize> {
    (0..assignment.bins.len())
        .filter_map(|bin_idx| {
            assignment.v.as_ref()
                .and_then(|v| v.model_index(&bin_idx))
        })
        .collect()
}

fn final_layer_weights(
    layers: &[BinLayer<f64, Meter>],
    items: &[ActualItem<f64, Meter>],
) -> Vec<f64> {
    let item_weights = items
        .iter()
        .map(|item| (item.id.as_str(), item.weight.value))
        .collect::<HashMap<_, _>>();
    layers
        .iter()
        .map(|layer| {
            layer
                .demand_coverage
                .iter()
                .filter_map(|coverage| match (&coverage.mode, &coverage.key) {
                    (
                        Bpp3dDemandMode::Item | Bpp3dDemandMode::ItemAmount,
                        Bpp3dDemandKey::Item { id },
                    ) => item_weights
                        .get(id.as_str())
                        .map(|weight| coverage.coefficient * *weight),
                    (
                        Bpp3dDemandMode::ItemWeight,
                        Bpp3dDemandKey::Item { .. },
                    ) => Some(coverage.coefficient),
                    _ => None,
                })
                .sum()
        })
        .collect()
}

fn final_layer_volumes(layers: &[BinLayer<f64, Meter>]) -> Vec<f64> {
    layers
        .iter()
        .map(|layer| {
            layer
                .bin
                .as_ref()
                .map(|bin| bin.width.value * bin.height.value * layer.depth.value)
                .unwrap_or(layer.depth.value)
        })
        .collect()
}

fn layer_depths(layers: &[BinLayer<f64, Meter>]) -> Vec<f64> {
    layers
        .iter()
        .map(|layer| layer.depth.value)
        .collect()
}

fn item_shadow_price_key(item_id: impl AsRef<str>) -> String {
    format!("item:{}", item_id.as_ref())
}

fn application_state_from_algorithm_with_continuous_radius(
    algorithm: &ColumnGenerationAlgorithm<f64, Meter>,
    items: Vec<ActualItem<f64, Meter>>,
    bins: Vec<BinType<f64, Meter>>,
    initial_layers: Vec<BinLayer<f64, Meter>>,
    layer_block_traces: HashMap<usize, Vec<LayerBlockTrace<f64, Meter>>>,
    layer_placement_traces: HashMap<usize, Vec<LayerPlacementTrace<f64, Meter>>>,
    continuous_radius_component: Option<ContinuousRadiusModelComponent>,
) -> ColumnGenerationApplicationState {
    ColumnGenerationApplicationState {
        items,
        bins,
        initial_layers,
        layers: algorithm.active_layers(),
        iteration: algorithm.state.iteration,
        layer_block_traces,
        layer_placement_traces,
        continuous_radius_component,
        additional_shadow_prices: algorithm.state.additional_shadow_prices.clone(),
        info: HashMap::new(),
    }
}

fn default_item_coverage(items: &[ActualItem<f64, Meter>]) -> Vec<Bpp3dLayerDemandCoverage> {
    items
        .iter()
        .map(|item| {
            Bpp3dLayerDemandCoverage::new(
                Bpp3dDemandMode::Item,
                Bpp3dDemandKey::Item { id: item.id.clone() },
                1.0,
            )
        })
        .collect()
}

fn ensure_layer_demand_coverage(
    layers: Vec<BinLayer<f64, Meter>>,
    items: &[ActualItem<f64, Meter>],
) -> Vec<BinLayer<f64, Meter>> {
    let fallback = default_item_coverage(items);
    layers
        .into_iter()
        .map(|layer| {
            if layer.demand_coverage.is_empty() {
                layer.with_demand_coverage(fallback.clone())
            } else {
                layer
            }
        })
        .collect()
}

/// 单层的块与放置 trace；两者同时存在时放置 trace 必须与块摘要一致。
/// Block and placement traces for one layer; when both exist, placement summaries must match the blocks.
pub type LayerTracePair = (
    Vec<LayerBlockTrace<f64, Meter>>,
    Vec<LayerPlacementTrace<f64, Meter>>,
);

/// 全流程入口只按初始层覆盖单件需求，无法承载 CSV 声明的分组需求；明确拒绝而不是静默降级。
/// Full-flow entries only cover single-unit demand from initial layers, so declared grouped CSV demand is rejected instead of silently downgraded.
#[cfg(feature = "serde")]
fn reject_grouped_csv_demand(
    request: &CsvMaterializedApplicationRequest,
    entry: &str,
) -> Result<(), Vec<String>> {
    let amounts = ordered_csv_item_amounts(request)?;
    if amounts.iter().any(|amount| *amount != 1) {
        return Err(vec![format!(
            "{} covers exactly one unit per item from initial layers; grouped CSV demand requires the traced one-generation-round entry",
            entry
        )]);
    }
    Ok(())
}

/// 将 CSV 声明的货物需求整理为物品目录顺序并复验；不猜默认值。
/// Order declared CSV demand into item-catalog order and revalidate it without guessing defaults.
#[cfg(feature = "serde")]
fn ordered_csv_item_amounts(request: &CsvMaterializedApplicationRequest) -> Result<Vec<u64>, Vec<String>> {
    let mut amounts = HashMap::new();
    for (id, amount) in &request.item_amounts {
        if amounts.insert(id.as_str(), *amount).is_some() {
            return Err(vec!["duplicate CSV item demand".into()]);
        }
    }
    let mut ordered_amounts = Vec::with_capacity(request.items.len());
    for item in &request.items {
        ordered_amounts.push(amounts.remove(item.id.as_str()).ok_or_else(|| vec!["missing CSV item demand".into()])?);
    }
    if !amounts.is_empty() { return Err(vec!["unknown CSV item demand".into()]); }
    validate_item_amounts(&request.items, &ordered_amounts).map_err(|error| vec![error])?;
    if ordered_amounts.iter().try_fold(0_u64, |total, amount| total.checked_add(*amount))
        .is_none_or(|total| total > 100_000)
    {
        return Err(vec!["CSV demand exceeds the maximum strict replay placement limit".into()]);
    }
    Ok(ordered_amounts)
}

#[cfg(feature = "serde")]
fn prepare_csv_traced_initial_layers(
    request: &mut CsvMaterializedApplicationRequest,
    traces: &[LayerTracePair],
) -> Result<Vec<u64>, Vec<String>> {
    if request.initial_layers.len() != traces.len() {
        return Err(vec!["initial trace count must match CSV initial layers".into()]);
    }
    let ordered_amounts = ordered_csv_item_amounts(request)?;
    for (layer, (blocks, placements)) in request.initial_layers.iter_mut().zip(traces) {
        let mut counts = BTreeMap::<usize, u64>::new();
        let mut add = |index: usize, id: &crate::domain::item::ItemId, amount: u64| -> Result<(), String> {
            let item = request.items.get(index).ok_or("initial trace references unknown item index")?;
            if &item.id != id || amount == 0 { return Err("invalid initial trace item identity or amount".into()); }
            let count = counts.entry(index).or_default();
            *count = count.checked_add(amount).ok_or("initial trace amount overflows")?;
            if *count > 9_007_199_254_740_992 { return Err("initial trace amount cannot be represented exactly".into()); }
            Ok(())
        };
        if blocks.is_empty() {
            for trace in placements { add(trace.item_index, &trace.item_id, trace.amount).map_err(|error| vec![error])?; }
        } else {
            for trace in blocks { add(trace.item_index, &trace.item_id, trace.item_count).map_err(|error| vec![error])?; }
        }
        if layer.demand_coverage.is_empty() && !counts.is_empty() {
            layer.demand_coverage = counts.into_iter().map(|(index, count)| Bpp3dLayerDemandCoverage::new(
                Bpp3dDemandMode::Item, Bpp3dDemandKey::Item { id: request.items[index].id.clone() }, count as f64,
            )).collect();
        }
    }
    Ok(ordered_amounts)
}

fn split_traced_initial_layers(
    results: Vec<LayerGenerationResult<f64, Meter>>,
) -> (Vec<BinLayer<f64, Meter>>, Vec<LayerTracePair>) {
    results.into_iter()
        .map(|result| (result.layer, (result.block_traces, result.placement_traces)))
        .unzip()
}

fn add_initial_layers_with_traces(
    algorithm: &mut ColumnGenerationAlgorithm<f64, Meter>,
    layers: &[BinLayer<f64, Meter>],
    traces: Vec<LayerTracePair>,
) -> (
    HashMap<usize, Vec<LayerBlockTrace<f64, Meter>>>,
    HashMap<usize, Vec<LayerPlacementTrace<f64, Meter>>>,
) {
    let mut traces_by_key = HashMap::new();
    for (layer, trace) in layers.iter().zip(traces) {
        traces_by_key.entry(layer.identity_key()).or_insert(trace);
    }
    let added = algorithm.add_initial_layers(layers.to_vec());
    let mut block_traces = HashMap::new();
    let mut placement_traces = HashMap::new();
    for (index, layer) in added.iter().enumerate() {
        if let Some((blocks, placements)) = traces_by_key.remove(&layer.identity_key()) {
            if !blocks.is_empty() { block_traces.insert(index, blocks); }
            if !placements.is_empty() { placement_traces.insert(index, placements); }
        }
    }
    (block_traces, placement_traces)
}

fn layer_generation_demand_entries(
    items: &[ActualItem<f64, Meter>],
    shadow_price_summary: &HashMap<String, f64>,
    amounts: Option<&[u64]>,
) -> Vec<LayerGenerationDemandEntry> {
    items
        .iter()
        .enumerate()
        .map(|(index, item)| {
            let demand = amounts.map(|amounts| amounts[index] as f64).unwrap_or(1.0);
            LayerGenerationDemandEntry {
                mode: Bpp3dDemandMode::Item,
                key: Bpp3dDemandKey::Item { id: item.id.clone() },
                demand,
                satisfied: if shadow_price_summary.contains_key(&item_shadow_price_key(&item.id)) {
                    0.0
                } else {
                    demand
                },
            }
        })
        .collect()
}

fn layer_generation_shadow_prices(
    shadow_price_summary: &HashMap<String, f64>,
) -> HashMap<DemandShadowPriceKey, f64> {
    shadow_price_summary
        .iter()
        .filter_map(|(key, value)| {
            key.strip_prefix("item:").map(|id| {
                (
                    DemandShadowPriceKey {
                        mode: Bpp3dDemandMode::Item,
                        key: Bpp3dDemandKey::Item { id: id.into() },
                    },
                    *value,
                )
            })
        })
        .collect()
}

