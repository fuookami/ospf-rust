/// 精确分配的物理回放输入；索引属于同一原候选集，不能由去重后的层集合替代。
/// Physical replay input for precise assignments; indices belong to one original candidate set, never a deduplicated selected-layer set.
pub struct LayerAssignmentTraceReplayRequest<'a, V, U>
where
    V: Debug + Clone + Send + Sync + num_traits::Float + Field + num_traits::FloatConst + Into<f64>,
    U: CTUnit + Default + Clone + Debug + Send + Sync,
{
    /// 已完整提取的原 x/v 分配，回放会再次检查索引和重数。 / Completely extracted original x/v selection; replay rechecks indices and multiplicity.
    pub selection: &'a crate::domain::layer_assignment::PreciseAssignmentSelection,
    /// 原列池。 / Original layer pool.
    pub layers: &'a [BinLayer<V, U>],
    /// 原候选箱，重复箱型仍为不同箱。 / Original candidate bins; repeated types remain distinct bins.
    pub bins: &'a [BinType<V, U>],
    /// 原物品目录。 / Original item catalog.
    pub items: &'a [ActualItem<V, U>],
    /// 按原层索引的精确块 trace。 / Exact block traces keyed by original layer index.
    pub block_traces: &'a HashMap<usize, Vec<LayerBlockTrace<V, U>>>,
    /// 按原层索引的放置 trace；块同时存在时只能作为匹配的摘要。 / Placement traces keyed by original layer; when blocks exist these must be matching summaries.
    pub placement_traces: &'a HashMap<usize, Vec<LayerPlacementTrace<V, U>>>,
    /// 全部箱展开物品上限，1..=100000；几何检查仍为合作式、可能为二次复杂度。 / Expanded-item limit across bins, 1..=100000; geometry checks remain cooperative and potentially quadratic.
    pub max_placements: usize,
    /// 基础几何检查之后的附加业务门禁，不能放宽基础门禁。 / Additional business gate after base geometry validation, never weakening that gate.
    pub extra_geometry: Option<&'a dyn PackingGeometryContract<V, U>>,
}

