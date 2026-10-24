//! 爱因斯坦求和 trait 定义
//! Einstein summation trait definitions
//!
//! 提供爱因斯坦求和的核心 trait 和错误类型。
//! Provides core traits and error types for Einstein summation.

use super::indices::{IndexList, find_common_indices};
use super::tensor_expr::TensorExpr;
use crate::{AbstractShape, DynShape, MultiArray};
use num_traits::Zero;
use std::ops::{Add, AddAssign, Mul};

// ============================================================================
// EinsumError - 爱因斯坦求和错误
// ============================================================================

/// 爱因斯坦求和错误类型 / Einstein summation error type
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EinsumError {
    /// 维度不匹配
    /// Dimension mismatch
    DimensionMismatch {
        /// 期望的维度
        /// Expected dimension
        expected: usize,
        /// 实际的维度
        /// Actual dimension
        actual: usize,
        /// 描述信息
        /// Description
        message: String,
    },

    /// 形状不兼容
    /// Incompatible shapes
    IncompatibleShapes {
        /// 第一个形状
        /// First shape
        shape1: Vec<usize>,
        /// 第二个形状
        /// Second shape
        shape2: Vec<usize>,
        /// 描述信息
        /// Description
        message: String,
    },

    /// 索引重复
    /// Duplicate indices
    DuplicateIndices {
        /// 重复的索引
        /// Duplicate index
        index: usize,
    },

    /// 不支持的运算
    /// Unsupported operation
    UnsupportedOperation {
        /// 描述信息
        /// Description
        message: String,
    },
}

impl std::fmt::Display for EinsumError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EinsumError::DimensionMismatch {
                expected,
                actual,
                message,
            } => {
                write!(
                    f,
                    "Dimension mismatch: expected {}, got {} / 维度不匹配：期望 {}，实际 {} ({})",
                    expected, actual, expected, actual, message
                )
            }
            EinsumError::IncompatibleShapes {
                shape1,
                shape2,
                message,
            } => {
                write!(
                    f,
                    "Incompatible shapes: {:?} vs {:?} / 形状不兼容：{:?} vs {:?} ({})",
                    shape1, shape2, shape1, shape2, message
                )
            }
            EinsumError::DuplicateIndices { index } => {
                write!(f, "Duplicate index: {} / 索引重复：{}", index, index)
            }
            EinsumError::UnsupportedOperation { message } => {
                write!(
                    f,
                    "Unsupported operation: {} / 不支持的运算：{}",
                    message, message
                )
            }
        }
    }
}

impl std::error::Error for EinsumError {}

// ============================================================================
// EinsteinSum trait - 爱因斯坦求和 trait
// ============================================================================

/// 爱因斯坦求和 trait / Einstein summation trait
///
/// 定义爱因斯坦求和的核心操作。
/// Defines core operations for Einstein summation.
///
/// # 类型参数 / Type Parameters
///
/// - `T`: 元素类型
/// - `S1`: 左操作数形状类型
/// - `Idx1`: 左操作数索引列表
/// - `S2`: 右操作数形状类型
/// - `Idx2`: 右操作数索引列表
/// - `OutIdx`: 输出索引列表
///
/// # 示例 / Example
///
/// ```
/// use ospf_rust_multiarray::{MultiArray, Shape, MultiArrayBuilder};
/// use ospf_rust_multiarray::einsum::{matmul, I, J, K};
///
/// // 矩阵乘法：A_ij * B_jk -> C_ik
/// // Matrix multiplication: A_ij * B_jk -> C_ik
/// let a: MultiArray<f64, Shape<2>> = MultiArrayBuilder::new_with(Shape::new([2, 3]), 1.0);
/// let b: MultiArray<f64, Shape<2>> = MultiArrayBuilder::new_with(Shape::new([3, 4]), 2.0);
/// let c = matmul(&a, &b);
/// assert!(c.is_ok());
/// ```
pub trait EinsteinSum<'a, T, S1, Idx1, S2, Idx2, OutIdx>
where
    S1: AbstractShape,
    S2: AbstractShape,
    Idx1: IndexList,
    Idx2: IndexList,
    OutIdx: IndexList,
{
    /// 爱因斯坦乘法（隐式求和）
    /// Einstein multiplication (implicit summation)
    ///
    /// 对公共索引进行求和，生成输出张量。
    /// Sums over common indices and produces output tensor.
    ///
    /// # 参数 / Arguments
    ///
    /// - `other`: 右操作数
    ///
    /// # 返回 / Returns
    ///
    /// 结果张量
    /// Result tensor
    fn einsum(
        &self,
        other: &TensorExpr<'a, T, S2, Idx2>,
    ) -> Result<MultiArray<T, DynShape>, EinsumError>
    where
        T: Clone + Zero + Add<Output = T> + Mul<Output = T> + AddAssign;
}

