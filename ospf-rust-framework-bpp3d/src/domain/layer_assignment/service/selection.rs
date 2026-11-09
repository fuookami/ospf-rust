/// 原 final 模型的箱／层赋值，不折叠跨箱重数。 / Bin/layer assignment from the original final model, without collapsing cross-bin multiplicity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LayerAssignment {
    /// 原候选箱索引，不是箱型去重索引。 / Original candidate-bin index, not a deduplicated type index.
    pub bin_index: usize,
    /// 原列池层索引。 / Original column-pool layer index.
    pub layer_index: usize,
}

/// 精确赋值的完整二值提取；本身不证明几何或需求可行性。 / Complete binary extraction of a precise assignment; not a geometry or demand-feasibility proof.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreciseAssignmentSelection {
    /// 原模型中 v=1 的箱，保留空启用箱。 / Bins with v=1 in the original model, retaining activated empty bins.
    pub activated_bins: Vec<usize>,
    /// 每个 x=1 的原箱／层对，按箱／层排序。 / Every original bin/layer pair with x=1, sorted by bin/layer.
    pub assignments: Vec<LayerAssignment>,
}

impl<V, U> PreciseAssignment<V, U>
where
    V: Debug + Clone + Send + Sync,
    U: ospf_rust_quantities::unit::concept::UnitTrait + Debug + Clone + Send + Sync,
{
    /// 完整读取已注册的 x/v，拒绝缺值、非法二值及未启用箱赋值，不以阈值掩盖小数。
    /// Completely read registered x/v, rejecting missing values, invalid binaries and assignments to inactive bins rather than hiding fractions with a threshold.
    pub fn extract_selection(&self, solution: &[f64], tolerance: f64) -> Result<PreciseAssignmentSelection, String> {
        if !tolerance.is_finite() || !(0.0..0.5).contains(&tolerance) {
            return Err("invalid precise-assignment tolerance".into());
        }
        let x = self.x.as_ref().ok_or("precise-assignment x is not registered")?;
        let v = self.v.as_ref().ok_or("precise-assignment v is not registered")?;
        let binary = |index: Option<usize>| -> Result<bool, String> {
            let index = index.ok_or("precise-assignment variable has no original model index")?;
            let value = *solution.get(index).ok_or("precise-assignment solution is incomplete")?;
            if value.is_finite() && value.abs() <= tolerance { Ok(false) }
            else if value.is_finite() && (value - 1.0).abs() <= tolerance { Ok(true) }
            else { Err(format!("invalid precise-assignment binary at original index {}", index)) }
        };
        let mut selection = PreciseAssignmentSelection { activated_bins: Vec::new(), assignments: Vec::new() };
        for bin_index in 0..self.bins.len() {
            let active = binary(v.model_index(&bin_index))?;
            if active { selection.activated_bins.push(bin_index); }
            for layer_index in 0..self.layers.len() {
                if binary(x.model_index(&bin_index, &layer_index))? {
                    if !active { return Err("precise assignment references an inactive bin".into()); }
                    selection.assignments.push(LayerAssignment { bin_index, layer_index });
                }
            }
        }
        Ok(selection)
    }
}
