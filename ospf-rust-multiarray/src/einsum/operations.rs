//! 常见爱因斯坦运算实现
//! Common Einstein operations implementation
//!
//! 提供矩阵乘法、点积、迹等常见运算的便捷函数。
//! Provides convenience functions for common operations like matrix multiplication, dot product, and trace.

use super::einsum_trait::EinsumError;
use super::indices::{I, IL, IL2, IndexLabel, IndexList, J, K};
use super::tensor_expr::TensorExpr;
use crate::{AbstractShape, DynShape, MultiArray};
use num_traits::Zero;
use std::ops::{Add, AddAssign, Mul};

// ============================================================================
// 矩阵乘法 / Matrix Multiplication
// ============================================================================

/// 矩阵乘法
/// Matrix multiplication
///
/// 计算 `C = A @ B`，其中 A 的形状为 `[m, k]`，B 的形状为 `[k, n]`，
/// 结果 C 的形状为 `[m, n]`。
/// Computes `C = A @ B`, where A has shape `[m, k]`, B has shape `[k, n]`,
/// and result C has shape `[m, n]`.
///
/// # 参数 / Arguments
///
/// - `a`: 左矩阵，形状 `[m, k]`
/// - `b`: 右矩阵，形状 `[k, n]`
///
/// # 返回 / Returns
///
/// 结果矩阵，形状 `[m, n]`
/// Result matrix with shape `[m, n]`
pub fn matmul<T, S1, S2>(
    a: &MultiArray<T, S1>,
    b: &MultiArray<T, S2>,
) -> Result<MultiArray<T, DynShape>, EinsumError>
where
    S1: AbstractShape,
    S2: AbstractShape,
    T: Clone + Zero + Add<Output = T> + Mul<Output = T> + AddAssign,
{
    // 验证维度
    // Validate dimensions
    let a_dim = a.shape.dimension();
    let b_dim = b.shape.dimension();

    if a_dim != 2 || b_dim != 2 {
        return Err(EinsumError::DimensionMismatch {
            expected: 2,
            actual: a_dim.max(b_dim),
            message: "Matrix multiplication requires 2D arrays".to_string(),
        });
    }

    // 获取形状
    // Get shapes
    let a_rows = a
        .shape
        .len_of_dimension(0)
        .map_err(|e| EinsumError::DimensionMismatch {
            expected: 0,
            actual: 0,
            message: format!("Failed to get row count of matrix A: {:?}", e),
        })?;
    let a_cols = a
        .shape
        .len_of_dimension(1)
        .map_err(|e| EinsumError::DimensionMismatch {
            expected: 1,
            actual: 1,
            message: format!("Failed to get column count of matrix A: {:?}", e),
        })?;
    let b_rows = b
        .shape
        .len_of_dimension(0)
        .map_err(|e| EinsumError::DimensionMismatch {
            expected: 0,
            actual: 0,
            message: format!("Failed to get row count of matrix B: {:?}", e),
        })?;
    let b_cols = b
        .shape
        .len_of_dimension(1)
        .map_err(|e| EinsumError::DimensionMismatch {
            expected: 1,
            actual: 1,
            message: format!("Failed to get column count of matrix B: {:?}", e),
        })?;

    if a_cols != b_rows {
        return Err(EinsumError::IncompatibleShapes {
            shape1: vec![a_rows, a_cols],
            shape2: vec![b_rows, b_cols],
            message: format!(
                "Matrix dimensions don't align: {}x{} and {}x{}",
                a_rows, a_cols, b_rows, b_cols
            ),
        });
    }

    // 创建结果矩阵
    // Create result matrix
    let result_shape = DynShape::new(vec![a_rows, b_cols]);
    let mut result = MultiArray::<T, DynShape>::new_with(result_shape, T::zero());

    // 执行矩阵乘法
    // Perform matrix multiplication
    for i in 0..a_rows {
        for j in 0..b_cols {
            let mut sum = T::zero();
            for k in 0..a_cols {
                let a_idx = i * a_cols + k;
                let b_idx = k * b_cols + j;
                if a_idx < a.len() && b_idx < b.len() {
                    sum = sum + a[a_idx].clone() * b[b_idx].clone();
                }
            }
            let result_idx = i * b_cols + j;
            if result_idx < result.len() {
                result[result_idx] = sum;
            }
        }
    }

    Ok(result)
}