// ============================================================================
// 通用实现
// ============================================================================

impl<'a, T, S1, Idx1, S2, Idx2, OutIdx> EinsteinSum<'a, T, S1, Idx1, S2, Idx2, OutIdx>
    for TensorExpr<'a, T, S1, Idx1>
where
    S1: AbstractShape,
    S2: AbstractShape,
    Idx1: IndexList,
    Idx2: IndexList,
    OutIdx: IndexList,
{
    fn einsum(
        &self,
        other: &TensorExpr<'a, T, S2, Idx2>,
    ) -> Result<MultiArray<T, DynShape>, EinsumError>
    where
        T: Clone + Zero + Add<Output = T> + Mul<Output = T> + AddAssign,
    {
        // 获取索引 ID
        // Get index IDs
        let lhs_ids = Idx1::to_ids();
        let rhs_ids = Idx2::to_ids();
        let out_ids = OutIdx::to_ids();

        // 找出公共索引（求和索引）
        // Find common indices (summation indices)
        let common_ids = find_common_indices(&lhs_ids, &rhs_ids);

        // 获取形状信息
        // Get shape information
        let lhs_shape: Vec<usize> = (0..self.data().shape.dimension())
            .filter_map(|i| self.data().shape.len_of_dimension(i).ok())
            .collect();
        let rhs_shape: Vec<usize> = (0..other.data().shape.dimension())
            .filter_map(|i| other.data().shape.len_of_dimension(i).ok())
            .collect();

        // 验证公共索引的维度匹配
        // Validate dimension match for common indices
        for &common_id in &common_ids {
            let lhs_pos = lhs_ids.iter().position(|&id| id == common_id);
            let rhs_pos = rhs_ids.iter().position(|&id| id == common_id);

            if let (Some(lp), Some(rp)) = (lhs_pos, rhs_pos) {
                if lhs_shape.get(lp) != rhs_shape.get(rp) {
                    return Err(EinsumError::IncompatibleShapes {
                        shape1: lhs_shape,
                        shape2: rhs_shape,
                        message: format!("Dimension mismatch for common index {}", common_id),
                    });
                }
            }
        }

        // 计算输出形状
        // Calculate output shape
        let mut out_shape = Vec::new();

        // 从 lhs 添加非求和索引的维度
        // Add dimensions from lhs for non-summation indices
        for (i, &id) in lhs_ids.iter().enumerate() {
            if !common_ids.contains(&id) && out_ids.contains(&id) {
                if let Some(dim) = lhs_shape.get(i) {
                    out_shape.push(*dim);
                }
            }
        }

        // 从 rhs 添加非求和索引的维度
        // Add dimensions from rhs for non-summation indices
        for (i, &id) in rhs_ids.iter().enumerate() {
            if !common_ids.contains(&id) && out_ids.contains(&id) {
                if let Some(dim) = rhs_shape.get(i) {
                    out_shape.push(*dim);
                }
            }
        }

        // 如果输出形状为空，至少需要一个维度（标量结果）
        // If output shape is empty, need at least one dimension (scalar result)
        if out_shape.is_empty() && !out_ids.is_empty() {
            // 标量输出
            // Scalar output
            out_shape.push(1);
        }

        // 创建输出数组
        // Create output array
        let out_dyn_shape = DynShape::new(out_shape);
        let mut result = MultiArray::<T, DynShape>::new_with(out_dyn_shape, T::zero());

        // 执行爱因斯坦求和
        // Perform Einstein summation
        perform_einsum(
            self.data(),
            &lhs_ids,
            &lhs_shape,
            other.data(),
            &rhs_ids,
            &rhs_shape,
            &mut result,
            &out_ids,
            &common_ids,
        )?;

        Ok(result)
    }
}