impl LayerTraceReplayAdapter {
    /// 严格保留箱／层分配，按原层序沿 Z 累加深度；缺 trace、截断或任一几何失败整体返回错误。
    /// Strictly retain bin/layer assignments, stacking depth along Z in original layer order; missing traces, truncation or any geometry failure reject the whole result.
    /// 不调用 coverage fallback，不证明需求、载重或源问题全局最优；这些由领域模型和后续复验负责。
    /// Never use coverage fallback or claim demand, load or global source optimality; those belong to domain modelling and subsequent validation.
    pub fn replay_assignments<V, U>(&self, request: LayerAssignmentTraceReplayRequest<'_, V, U>) -> Result<Vec<PackedBin<V, U>>, Vec<String>>
    where
        V: Debug + Clone + Send + Sync + num_traits::Float + Field + num_traits::FloatConst + Into<f64>,
        U: CTUnit + Default + Clone + Debug + Send + Sync,
    {
        strict_replay_assignments(self, request).map_err(|error| vec![error])
    }
}

fn strict_replay_assignments<V, U>(
    adapter: &LayerTraceReplayAdapter,
    request: LayerAssignmentTraceReplayRequest<'_, V, U>,
) -> Result<Vec<PackedBin<V, U>>, String>
where
    V: Debug + Clone + Send + Sync + num_traits::Float + Field + num_traits::FloatConst + Into<f64>,
    U: CTUnit + Default + Clone + Debug + Send + Sync,
{
    use std::collections::BTreeSet;
    let fail = |message: &str| Err(message.to_string());
    if !(1..=100_000).contains(&request.max_placements) { return fail("invalid strict replay placement limit"); }
    let active: BTreeSet<_> = request.selection.activated_bins.iter().copied().collect();
    if active.len() != request.selection.activated_bins.len() || active.iter().any(|index| *index >= request.bins.len()) {
        return fail("duplicate or unknown activated bin");
    }
    let assignments: BTreeSet<_> = request.selection.assignments.iter().copied().collect();
    if assignments.len() != request.selection.assignments.len() || assignments.iter().any(|assignment| {
        !active.contains(&assignment.bin_index) || assignment.layer_index >= request.layers.len()
    }) { return fail("duplicate, unknown or inactive bin/layer assignment"); }
    let mut output = Vec::new();
    let mut total = 0_usize;
    for bin_index in active {
        let bin = &request.bins[bin_index];
        let dimensions = [bin.width.value.into(), bin.height.value.into(), bin.depth.value.into()];
        if dimensions.iter().any(|value: &f64| !value.is_finite() || *value <= 0.0) { return fail("invalid strict replay bin dimensions"); }
        let mut packed_items = Vec::new();
        let mut offset = 0.0;
        for assignment in assignments.iter().filter(|assignment| assignment.bin_index == bin_index) {
            let index = assignment.layer_index;
            let layer = &request.layers[index];
            let depth: f64 = layer.depth.value.into();
            if !depth.is_finite() || depth <= 0.0 || !((offset + depth) <= dimensions[2] + PACKING_GEOMETRY_OVERLAP_TOLERANCE) {
                return fail("assigned layers exceed bin depth or have invalid depth");
            }
            if let Some(source) = &layer.bin {
                if source.type_code != bin.type_code || [source.width.value.into(), source.height.value.into(), source.depth.value.into()]
                    .iter().zip(dimensions).any(|(lhs, rhs)| !lhs.is_finite() || (*lhs - rhs).abs() > PACKING_GEOMETRY_OVERLAP_TOLERANCE)
                { return fail("assigned layer belongs to a different bin type or dimensions"); }
            }
            let blocks = request.block_traces.get(&index).map(Vec::as_slice).unwrap_or(&[]);
            let traces = request.placement_traces.get(&index).map(Vec::as_slice).unwrap_or(&[]);
            if blocks.is_empty() && traces.is_empty() { return fail("selected assignment has no physical trace; coverage fallback forbidden"); }
            if !blocks.is_empty() && !traces.is_empty() {
                if blocks.len() != traces.len() || blocks.iter().zip(traces).any(|(block, trace)| {
                    block.item_index != trace.item_index || block.item_id != trace.item_id || block.orientation != trace.orientation
                        || block.item_count != trace.amount || [block.origin.x.value.into(), block.origin.y.value.into(), block.origin.z.value.into()]
                            .iter().zip([trace.position.x.value.into(), trace.position.y.value.into(), trace.position.z.value.into()])
                            .any(|(lhs, rhs)| !lhs.is_finite() || !rhs.is_finite() || (*lhs - rhs).abs() > PACKING_GEOMETRY_OVERLAP_TOLERANCE)
                }) { return fail("placement summaries disagree with authoritative block traces"); }
            }
            let trace_count = if blocks.is_empty() { traces.len() } else { blocks.len() };
            for trace_index in 0..trace_count {
                let (item_index, item_id, orientation, origin, amount, grid, declared_size) = if blocks.is_empty() {
                    let trace = &traces[trace_index];
                    (trace.item_index, &trace.item_id, trace.orientation, &trace.position, trace.amount, None, None)
                } else {
                    let block = &blocks[trace_index];
                    (block.item_index, &block.item_id, block.orientation, &block.origin, block.item_count,
                        Some([block.nx, block.ny, block.nz]), Some(&block.size))
                };
                let item = request.items.get(item_index).ok_or("physical trace references unknown item index")?;
                if &item.id != item_id { return fail("physical trace item identity mismatch"); }
                if !item.enabled_orientations.is_empty() && !item.enabled_orientations.contains(&orientation) { return fail("physical trace uses disabled orientation"); }
                let count = usize::try_from(amount).map_err(|_| "physical trace amount exceeds platform limit")?;
                total = total.checked_add(count).ok_or("physical trace amount overflows")?;
                if count == 0 || total > request.max_placements { return fail("physical trace amount is zero or exceeds placement limit"); }
                let size = oriented_item_size(item, orientation);
                let size: [f64; 3] = [size.width.value.into(), size.height.value.into(), size.depth.value.into()];
                let origin: [f64; 3] = [origin.x.value.into(), origin.y.value.into(), origin.z.value.into()];
                if size.iter().any(|value| !value.is_finite() || *value <= 0.0) || origin.iter().any(|value| !value.is_finite() || *value < 0.0) {
                    return fail("physical trace has invalid size or origin");
                }
                let local_bounds = [dimensions[0], dimensions[1], depth];
                let grid = match grid {
                    Some(grid) => {
                        let capacity = grid[0].checked_mul(grid[1]).and_then(|value| value.checked_mul(grid[2])).ok_or("block grid capacity overflows")?;
                        if grid.contains(&0) || amount != capacity { return fail("block grid does not specify every item position"); }
                        let declared = declared_size.ok_or("block size missing")?;
                        for (axis, value) in [declared.width.value.into(), declared.height.value.into(), declared.depth.value.into()].into_iter().enumerate() {
                            let expected = size[axis] * grid[axis] as f64;
                            if !value.is_finite() || !expected.is_finite() || (value - expected).abs() > PACKING_GEOMETRY_OVERLAP_TOLERANCE {
                                return fail("block dimensions disagree with oriented item grid");
                            }
                        }
                        grid
                    }
                    None => {
                        if amount != 1 { return fail("placement summary lacks explicit positions for multiple items"); }
                        [1, 1, 1]
                    }
                };
                for position in 0..amount {
                    let coordinates = [position % grid[0], (position / grid[0]) % grid[1], position / grid[0] / grid[1]];
                    let mut point = [0.0; 3];
                    for axis in 0..3 {
                        point[axis] = origin[axis] + size[axis] * coordinates[axis] as f64;
                        if !point[axis].is_finite() || point[axis] + size[axis] > local_bounds[axis] + PACKING_GEOMETRY_OVERLAP_TOLERANCE {
                            return fail("physical trace exceeds assigned layer bounds");
                        }
                    }
                    point[2] += offset;
                    let placement = KnownCoordinatePlacement { item_index, item: item.clone(), orientation, position: MetricPoint3 {
                        x: strict_replay_quantity(point[0])?, y: strict_replay_quantity(point[1])?, z: strict_replay_quantity(point[2])?,
                    } };
                    packed_items.push(adapter.placement_adapter.to_packed_item(placement));
                }
            }
            offset += depth;
        }
        let packed = PackedBin { name: format!("assigned-bin-{}", bin_index), bin_type: bin.clone(), batch_no: None, items: packed_items };
        PackingGeometryGuard::validate(&packed).map_err(|errors| errors.join("; "))?;
        if let Some(extra) = request.extra_geometry { extra.validate(&packed).map_err(|errors| errors.join("; "))?; }
        output.push(packed);
    }
    Ok(output)
}

fn strict_replay_quantity<V, U>(value: f64) -> Result<Quantity<V, U>, String>
where
    V: num_traits::Float + Into<f64>,
    U: CTUnit + Default,
{
    let converted = V::from(value).ok_or("physical trace coordinate cannot be represented")?;
    let roundtrip: f64 = converted.into();
    if !converted.is_finite() || !value.is_finite() || (roundtrip - value).abs() > PACKING_GEOMETRY_OVERLAP_TOLERANCE {
        return Err("physical trace coordinate loses precision".into());
    }
    Ok(Quantity::new_ct(converted))
}