// ============================================================================
// 点积 / Dot Product
// ============================================================================

/// 向量点积
/// Vector dot product
///
/// 计算两个向量的点积（内积）。
/// Computes the dot product (inner product) of two vectors.
///
/// # 参数 / Arguments
///
/// - `a`: 第一个向量
/// - `b`: 第二个向量
///
/// # 返回 / Returns
///
/// 点积结果（标量）
/// Dot product result (scalar)
pub fn dot<T, S1, S2>(a: &MultiArray<T, S1>, b: &MultiArray<T, S2>) -> Result<T, EinsumError>
where
    S1: AbstractShape,
    S2: AbstractShape,
    T: Clone + Zero + Add<Output = T> + Mul<Output = T> + AddAssign,
{
    // 验证维度
    // Validate dimensions
    let a_dim = a.shape.dimension();
    let b_dim = b.shape.dimension();

    if a_dim != 1 || b_dim != 1 {
        return Err(EinsumError::DimensionMismatch {
            expected: 1,
            actual: a_dim.max(b_dim),
            message: "Dot product requires 1D vectors".to_string(),
        });
    }

    if a.len() != b.len() {
        return Err(EinsumError::IncompatibleShapes {
            shape1: vec![a.len()],
            shape2: vec![b.len()],
            message: "Vector lengths don't match".to_string(),
        });
    }

    let mut result = T::zero();
    for i in 0..a.len() {
        result = result + a[i].clone() * b[i].clone();
    }

    Ok(result)
}

// ============================================================================
// 迹 / Trace
// ============================================================================

/// 矩阵迹
/// Matrix trace
///
/// 计算方阵的迹（对角元素之和）。
/// Computes the trace of a square matrix (sum of diagonal elements).
///
/// # 参数 / Arguments
///
/// - `a`: 方阵
///
/// # 返回 / Returns
///
/// 迹（标量）
/// Trace (scalar)
pub fn trace<T, S>(a: &MultiArray<T, S>) -> Result<T, EinsumError>
where
    S: AbstractShape,
    T: Clone + Zero + Add<Output = T> + Mul<Output = T> + AddAssign,
{
    let shape: Vec<usize> = (0..a.shape.dimension())
        .filter_map(|i| a.shape.len_of_dimension(i).ok())
        .collect();

    if shape.len() != 2 {
        return Err(EinsumError::UnsupportedOperation {
            message: "Trace only defined for 2D matrices".to_string(),
        });
    }

    if shape[0] != shape[1] {
        return Err(EinsumError::UnsupportedOperation {
            message: "Trace only defined for square matrices".to_string(),
        });
    }

    let n = shape[0];
    let mut result = T::zero();

    for i in 0..n {
        let idx = i * (n + 1);
        if idx < a.len() {
            result = result + a[idx].clone();
        }
    }

    Ok(result)
}

// ============================================================================
// 外积 / Outer Product
// ============================================================================

/// 向量外积
/// Vector outer product
///
/// 计算两个向量的外积，生成矩阵。
/// Computes the outer product of two vectors, producing a matrix.
///
/// # 参数 / Arguments
///
/// - `a`: 第一个向量
/// - `b`: 第二个向量
///
/// # 返回 / Returns
///
/// 外积矩阵
/// Outer product matrix
pub fn outer<T, S1, S2>(
    a: &MultiArray<T, S1>,
    b: &MultiArray<T, S2>,
) -> Result<MultiArray<T, DynShape>, EinsumError>
where
    S1: AbstractShape,
    S2: AbstractShape,
    T: Clone + Zero + Add<Output = T> + Mul<Output = T> + AddAssign,
{
    // 验证维度
    // Validate dimensions
    let a_dim = a.shape.dimension();
    let b_dim = b.shape.dimension();

    if a_dim != 1 || b_dim != 1 {
        return Err(EinsumError::DimensionMismatch {
            expected: 1,
            actual: a_dim.max(b_dim),
            message: "Outer product requires 1D vectors".to_string(),
        });
    }

    let a_len = a.len();
    let b_len = b.len();

    // 创建结果矩阵
    // Create result matrix
    let result_shape = DynShape::new(vec![a_len, b_len]);
    let mut result = MultiArray::<T, DynShape>::new_with(result_shape, T::zero());

    // 计算外积
    // Calculate outer product
    for i in 0..a_len {
        for j in 0..b_len {
            let idx = i * b_len + j;
            if idx < result.len() {
                result[idx] = a[i].clone() * b[j].clone();
            }
        }
    }

    Ok(result)
}