// ============================================================================
// 辅助函数
// ============================================================================

/// 执行爱因斯坦求和
/// Perform Einstein summation
///
/// # 参数 / Arguments
///
/// - `lhs`: 左操作数
/// - `lhs_ids`: 左操作数索引 ID
/// - `lhs_shape`: 左操作数形状
/// - `rhs`: 右操作数
/// - `rhs_ids`: 右操作数索引 ID
/// - `rhs_shape`: 右操作数形状
/// - `result`: 输出数组
/// - `out_ids`: 输出索引 ID
/// - `common_ids`: 公共索引（求和索引）
fn perform_einsum<T, S1, S2>(
    lhs: &MultiArray<T, S1>,
    lhs_ids: &[usize],
    lhs_shape: &[usize],
    rhs: &MultiArray<T, S2>,
    rhs_ids: &[usize],
    rhs_shape: &[usize],
    result: &mut MultiArray<T, DynShape>,
    out_ids: &[usize],
    common_ids: &[usize],
) -> Result<(), EinsumError>
where
    S1: AbstractShape,
    S2: AbstractShape,
    T: Clone + Zero + Add<Output = T> + Mul<Output = T> + AddAssign,
{
    let fallback = |result: &mut MultiArray<T, DynShape>| perform_einsum_pairs(
        lhs, lhs_ids, lhs_shape, rhs, rhs_ids, rhs_shape, result, out_ids, common_ids,
    );
    let Some((lhs_strides, lhs_size)) = row_major_strides(lhs_shape) else { return fallback(result); };
    let Some((rhs_strides, rhs_size)) = row_major_strides(rhs_shape) else { return fallback(result); };
    let Some(output_shape) = (0..result.shape.dimension()).map(|axis| result.shape.len_of_dimension(axis).ok())
        .collect::<Option<Vec<_>>>() else { return fallback(result); };
    let Some((output_strides, output_size)) = row_major_strides(&output_shape) else { return fallback(result); };
    if lhs_size != lhs.len() || rhs_size != rhs.len() || output_size != result.len() { return fallback(result); }
    if lhs_size == 0 || rhs_size == 0 { return Ok(()); }

    let mut rhs_sources = vec![None; rhs_shape.len()];
    for id in common_ids {
        if let (Some(left), Some(right)) = (lhs_ids.iter().position(|value| value == id), rhs_ids.iter().position(|value| value == id)) {
            if left >= lhs_shape.len() || right >= rhs_shape.len() { return fallback(result); }
            rhs_sources[right] = Some(left);
        }
    }
    let free_axes = rhs_sources.iter().enumerate().filter_map(|(axis, source)| source.is_none().then_some(axis)).collect::<Vec<_>>();
    let Some(output_axes) = out_ids.iter().map(|id| {
        lhs_ids.iter().position(|value| value == id).filter(|axis| *axis < lhs_shape.len()).map(|axis| (true, axis))
            .or_else(|| rhs_ids.iter().position(|value| value == id).filter(|axis| *axis < rhs_shape.len()).map(|axis| (false, axis)))
    }).collect::<Option<Vec<_>>>() else { return fallback(result); };
    let mut lhs_coords = vec![0; lhs_shape.len()];
    let mut rhs_coords = vec![0; rhs_shape.len()];
    // 保留原来的lhs外循环/rhs物理顺序，以及重复标签首次匹配；只枚举满足共享坐标的rhs项。 / Retain lhs-outer/rhs-physical order and first matches for duplicate labels; enumerate only matching rhs items.
    'left: for lhs_linear in 0..lhs_size {
        for (axis, coordinate) in lhs_coords.iter_mut().enumerate() {
            *coordinate = lhs_linear / lhs_strides[axis] % lhs_shape[axis];
        }
        let mut rhs_linear = 0;
        for (axis, source) in rhs_sources.iter().enumerate() {
            let coordinate = source.map_or(0, |left| lhs_coords[left]);
            if coordinate >= rhs_shape[axis] { continue 'left; }
            rhs_coords[axis] = coordinate;
            rhs_linear += coordinate * rhs_strides[axis];
        }
        loop {
            let product = lhs[lhs_linear].clone() * rhs[rhs_linear].clone();
            let mut output_linear = 0;
            let mut valid = output_axes.len() == output_shape.len();
            for (axis, &(left, source)) in output_axes.iter().enumerate() {
                let coordinate = if left { lhs_coords[source] } else { rhs_coords[source] };
                if output_shape.get(axis).is_none_or(|size| coordinate >= *size) { valid = false; break; }
                output_linear += coordinate * output_strides[axis];
            }
            if valid { result[output_linear] += product; }
            let mut advanced = false;
            for &axis in free_axes.iter().rev() {
                if rhs_coords[axis] + 1 < rhs_shape[axis] {
                    rhs_coords[axis] += 1;
                    rhs_linear += rhs_strides[axis];
                    advanced = true;
                    break;
                }
                rhs_linear -= rhs_coords[axis] * rhs_strides[axis];
                rhs_coords[axis] = 0;
            }
            if !advanced { break; }
        }
    }
    Ok(())
}

