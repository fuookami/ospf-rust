//! 动态模型生命周期绑定：列状态、解注入、warm start 与 flush 的可移植语义。
//! Dynamic model lifecycle binding: portable semantics for column state, solution injection, warm start and flush.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use ospf_rust_framework::model::dynamic_lifecycle::{ColumnRange, ColumnState, DynamicModelLifecycle, DynamicModelState};

use crate::{BridgeError, Result};

/// 列的生命周期状态。 / The lifecycle state of one column.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum LinearColumnState {
    /// 可选且活跃。 / Selectable and active.
    Active,
    /// 隐藏但未固定。 / Hidden without being fixed.
    Hidden,
    /// 已固定到指定值。 / Fixed to a specific value.
    Fixed,
    /// 已移除。 / Removed.
    Removed,
}

impl LinearColumnState {
    fn to_domain(self) -> ColumnState {
        match self {
            Self::Active => ColumnState::Active,
            Self::Hidden => ColumnState::Hidden,
            Self::Fixed => ColumnState::Fixed,
            Self::Removed => ColumnState::Removed,
        }
    }

    fn from_domain(state: ColumnState) -> Self {
        match state {
            ColumnState::Active => Self::Active,
            ColumnState::Hidden => Self::Hidden,
            ColumnState::Fixed => Self::Fixed,
            ColumnState::Removed => Self::Removed,
        }
    }
}

/// 一次生命周期操作；按顺序应用，后面的操作覆盖前面的。
/// One lifecycle operation; they apply in order and later operations override earlier ones.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LinearColumnOperation {
    /// 操作类型。 / The operation kind.
    pub kind: LinearColumnOperationKind,
    /// 目标列索引；按稳定升序应用。 / Target column indices, applied in stable ascending order.
    pub columns: Vec<u32>,
}

/// 生命周期操作类型。 / Lifecycle operation kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum LinearColumnOperationKind {
    /// 隐藏列。 / Hide columns.
    Hide,
    /// 固定列（用当前解的值）。 / Fix columns to their current solution values.
    Fix,
    /// 移除列。 / Remove columns.
    Remove,
    /// 标记为 warm start。 / Mark as warm start.
    WarmStart,
    /// 清空瞬时状态。 / Flush transient state.
    FlushTransient,
}

/// 生命周期请求：列数 + 顺序操作 + 可选注入解 + 可选 warm-start 阈值。
/// Lifecycle request: column count, ordered operations, an optional injected solution and an optional warm-start
/// threshold.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LinearLifecycleRequest {
    /// 模型的列数；所有列索引必须落在此范围内。 / The model's column count; every index must fall inside it.
    pub column_count: u32,
    /// 顺序应用的列操作。 / Column operations applied in order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub operations: Vec<LinearColumnOperation>,
    /// 注入解；长度必须等于列数。 / Injected solution; its length must equal the column count.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub solution: Option<Vec<f64>>,
    /// warm-start 阈值；给出时按当前解重建 warm start。 / Warm-start threshold; when present the warm start is rebuilt from the current solution.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub warm_start_threshold: Option<f64>,
}

/// 生命周期结果。 / Lifecycle result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LinearLifecycleReport {
    /// 每列的最终状态，按列索引升序。 / Final state per column, ascending by index.
    pub states: Vec<LinearColumnStateEntry>,
    /// warm-start 列，按升序。 / Warm-start columns, ascending.
    pub warm_start_columns: Vec<u32>,
    /// 当前注入解；未注入时为空。 / The current injected solution; absent when none was injected.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub solution: Option<Vec<f64>>,
    /// flush 次数。 / Flush count.
    pub flush_count: u32,
    /// 仍可选的列，按升序。 / Columns that remain selectable, ascending.
    pub selectable_columns: Vec<u32>,
}

/// 一列的最终状态。 / The final state of one column.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LinearColumnStateEntry {
    /// 列索引。 / Column index.
    pub column: u32,
    /// 该列的最终状态。 / The column's final state.
    pub state: LinearColumnState,
}