// ============================================================================
// 转置 / Transpose
// ============================================================================

/// 矩阵转置
/// Matrix transpose
///
/// 返回矩阵的转置。
/// Returns the transpose of a matrix.
///
/// # 参数 / Arguments
///
/// - `a`: 输入矩阵
///
/// # 返回 / Returns
///
/// 转置矩阵
/// Transposed matrix
pub fn transpose<T, S>(a: &MultiArray<T, S>) -> Result<MultiArray<T, DynShape>, EinsumError>
where
    S: AbstractShape,
    T: Clone,
{
    let shape: Vec<usize> = (0..a.shape.dimension())
        .filter_map(|i| a.shape.len_of_dimension(i).ok())
        .collect();

    if shape.len() != 2 {
        return Err(EinsumError::UnsupportedOperation {
            message: "Transpose only defined for 2D matrices".to_string(),
        });
    }

    let transposed_shape = DynShape::new(vec![shape[1], shape[0]]);
    let mut result = MultiArray::<T, DynShape>::new_with(transposed_shape, a[0].clone());

    for i in 0..shape[0] {
        for j in 0..shape[1] {
            let src_idx = i * shape[1] + j;
            let dst_idx = j * shape[0] + i;
            if src_idx < a.len() && dst_idx < result.len() {
                result[dst_idx] = a[src_idx].clone();
            }
        }
    }

    Ok(result)
}

// ============================================================================
// 张量缩并 / Tensor Contraction
// ============================================================================