fn row_major_strides(shape: &[usize]) -> Option<(Vec<usize>, usize)> {
    let mut strides = vec![0; shape.len()];
    let mut size = 1usize;
    for (axis, dimension) in shape.iter().enumerate().rev() {
        strides[axis] = size;
        size = size.checked_mul(*dimension)?;
    }
    Some((strides, size))
}

// 保留非常规形状/缺失索引的兼容路径，也作为精确累加顺序的参考。 / Retain compatibility for irregular shapes/missing indices and a reference for exact accumulation order.
fn perform_einsum_pairs<T, S1, S2>(
    lhs: &MultiArray<T, S1>,
    lhs_ids: &[usize],
    lhs_shape: &[usize],
    rhs: &MultiArray<T, S2>,
    rhs_ids: &[usize],
    rhs_shape: &[usize],
    result: &mut MultiArray<T, DynShape>,
    out_ids: &[usize],
    common_ids: &[usize],
) -> Result<(), EinsumError>
where
    S1: AbstractShape,
    S2: AbstractShape,
    T: Clone + Zero + Add<Output = T> + Mul<Output = T> + AddAssign,
{
    let lhs_len = lhs.len();
    let rhs_len = rhs.len();

    // 对于每个 lhs 元素
    // For each lhs element
    for lhs_linear in 0..lhs_len {
        // 计算 lhs 的向量坐标
        // Calculate lhs vector coordinates
        let lhs_coords = linear_to_coords(lhs_linear, lhs_shape);

        // 对于每个 rhs 元素
        // For each rhs element
        for rhs_linear in 0..rhs_len {
            // 计算 rhs 的向量坐标
            // Calculate rhs vector coordinates
            let rhs_coords = linear_to_coords(rhs_linear, rhs_shape);

            // 检查公共索引是否匹配
            // Check if common indices match
            if !check_common_indices_match(&lhs_coords, lhs_ids, &rhs_coords, rhs_ids, common_ids) {
                continue;
            }

            // 计算输出坐标
            // Calculate output coordinates
            let out_coords = calculate_output_coords(
                &lhs_coords,
                lhs_ids,
                &rhs_coords,
                rhs_ids,
                out_ids,
                common_ids,
            );

            // 计算乘积并累加到结果
            // Calculate product and accumulate to result
            let lhs_val = &lhs[lhs_linear];
            let rhs_val = &rhs[rhs_linear];
            let product = lhs_val.clone() * rhs_val.clone();

            // 计算结果的线性索引
            // Calculate result linear index
            if let Some(result_linear) = coords_to_linear(&out_coords, &result.shape) {
                result[result_linear] += product;
            }
        }
    }

    Ok(())
}