/// 应用一次生命周期请求，返回每列的最终状态。 / Apply one lifecycle request and return each column's final state.
pub fn apply_lifecycle(request: &LinearLifecycleRequest) -> Result<LinearLifecycleReport> {
    if request.column_count == 0 {
        return Err(BridgeError::new("InvalidInput", "lifecycle requires at least one column"));
    }
    let columns = usize::try_from(request.column_count)
        .map_err(|_| BridgeError::new("InvalidInput", "column count overflow"))?;
    let mut lifecycle = DynamicModelLifecycle::from_state(DynamicModelState::new());
    if let Some(solution) = &request.solution {
        if solution.len() != columns {
            return Err(BridgeError::new("InvalidInput", "lifecycle solution length must equal the column count"));
        }
        if solution.iter().any(|value| !value.is_finite()) {
            return Err(BridgeError::new("InvalidInput", "lifecycle solution values must be finite"));
        }
        lifecycle.set_solution(solution.clone());
    }
    let mut seen_flush = false;
    for operation in &request.operations {
        // 列索引越界是调用方的错误，不能静默忽略。 / An out-of-range column index is a caller error and is never ignored.
        let mut indices = Vec::with_capacity(operation.columns.len());
        for column in &operation.columns {
            let index = usize::try_from(*column)
                .map_err(|_| BridgeError::new("InvalidInput", "column index overflow"))?;
            if index >= columns {
                return Err(BridgeError::new("InvalidInput", "column index is outside the declared column count"));
            }
            indices.push(index);
        }
        match operation.kind {
            LinearColumnOperationKind::Hide => lifecycle.state_mut().hide_columns(indices),
            LinearColumnOperationKind::Fix => lifecycle.state_mut().fix_columns(indices),
            LinearColumnOperationKind::Remove => lifecycle.state_mut().remove_columns(indices),
            LinearColumnOperationKind::WarmStart => lifecycle.state_mut().set_warm_start(indices),
            LinearColumnOperationKind::FlushTransient => {
                lifecycle.state_mut().flush_transient();
                seen_flush = true;
            }
        }
    }
    // warm-start 阈值走领域实现：它按当前解重建，而不是复用调用方给的列。
    // The warm-start threshold uses the domain implementation: it rebuilds from the current solution instead of reusing
    // the caller's columns.
    if let Some(threshold) = request.warm_start_threshold {
        if !threshold.is_finite() {
            return Err(BridgeError::new("InvalidInput", "warm-start threshold must be finite"));
        }
        lifecycle.set_warm_start_from_solution(threshold);
    }
    let mut states = Vec::with_capacity(columns);
    for column in 0..columns {
        states.push(LinearColumnStateEntry { column: u32::try_from(column).unwrap_or(u32::MAX),
            state: LinearColumnState::from_domain(lifecycle.state().column_state(column)) });
    }
    let selectable = (0..columns).filter(|column| lifecycle.state().is_selectable(*column))
        .map(|column| u32::try_from(column).unwrap_or(u32::MAX)).collect();
    Ok(LinearLifecycleReport {
        states,
        warm_start_columns: lifecycle.warm_start_columns().into_iter()
            .map(|column| u32::try_from(column).unwrap_or(u32::MAX)).collect(),
        solution: lifecycle.solution().map(|solution| solution.to_vec()),
        flush_count: u32::from(seen_flush),
        selectable_columns: selectable,
    })
}