/// 沿指定轴的张量缩并
/// Tensor contraction along specified axes
///
/// # 参数 / Arguments
///
/// - `a`: 第一个张量
/// - `axis_a`: 第一个张量的缩并轴
/// - `b`: 第二个张量
/// - `axis_b`: 第二个张量的缩并轴
///
/// # 返回 / Returns
///
/// 缩并结果
/// Contraction result
///
/// # 语义 / Semantics
///
/// 沿 `axis_a`／`axis_b` 求和，输出形状是两侧各自去掉该轴后按顺序拼接；两轴长度必须相等。
/// 这是真正的张量缩并，二维情形等价于矩阵乘，一维情形等价于点积。
///
/// Sum over `axis_a`／`axis_b`: the output shape concatenates the remaining axes of each side in order, and the
/// two contracted axes must have equal length. This is a real tensor contraction, equal to matrix multiplication
/// in the 2D case and to a dot product in the 1D case.
pub fn contract<T, S1, S2>(
    a: &MultiArray<T, S1>,
    axis_a: usize,
    b: &MultiArray<T, S2>,
    axis_b: usize,
) -> Result<MultiArray<T, DynShape>, EinsumError>
where
    S1: AbstractShape,
    S2: AbstractShape,
    T: Clone + Zero + Add<Output = T> + Mul<Output = T> + AddAssign,
{
    let a_shape: Vec<usize> = (0..a.shape.dimension())
        .filter_map(|i| a.shape.len_of_dimension(i).ok())
        .collect();
    let b_shape: Vec<usize> = (0..b.shape.dimension())
        .filter_map(|i| b.shape.len_of_dimension(i).ok())
        .collect();

    if axis_a >= a_shape.len() || axis_b >= b_shape.len() {
        return Err(EinsumError::DimensionMismatch {
            expected: axis_a.max(axis_b),
            actual: a_shape.len().max(b_shape.len()).saturating_sub(1),
            message: "Axis index out of bounds".to_string(),
        });
    }

    if a_shape[axis_a] != b_shape[axis_b] {
        return Err(EinsumError::IncompatibleShapes {
            shape1: a_shape,
            shape2: b_shape,
            message: "Contraction axis dimensions don't match".to_string(),
        });
    }

    // 计算输出形状
    // Calculate output shape
    let mut out_shape: Vec<usize> = Vec::new();
    for (i, &dim) in a_shape.iter().enumerate() {
        if i != axis_a {
            out_shape.push(dim);
        }
    }
    for (i, &dim) in b_shape.iter().enumerate() {
        if i != axis_b {
            out_shape.push(dim);
        }
    }

    let out_dyn_shape = DynShape::new(out_shape.clone());
    let mut result = MultiArray::<T, DynShape>::new_with(out_dyn_shape, T::zero());
    // 复用本模块已有的通用 einsum 执行器：用共享标签把缩并轴对齐，其余轴按原顺序输出。
    // Reuse this module's existing generic einsum engine: align the contracted axes with a shared label and keep
    // every other axis in its original order.
    let lhs_ids: Vec<usize> = (0..a_shape.len()).map(|axis| if axis == axis_a { 0 } else { axis + 1 }).collect();
    let rhs_ids: Vec<usize> = (0..b_shape.len()).map(|axis| if axis == axis_b { 0 } else { a_shape.len() + 1 + axis }).collect();
    let out_ids: Vec<usize> = a_shape.iter().enumerate().filter(|(axis, _)| *axis != axis_a).map(|(axis, _)| lhs_ids[axis])
        .chain(b_shape.iter().enumerate().filter(|(axis, _)| *axis != axis_b).map(|(axis, _)| rhs_ids[axis])).collect();
    crate::einsum::einsum_trait::perform_einsum(a, &lhs_ids, &a_shape, b, &rhs_ids, &b_shape, &mut result, &out_ids, &[0])?;

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AccessOrder, MultiArrayBuilder, Shape};

    /// 缩并必须与独立参照一致：二维等价矩阵乘、一维等价点积、三维按定义手算。
    /// Contraction must match independent references: matrix multiplication in 2D, a dot product in 1D and a
    /// hand-computed definition in 3D.
    #[test]
    fn test_contract_matches_independent_references() {
        let a: MultiArray<f64, Shape<2>> = MultiArrayBuilder::new_by(Shape::new([2, 3]), |i, _| i as f64 + 1.0);
        let b: MultiArray<f64, Shape<2>> = MultiArrayBuilder::new_by(Shape::new([3, 2]), |i, _| i as f64 + 7.0);
        let contracted = contract(&a, 1, &b, 0).unwrap();
        assert_eq!(shape_of(&contracted), vec![2, 2]);
        assert_eq!(values(&contracted), values(&matmul(&a, &b).unwrap()));
        // 一维缩并等于点积 / A 1D contraction equals the dot product.
        let u: MultiArray<f64, Shape<1>> = MultiArrayBuilder::new_by(Shape::new([3]), |i, _| i as f64 + 1.0);
        let v: MultiArray<f64, Shape<1>> = MultiArrayBuilder::new_by(Shape::new([3]), |i, _| i as f64 + 4.0);
        assert_eq!(values(&contract(&u, 0, &v, 0).unwrap())[0], dot(&u, &v).unwrap());
        // 三维：A[i,k,j] · B[j,l] → C[i,k,l] / 3D: A[i,k,j] · B[j,l] → C[i,k,l].
        let cube: MultiArray<f64, Shape<3>> = MultiArrayBuilder::new_by(Shape::new([2, 2, 2]), |i, _| i as f64 + 1.0);
        let plane: MultiArray<f64, Shape<2>> = MultiArrayBuilder::new_by(Shape::new([2, 2]), |i, _| i as f64 + 10.0);
        let result = contract(&cube, 2, &plane, 0).unwrap();
        assert_eq!(shape_of(&result), vec![2, 2, 2]);
        // 手算：下标记 (i,k,l)，C = Σ_j A[i,k,j]·B[j,l]。
        // Hand-computed: with indices (i,k,l), C = Σ_j A[i,k,j]·B[j,l].
        for i in 0..2 { for k in 0..2 { for l in 0..2 {
            let expected = (0..2).map(|j| values(&cube)[i * 4 + k * 2 + j] * values(&plane)[j * 2 + l]).sum::<f64>();
            let actual = values(&result)[i * 4 + k * 2 + l];
            assert!((actual - expected).abs() < 1e-12, "C({i},{k},{l}) = {actual} != {expected}");
        } } }
        // 轴长不等与越界必须报错，不返回垃圾 / Mismatched or out-of-range axes must fail instead of returning garbage.
        assert!(contract(&a, 0, &b, 0).is_err());
        assert!(contract(&a, 5, &b, 0).is_err());
    }

    fn values<S: AbstractShape>(array: &MultiArray<f64, S>) -> Vec<f64> {
        array.flatten(AccessOrder::RowMajor)
    }

    fn shape_of<S: AbstractShape>(array: &MultiArray<f64, S>) -> Vec<usize> {
        (0..array.shape.dimension()).filter_map(|axis| array.shape.len_of_dimension(axis).ok()).collect()
    }

    #[test]
    fn test_matmul() {
        // 创建 2x3 矩阵
        // Create 2x3 matrix
        let a: MultiArray<f64, Shape<2>> =
            MultiArrayBuilder::new_by(Shape::new([2, 3]), |i, _| i as f64);

        // 创建 3x2 矩阵
        // Create 3x2 matrix
        let b: MultiArray<f64, Shape<2>> =
            MultiArrayBuilder::new_by(Shape::new([3, 2]), |i, _| (i + 1) as f64);

        // 矩阵乘法
        // Matrix multiplication
        let c = matmul(&a, &b).unwrap();

        // 结果形状应为 2x2
        // Result shape should be 2x2
        assert_eq!(c.shape.dimension(), 2);
    }

    #[test]
    fn test_dot() {
        let a: MultiArray<f64, Shape<1>> =
            MultiArrayBuilder::new_by(Shape::new([3]), |i, _| (i + 1) as f64);
        let b: MultiArray<f64, Shape<1>> =
            MultiArrayBuilder::new_by(Shape::new([3]), |i, _| (i + 1) as f64);

        // 点积：1*1 + 2*2 + 3*3 = 14
        // Dot product: 1*1 + 2*2 + 3*3 = 14
        let result = dot(&a, &b).unwrap();
        assert!((result - 14.0).abs() < 1e-10);
    }

    #[test]
    fn test_trace() {
        // 创建 3x3 单位矩阵
        // Create 3x3 identity matrix
        let a: MultiArray<f64, Shape<2>> =
            MultiArrayBuilder::new_by(
                Shape::new([3, 3]),
                |i, _| {
                    if i % 4 == 0 { 1.0 } else { 0.0 }
                },
            );

        // 迹 = 3
        // Trace = 3
        let result = trace(&a).unwrap();
        assert!((result - 3.0).abs() < 1e-10);
    }

    #[test]
    fn test_outer() {
        let a: MultiArray<f64, Shape<1>> =
            MultiArrayBuilder::new_by(Shape::new([2]), |i, _| (i + 1) as f64);
        let b: MultiArray<f64, Shape<1>> =
            MultiArrayBuilder::new_by(Shape::new([3]), |i, _| (i + 1) as f64);

        // 外积结果形状应为 2x3
        // Outer product result shape should be 2x3
        let result = outer(&a, &b).unwrap();
        assert_eq!(result.shape.dimension(), 2);
    }

    #[test]
    fn test_transpose() {
        let a: MultiArray<f64, Shape<2>> =
            MultiArrayBuilder::new_by(Shape::new([2, 3]), |i, _| i as f64);

        let result = transpose(&a).unwrap();

        // 转置后形状应为 3x2
        // Shape after transpose should be 3x2
        assert_eq!(result.shape.len_of_dimension(0).unwrap(), 3);
        assert_eq!(result.shape.len_of_dimension(1).unwrap(), 2);
    }
}