/// 线性索引转坐标
/// Convert linear index to coordinates
fn linear_to_coords(linear: usize, shape: &[usize]) -> Vec<usize> {
    let ndim = shape.len();
    let mut coords = vec![0; ndim];
    let mut remaining = linear;

    for i in 0..ndim {
        let stride: usize = shape[i + 1..].iter().copied().product::<usize>().max(1);
        coords[i] = remaining / stride;
        remaining %= stride;
    }

    coords
}

/// 坐标转线性索引
/// Convert coordinates to linear index
fn coords_to_linear(coords: &[usize], shape: &DynShape) -> Option<usize> {
    if coords.len() != shape.dimension() {
        return None;
    }

    let mut linear = 0;
    let mut stride = 1;

    for i in (0..shape.dimension()).rev() {
        let dim_len = shape.len_of_dimension(i).ok()?;
        if *coords.get(i)? >= dim_len {
            return None;
        }
        linear += coords[i] * stride;
        stride *= dim_len;
    }

    Some(linear)
}

/// 检查公共索引是否匹配
/// Check if common indices match
fn check_common_indices_match(
    lhs_coords: &[usize],
    lhs_ids: &[usize],
    rhs_coords: &[usize],
    rhs_ids: &[usize],
    common_ids: &[usize],
) -> bool {
    for &common_id in common_ids {
        let lhs_pos = lhs_ids.iter().position(|&id| id == common_id);
        let rhs_pos = rhs_ids.iter().position(|&id| id == common_id);

        if let (Some(lp), Some(rp)) = (lhs_pos, rhs_pos) {
            if lhs_coords.get(lp) != rhs_coords.get(rp) {
                return false;
            }
        }
    }

    true
}

/// 计算输出坐标
/// Calculate output coordinates
fn calculate_output_coords(
    lhs_coords: &[usize],
    lhs_ids: &[usize],
    rhs_coords: &[usize],
    rhs_ids: &[usize],
    out_ids: &[usize],
    _common_ids: &[usize],
) -> Vec<usize> {
    let mut out_coords = Vec::with_capacity(out_ids.len());

    for &out_id in out_ids {
        // 先在 lhs 中查找
        // First search in lhs
        if let Some(pos) = lhs_ids.iter().position(|&id| id == out_id) {
            if let Some(&coord) = lhs_coords.get(pos) {
                out_coords.push(coord);
                continue;
            }
        }

        // 再在 rhs 中查找
        // Then search in rhs
        if let Some(pos) = rhs_ids.iter().position(|&id| id == out_id) {
            if let Some(&coord) = rhs_coords.get(pos) {
                out_coords.push(coord);
            }
        }
    }

    out_coords
}

#[cfg(test)]
mod tests {
    use super::super::indices::{I, IL, IL2, J, K};
    use super::*;
    use crate::{MultiArrayBuilder, Shape};