/// 列范围覆盖的便捷换算：把 `ColumnRange` 转成 `VariableRange`。
/// Convenience conversion for column-range overrides: turn a `ColumnRange` into a `VariableRange`.
pub fn column_range_override(lower: f64, upper: f64) -> Result<HashMap<String, (f64, f64)>> {
    if !lower.is_finite() || !upper.is_finite() || lower > upper {
        return Err(BridgeError::new("InvalidInput", "column range must be finite with lower <= upper"));
    }
    let range = ColumnRange::new(lower, upper).to_variable_range();
    let mut map = HashMap::new();
    map.insert("lower".to_owned(), (range.lower_bound.unwrap_or(f64::NEG_INFINITY), range.upper_bound.unwrap_or(f64::INFINITY)));
    Ok(map)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> LinearLifecycleRequest {
        LinearLifecycleRequest { column_count: 3, operations: Vec::new(), solution: None, warm_start_threshold: None }
    }

    #[test]
    fn operations_apply_in_order_and_later_ones_win() {
        // 先隐藏 1，再固定 1：最终是固定。 / Hide 1 then fix 1: the final state is fixed.
        let report = apply_lifecycle(&LinearLifecycleRequest { operations: vec![
            LinearColumnOperation { kind: LinearColumnOperationKind::Hide, columns: vec![1] },
            LinearColumnOperation { kind: LinearColumnOperationKind::Fix, columns: vec![1] }], ..request() }).unwrap();
        assert_eq!(report.states[0].state, LinearColumnState::Active);
        assert_eq!(report.states[1].state, LinearColumnState::Fixed);
        assert_eq!(report.states[2].state, LinearColumnState::Active);
        // 固定列仍然可选：`is_selectable` 只排除隐藏与移除，固定只是收窄范围。
        // A fixed column stays selectable: `is_selectable` only excludes hidden and removed columns, and fixing merely
        // narrows the range.
        assert_eq!(report.selectable_columns, vec![0, 1, 2]);
        assert_eq!(report.flush_count, 0);
        // 移除后不可选。 / A removed column is not selectable.
        let report = apply_lifecycle(&LinearLifecycleRequest { operations: vec![
            LinearColumnOperation { kind: LinearColumnOperationKind::Remove, columns: vec![0] }], ..request() }).unwrap();
        assert_eq!(report.states[0].state, LinearColumnState::Removed);
        assert!(!report.selectable_columns.contains(&0));
    }

    #[test]
    fn warm_start_threshold_rebuilds_from_the_current_solution() {
        let report = apply_lifecycle(&LinearLifecycleRequest { column_count: 3,
            solution: Some(vec![0.0, 1.0, 2.0]), warm_start_threshold: Some(1.0), ..request() }).unwrap();
        // 只有 >= 阈值且可选的列进入 warm start。 / Only selectable columns at or above the threshold enter the warm start.
        assert_eq!(report.warm_start_columns, vec![1, 2]);
        assert_eq!(report.solution.as_deref(), Some(&[0.0, 1.0, 2.0][..]));
        // 隐藏列即便超过阈值也不进入 warm start。 / A hidden column stays out of the warm start even above the threshold.
        let report = apply_lifecycle(&LinearLifecycleRequest { column_count: 3, solution: Some(vec![0.0, 1.0, 2.0]),
            warm_start_threshold: Some(1.0), operations: vec![LinearColumnOperation {
                kind: LinearColumnOperationKind::Hide, columns: vec![2] }], ..request() }).unwrap();
        assert_eq!(report.warm_start_columns, vec![1]);
    }

    #[test]
    fn bad_lifecycle_requests_are_refused() {
        let broken: Vec<(&str, LinearLifecycleRequest)> = vec![
            ("no columns", LinearLifecycleRequest { column_count: 0, ..request() }),
            ("solution length mismatch", LinearLifecycleRequest { solution: Some(vec![1.0, 2.0]), ..request() }),
            ("non-finite solution", LinearLifecycleRequest { solution: Some(vec![1.0, f64::INFINITY, 3.0]), ..request() }),
            ("index out of range", LinearLifecycleRequest { operations: vec![LinearColumnOperation {
                kind: LinearColumnOperationKind::Hide, columns: vec![3] }], ..request() }),
            ("non-finite threshold", LinearLifecycleRequest { warm_start_threshold: Some(f64::NAN), ..request() }),
        ];
        for (label, case) in broken {
            assert!(apply_lifecycle(&case).is_err(), "{label} must be refused");
        }
        assert!(column_range_override(3.0, 1.0).is_err());
        assert!(column_range_override(f64::NAN, 1.0).is_err());
        assert!(column_range_override(0.0, 4.0).is_ok());
    }
}