    struct Case<'a> {
        lhs_shape: &'a [usize],
        rhs_shape: &'a [usize],
        lhs_ids: &'a [usize],
        rhs_ids: &'a [usize],
        output_shape: &'a [usize],
        output_ids: &'a [usize],
        common: &'a [usize],
    }

    fn compare_pair_reference(case: &Case<'_>) {
        for order in [crate::StorageOrder::RowMajor, crate::StorageOrder::ColumnMajor] {
            for palette in [&[1e16f64, 1.0, -1e16, -0.0, 0.0, -3.0][..],
                &[1e16f64, 1.0, -1e16, -0.0, 0.0, -3.0, f64::INFINITY, f64::NAN][..]]
            {
                let lhs: MultiArray<f64, DynShape> = MultiArrayBuilder::new_by(
                    DynShape::new_with_order(case.lhs_shape.to_vec(), order), |index, coordinates| palette[(index + coordinates.iter().copied().sum::<usize>()) % palette.len()],
                );
                let rhs: MultiArray<f64, DynShape> = MultiArrayBuilder::new_by(
                    DynShape::new_with_order(case.rhs_shape.to_vec(), order), |index, coordinates| palette[(index + coordinates.iter().copied().sum::<usize>() + 1) % palette.len()],
                );
                let mut actual = MultiArray::new_with(DynShape::new(case.output_shape.to_vec()), 0.0);
                let mut expected = MultiArray::new_with(DynShape::new(case.output_shape.to_vec()), 0.0);
                perform_einsum(&lhs, case.lhs_ids, case.lhs_shape, &rhs, case.rhs_ids, case.rhs_shape,
                    &mut actual, case.output_ids, case.common).unwrap();
                perform_einsum_pairs(&lhs, case.lhs_ids, case.lhs_shape, &rhs, case.rhs_ids, case.rhs_shape,
                    &mut expected, case.output_ids, case.common).unwrap();
                for index in 0..expected.len() { assert_eq!(actual[index].to_bits(), expected[index].to_bits(), "index={index} order={order:?}"); }
            }
        }
    }

    #[test]
    fn generic_pair_plan_preserves_first_matches_empty_axes_and_physical_order() {
        for case in [
            Case { lhs_shape: &[2, 3], rhs_shape: &[3, 4], lhs_ids: &[0, 1], rhs_ids: &[1, 2], output_shape: &[2, 4], output_ids: &[0, 2], common: &[1] },
            Case { lhs_shape: &[2], rhs_shape: &[3], lhs_ids: &[0], rhs_ids: &[1], output_shape: &[2, 3], output_ids: &[0, 1], common: &[] },
            Case { lhs_shape: &[3], rhs_shape: &[3], lhs_ids: &[0], rhs_ids: &[0], output_shape: &[], output_ids: &[], common: &[0] },
            Case { lhs_shape: &[2, 3], rhs_shape: &[2, 4], lhs_ids: &[0, 0], rhs_ids: &[0, 2], output_shape: &[2, 4], output_ids: &[0, 2], common: &[0, 0] },
            Case { lhs_shape: &[2, 3], rhs_shape: &[3, 2, 4], lhs_ids: &[0, 1], rhs_ids: &[1, 1, 2], output_shape: &[2, 4], output_ids: &[0, 2], common: &[1] },
            Case { lhs_shape: &[2, 3], rhs_shape: &[3, 4], lhs_ids: &[0, 1], rhs_ids: &[1, 2], output_shape: &[4, 2, 4], output_ids: &[2, 0, 2], common: &[1] },
            Case { lhs_shape: &[2, 3], rhs_shape: &[2, 3], lhs_ids: &[0, 1], rhs_ids: &[0, 1], output_shape: &[], output_ids: &[], common: &[0, 1] },
            Case { lhs_shape: &[], rhs_shape: &[], lhs_ids: &[], rhs_ids: &[], output_shape: &[], output_ids: &[], common: &[] },
            Case { lhs_shape: &[0, 3], rhs_shape: &[3, 2], lhs_ids: &[0, 1], rhs_ids: &[1, 2], output_shape: &[0, 2], output_ids: &[0, 2], common: &[1] },
            Case { lhs_shape: &[2, 3], rhs_shape: &[3, 4], lhs_ids: &[0, 1], rhs_ids: &[1, 2], output_shape: &[1, 1], output_ids: &[0, 2], common: &[1] },
            Case { lhs_shape: &[2], rhs_shape: &[3], lhs_ids: &[0, 99], rhs_ids: &[1, 99], output_shape: &[2, 3], output_ids: &[0, 1], common: &[99] },
            Case { lhs_shape: &[2], rhs_shape: &[3], lhs_ids: &[0], rhs_ids: &[1], output_shape: &[2, 3], output_ids: &[0, 99], common: &[] },
        ] { compare_pair_reference(&case); }
        let mut random = 0x1234_5678u64;
        for _ in 0..256 {
            let mut next = || { random = random.wrapping_mul(6364136223846793005).wrapping_add(1); (random >> 32) as usize };
            let left = [next() % 4, next() % 4];
            let right = [next() % 4, next() % 4, next() % 4];
            let output = [next() % 4, next() % 4];
            let common = find_common_indices(&left, &right);
            compare_pair_reference(&Case { lhs_shape: &[2, 3], rhs_shape: &[3, 2, 2], lhs_ids: &left, rhs_ids: &right,
                output_shape: &[3, 2], output_ids: &output, common: &common });
        }
    }

    #[test]
    fn pair_enumeration_retains_generic_multiplication_order_even_for_discarded_outputs() {
        use std::cell::RefCell;
        thread_local! { static CALLS: RefCell<Vec<(usize, usize)>> = const { RefCell::new(Vec::new()) }; }
        #[derive(Clone, Debug, PartialEq)]
        struct Ordered { id: usize, terms: Vec<(usize, usize)> }
        impl AddAssign for Ordered { fn add_assign(&mut self, rhs: Self) { self.terms.extend(rhs.terms); } }
        impl Add for Ordered { type Output = Self; fn add(mut self, rhs: Self) -> Self { self += rhs; self } }
        impl Mul for Ordered {
            type Output = Self;
            fn mul(self, rhs: Self) -> Self {
                CALLS.with(|calls| calls.borrow_mut().push((self.id, rhs.id)));
                Self { id: 0, terms: vec![(self.id, rhs.id)] }
            }
        }
        impl Zero for Ordered { fn zero() -> Self { Self { id: 0, terms: Vec::new() } } fn is_zero(&self) -> bool { self.terms.is_empty() } }
        for case in [
            Case { lhs_shape: &[2, 3], rhs_shape: &[3, 2], lhs_ids: &[0, 1], rhs_ids: &[1, 2], output_shape: &[2, 2], output_ids: &[0, 2], common: &[1] },
            Case { lhs_shape: &[2, 3], rhs_shape: &[2, 2, 3, 2], lhs_ids: &[0, 1], rhs_ids: &[0, 2, 1, 3], output_shape: &[2, 2], output_ids: &[2, 3], common: &[0, 1] },
        ] {
            let lhs: MultiArray<Ordered, DynShape> = MultiArrayBuilder::new_by(DynShape::new(case.lhs_shape.to_vec()), |id, _| Ordered { id, terms: Vec::new() });
            let rhs: MultiArray<Ordered, DynShape> = MultiArrayBuilder::new_by(DynShape::new(case.rhs_shape.to_vec()), |id, _| Ordered { id: id + 10, terms: Vec::new() });
            for output_shape in [case.output_shape.to_vec(), vec![1, 1], vec![4]] {
                let mut expected = MultiArray::new_with(DynShape::new(output_shape.clone()), Ordered::zero());
                let mut actual = MultiArray::new_with(DynShape::new(output_shape), Ordered::zero());
                CALLS.with(|calls| calls.borrow_mut().clear());
                perform_einsum_pairs(&lhs, case.lhs_ids, case.lhs_shape, &rhs, case.rhs_ids, case.rhs_shape, &mut expected, case.output_ids, case.common).unwrap();
                let expected_calls = CALLS.with(|calls| std::mem::take(&mut *calls.borrow_mut()));
                perform_einsum(&lhs, case.lhs_ids, case.lhs_shape, &rhs, case.rhs_ids, case.rhs_shape, &mut actual, case.output_ids, case.common).unwrap();
                assert_eq!(CALLS.with(|calls| std::mem::take(&mut *calls.borrow_mut())), expected_calls);
                for index in 0..actual.len() { assert_eq!(actual[index], expected[index]); }
            }
        }
    }

    #[test]
    fn public_generic_contraction_matches_specialized_matmul_and_compatibility_path() {
        let lhs: MultiArray<f64, DynShape> = MultiArrayBuilder::new_by(DynShape::new(vec![2, 3]), |index, _| index as f64 - 2.0);
        let rhs: MultiArray<f64, DynShape> = MultiArrayBuilder::new_by(DynShape::new(vec![3, 4]), |index, _| index as f64 + 1.0);
        let left: TensorExpr<'_, _, _, IL2<I, J>> = TensorExpr::new(&lhs);
        let right: TensorExpr<'_, _, _, IL2<J, K>> = TensorExpr::new(&rhs);
        let actual = <_ as EinsteinSum<'_, f64, _, IL2<I, J>, _, IL2<J, K>, IL2<I, K>>>::einsum(&left, &right).unwrap();
        let expected = crate::einsum::matmul(&lhs, &rhs).unwrap();
        for index in 0..actual.len() { assert_eq!(actual[index].to_bits(), expected[index].to_bits()); }
        let wrong: MultiArray<f64, DynShape> = MultiArray::new_with(DynShape::new(vec![2, 4]), 1.0);
        let wrong: TensorExpr<'_, _, _, IL2<J, K>> = TensorExpr::new(&wrong);
        assert!(matches!(<_ as EinsteinSum<'_, f64, _, IL2<I, J>, _, IL2<J, K>, IL2<I, K>>>::einsum(&left, &wrong), Err(EinsumError::IncompatibleShapes { .. })));
        let mut expected = MultiArray::new_with(DynShape::new(vec![2, 4]), 0.0);
        let mut actual = MultiArray::new_with(DynShape::new(vec![2, 4]), 0.0);
        perform_einsum_pairs(&lhs, &[0, 1], &[2, 2], &rhs, &[1, 2], &[3, 4], &mut expected, &[0, 2], &[1]).unwrap();
        perform_einsum(&lhs, &[0, 1], &[2, 2], &rhs, &[1, 2], &[3, 4], &mut actual, &[0, 2], &[1]).unwrap();
        for index in 0..actual.len() { assert_eq!(actual[index].to_bits(), expected[index].to_bits()); }
    }

    #[test]
    #[ignore = "手动 release 性能测量 / Manual release performance measurement"]
    fn performance_b09_generic_contraction() {
        use std::hint::black_box;
        use std::time::Instant;
        for size in [8, 24, 48] {
            let lhs: MultiArray<f64, DynShape> = MultiArray::new_with(DynShape::new(vec![size, size]), 1.0f64);
            let rhs: MultiArray<f64, DynShape> = MultiArray::new_with(DynShape::new(vec![size, size]), 2.0f64);
            for sample in 0..5 {
                let mut result = MultiArray::new_with(DynShape::new(vec![size, size]), 0.0f64);
                let start = Instant::now();
                perform_einsum(black_box(&lhs), &[0, 1], &[size, size], black_box(&rhs), &[1, 2], &[size, size],
                    black_box(&mut result), &[0, 2], &[1]).unwrap();
                let elapsed = start.elapsed();
                for index in 0..size * size { assert_eq!(result[index], 2.0 * size as f64); }
                black_box(result);
                println!("generic_contraction size={size} sample={sample} elapsed={elapsed:?}");
            }
        }
    }

    #[test]
    fn test_linear_to_coords() {
        let shape = vec![2, 3];

        // 线性索引 0 -> [0, 0]
        assert_eq!(linear_to_coords(0, &shape), vec![0, 0]);

        // 线性索引 1 -> [0, 1]
        assert_eq!(linear_to_coords(1, &shape), vec![0, 1]);

        // 线性索引 3 -> [1, 0]
        assert_eq!(linear_to_coords(3, &shape), vec![1, 0]);
    }

    #[test]
    fn test_check_common_indices_match() {
        let lhs_coords = vec![1, 2, 3]; // i=1, j=2, k=3
        let lhs_ids = vec![0, 1, 2]; // i, j, k
        let rhs_coords = vec![2, 3, 4]; // j=2, k=3, l=4
        let rhs_ids = vec![1, 2, 3]; // j, k, l
        let common_ids = vec![1, 2]; // j, k

        // j=2 在两边匹配，k=3 在两边匹配
        // j=2 matches on both sides, k=3 matches on both sides
        assert!(check_common_indices_match(
            &lhs_coords,
            &lhs_ids,
            &rhs_coords,
            &rhs_ids,
            &common_ids
        ));

        // 测试不匹配情况
        // Test mismatch case
        let rhs_coords_mismatch = vec![5, 3, 4]; // j=5, k=3, l=4
        assert!(!check_common_indices_match(
            &lhs_coords,
            &lhs_ids,
            &rhs_coords_mismatch,
            &rhs_ids,
            &common_ids
        ));
    }
}
