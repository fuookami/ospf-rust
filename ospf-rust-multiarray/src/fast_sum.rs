//! 快速求和模块
//! Fast summation module
//!
//! 提供多维数组的高效求和操作，基于 `AddAssign` 避免不必要的克隆。
//! Provides efficient summation operations for multi-dimensional arrays,
//! using `AddAssign` to avoid unnecessary cloning.
//!
//! ## 核心特性 / Core Features
//!
//! - **零克隆求和 / Zero-clone summation**: 使用 `AddAssign<&T>` 直接从引用累加
//!   Accumulate directly from references using `AddAssign<&T>`
//! - **多轴求和 / Multi-axis summation**: 支持沿指定轴或多轴求和
//!   Support summation along specified axes or multiple axes
//! - **符号运算兼容 / Symbolic operation compatible**: 支持 `Linear<f64>`、`Quadratic<f64>` 等符号类型
//!   Compatible with symbolic types like `Linear<f64>`, `Quadratic<f64>`

use crate::{AbstractShape, DynShape, MultiArray, MultiArrayCollection, Shape};
use ospf_rust_base::collection_traits::CollectionRef;
use num_traits::Zero;
use std::ops::AddAssign;

/// 求和错误类型
/// Summation error type
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SumError {
    /// 轴索引超出范围
    /// Axis index out of bounds
    AxisOutOfBounds {
        /// 请求的轴索引
        /// Requested axis index
        axis: usize,
        /// 最大有效轴索引
        /// Maximum valid axis index
        max_axis: usize,
    },
}

impl std::fmt::Display for SumError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SumError::AxisOutOfBounds { axis, max_axis } => {
                write!(
                    f,
                    "Axis {} out of bounds (max: {}) / 轴 {} 超出范围（最大: {}）",
                    axis, max_axis, axis, max_axis
                )
            }
        }
    }
}

impl std::error::Error for SumError {}

/// 快速求和 trait / Fast summation trait
///
/// 提供多维数组的高效求和操作。
/// Provides efficient summation operations for multi-dimensional arrays.
///
/// ## 类型参数 / Type Parameters
///
/// - `T`: 元素类型，必须实现 `Zero`、`Clone` 和 `AddAssign<&T>`
///   Element type, must implement `Zero`, `Clone` and `AddAssign<&T>`
pub trait FastSum<T> {
    /// 沿指定轴求和
    /// Sum along specified axis
    fn sum_axis(&self, axis: usize) -> Result<MultiArray<T, DynShape>, SumError>
    where
        T: Zero + Clone + for<'a> AddAssign<&'a T>;

    /// 沿多个轴求和
    /// Sum along multiple axes
    fn sum_axes(&self, axes: &[usize]) -> Result<MultiArray<T, DynShape>, SumError>
    where
        T: Zero + Clone + for<'a> AddAssign<&'a T>;

    /// 全局求和
    /// Sum all elements
    fn sum_all(&self) -> T
    where
        T: Zero + for<'a> AddAssign<&'a T>;
}

/// 快速累积求和 trait / Fast cumulative sum trait
///
/// 提供累积求和（前缀和）操作。
/// Provides cumulative sum (prefix sum) operations.
pub trait FastCumSum<T> {
    /// 沿指定轴累积求和（前缀和）
    /// Cumulative sum along axis (prefix sum)
    fn cumsum_axis(&self, axis: usize) -> Result<MultiArray<T, DynShape>, SumError>
    where
        T: Zero + Clone + for<'a> AddAssign<&'a T>;
}

// 为 MultiArray 实现 FastSum trait
// Implement FastSum trait for MultiArray
impl<T, S, C> FastSum<T> for MultiArray<T, S, C>
where
    S: AbstractShape,
    C: MultiArrayCollection<T>,
{
    fn sum_axis(&self, axis: usize) -> Result<MultiArray<T, DynShape>, SumError>
    where
        T: Zero + Clone + for<'a> AddAssign<&'a T>,
    {
        let ndim = self.shape.dimension();
        if axis >= ndim {
            return Err(SumError::AxisOutOfBounds {
                axis,
                max_axis: ndim.saturating_sub(1),
            });
        }

        self.sum_axes(&[axis])
    }

    fn sum_axes(&self, axes: &[usize]) -> Result<MultiArray<T, DynShape>, SumError>
    where
        T: Zero + Clone + for<'a> AddAssign<&'a T>,
    {
        if axes.is_empty() {
            // 没有指定轴，返回原数组的克隆（转换为 DynShape）
            // No axes specified, return clone of original array (converted to DynShape)
            let new_shape: Vec<usize> = (0..self.shape.dimension())
                .map(|i| {
                    self.shape
                        .len_of_dimension(i)
                        .expect("dimension should be valid in sum_axes")
                })
                .collect();
            let new_dyn_shape = DynShape::new(new_shape);
            let mut result = MultiArray::<T, DynShape>::new_with(new_dyn_shape, T::zero());
            for i in 0..self.len() {
                result[i] = self[i].clone();
            }
            return Ok(result);
        }

        let ndim = self.shape.dimension();
        for &axis in axes {
            if axis >= ndim {
                return Err(SumError::AxisOutOfBounds {
                    axis,
                    max_axis: ndim.saturating_sub(1),
                });
            }
        }

        // 获取当前形状
        // Get current shape
        let current_shape: Vec<usize> = (0..self.shape.dimension())
            .map(|i| {
                self.shape
                    .len_of_dimension(i)
                    .expect("dimension should be valid in sum_axis/sum_axes/cumsum_axis")
            })
            .collect();

        // 计算新形状（移除所有求和轴）
        // Calculate new shape (remove all summed axes)
        let axes_set: std::collections::HashSet<usize> = axes.iter().cloned().collect();
        let new_shape: Vec<usize> = current_shape
            .iter()
            .enumerate()
            .filter_map(|(i, &dim)| {
                if !axes_set.contains(&i) {
                    Some(dim)
                } else {
                    None
                }
            })
            .collect();

        // 创建结果数组
        // Create result array
        let new_dyn_shape = DynShape::new(new_shape);
        let mut result = MultiArray::<T, DynShape>::new_with(new_dyn_shape, T::zero());

        let retained_axes: Vec<usize> = (0..ndim).filter(|axis| !axes_set.contains(axis)).collect();
        let mut vector = self.shape.zero();
        let mut result_vector = vec![0; retained_axes.len()];
        // 复用坐标缓冲，保持源物理顺序和自定义映射。 / Reuse coordinate buffers, preserving source physical order and custom mappings.
        for linear_idx in 0..self.len() {
            self.shape.vector_of_into(linear_idx, &mut vector)
                .expect("linear index should be valid in sum_axes");
            for (target_axis, &source_axis) in retained_axes.iter().enumerate() {
                result_vector[target_axis] = vector[source_axis];
            }
            let result_linear_idx = result.shape.index_of(&result_vector)
                .expect("result vector should be valid in sum_axes");
            result[result_linear_idx] += &self[linear_idx];
        }

        Ok(result)
    }

    fn sum_all(&self) -> T
    where
        T: Zero + for<'a> AddAssign<&'a T>,
    {
        let mut acc = T::zero();
        for i in 0..self.len() {
            acc += &self[i];
        }
        acc
    }
}

// 为 MultiArray 实现 FastCumSum trait
// Implement FastCumSum trait for MultiArray
impl<T, S, C> FastCumSum<T> for MultiArray<T, S, C>
where
    S: AbstractShape,
    C: MultiArrayCollection<T>,
{
    fn cumsum_axis(&self, axis: usize) -> Result<MultiArray<T, DynShape>, SumError>
    where
        T: Zero + Clone + for<'a> AddAssign<&'a T>,
    {
        let ndim = self.shape.dimension();
        if axis >= ndim {
            return Err(SumError::AxisOutOfBounds {
                axis,
                max_axis: ndim.saturating_sub(1),
            });
        }

        // 获取当前形状
        // Get current shape
        let current_shape: Vec<usize> = (0..self.shape.dimension())
            .map(|i| {
                self.shape
                    .len_of_dimension(i)
                    .expect("dimension should be valid in sum_axis/sum_axes/cumsum_axis")
            })
            .collect();

        // 创建结果数组（形状相同）
        // Create result array (same shape)
        let new_dyn_shape = DynShape::new(current_shape.clone());
        let mut result = MultiArray::<T, DynShape>::new_with(new_dyn_shape, T::zero());

        // 计算沿该轴的步长和相邻元素距离
        // Calculate stride and adjacent element distance along the axis
        let axis_stride = self
            .shape
            .offset_of_dimension(axis)
            .expect("axis should be valid in cumsum_axis");
        let axis_size = current_shape[axis];

        // 计算每个"行"的大小（沿轴方向的一组元素）
        // Calculate size of each "row" (group of elements along the axis)
        let group_size = if axis_stride > 0 {
            axis_stride * axis_size
        } else {
            1
        };
        let num_groups = self.len() / group_size.max(1);

        // 遍历每个组，计算组内的累积和
        // Iterate each group and calculate cumulative sum within the group
        for group_idx in 0..num_groups {
            let group_start = group_idx * group_size.max(1);
            let mut running_sum = T::zero();

            for k in 0..axis_size {
                let elem_idx = group_start + k * axis_stride;
                if elem_idx < self.len() {
                    running_sum += &self[elem_idx];
                    result[elem_idx] = running_sum.clone();
                }
            }
        }

        // 处理边界情况：axis_stride 为 0（如最后一个维度在 RowMajor 下）
        // Handle edge case: axis_stride is 0 (e.g., last dimension in RowMajor)
        if axis_stride == 1
            || (axis + 1 == ndim
                && self.shape.storage_order() == crate::concept::StorageOrder::RowMajor)
        {
            // 直接使用线性遍历
            // Use linear traversal directly
            let stride = if axis_stride == 0 { 1 } else { axis_stride };
            let block_size = stride * axis_size;
            let num_blocks = self.len() / block_size.max(1);

            for block_idx in 0..num_blocks {
                let block_start = block_idx * block_size;
                let mut running_sum = T::zero();

                for k in 0..axis_size {
                    let elem_idx = block_start + k * stride;
                    running_sum += &self[elem_idx];
                    result[elem_idx] = running_sum.clone();
                }
            }
        }

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MultiArrayBuilder, StorageOrder};

    fn reference_sum_axes<S: AbstractShape>(array: &MultiArray<f64, S>, axes: &[usize]) -> MultiArray<f64, DynShape> {
        let dimensions = (0..array.shape.dimension()).filter(|axis| !axes.contains(axis))
            .map(|axis| array.shape.len_of_dimension(axis).unwrap()).collect();
        let mut result = MultiArray::new_with(DynShape::new(dimensions), 0.0);
        for index in 0..array.len() {
            if axes.is_empty() { result[index] = array[index]; continue; }
            let vector = array.shape.vector_of(index).unwrap();
            let coordinates = (0..array.shape.dimension()).filter(|axis| !axes.contains(axis))
                .map(|axis| vector[axis]).collect();
            let target = result.shape.index_of(&coordinates).unwrap();
            result[target] += &array[index];
        }
        result
    }

    fn assert_projection<S: AbstractShape>(shape: S) {
        let values = [1e16, 1.0, -1e16, 2.0, -0.0, 0.5];
        let array = MultiArrayBuilder::new_by(shape, |index, _| values[index % values.len()]);
        for axes in [vec![], vec![0], vec![1], vec![2], vec![2, 0], vec![1, 1]] {
            let expected = reference_sum_axes(&array, &axes);
            let actual = array.sum_axes(&axes).unwrap();
            assert_eq!(actual.shape.shape, expected.shape.shape);
            for index in 0..actual.len() { assert_eq!(actual[index].to_bits(), expected[index].to_bits()); }
            if axes.len() == 1 {
                let single = array.sum_axis(axes[0]).unwrap();
                for index in 0..actual.len() { assert_eq!(single[index].to_bits(), actual[index].to_bits()); }
            }
        }
    }

    #[test]
    fn buffered_axis_projection_preserves_storage_order_and_empty_dimensions() {
        for order in [StorageOrder::RowMajor, StorageOrder::ColumnMajor] {
            assert_projection(Shape::<3>::new_with_order([3, 4, 2], order));
            assert_projection(DynShape::<crate::concept::Vec>::new_with_order(vec![3, 4, 2], order));
            assert_projection(DynShape::<crate::concept::Vec>::new_with_order(vec![3, 0, 2], order));
        }
        assert_projection(Shape::<3, crate::RowMajor>::new_with_order([3, 4, 2], crate::RowMajor));
        assert_projection(Shape::<3, crate::ColumnMajor>::new_with_order([3, 4, 2], crate::ColumnMajor));
        assert_projection(CustomShape(Shape::new([3, 4, 2])));
        let empty = MultiArray::<f64, DynShape>::new_with(DynShape::new(vec![]), 0.0);
        assert_eq!(empty.sum_axes(&[]).unwrap().len(), 1);
        assert!(empty.sum_axis(0).is_err());
        assert!(matches!(empty.sum_axes(&[1, 0]), Err(SumError::AxisOutOfBounds { axis: 1, max_axis: 0 })));
    }

    struct CustomShape(Shape<3>);

    impl AbstractShape for CustomShape {
        const DIMENSION: usize = 3;
        type StorageOrder = StorageOrder;
        type VectorType = <Shape<3> as AbstractShape>::VectorType;
        type DummyVectorType = <Shape<3> as AbstractShape>::DummyVectorType;
        type MapVectorType = <Shape<3> as AbstractShape>::MapVectorType;
        type ShapeVectorType = <Shape<3> as AbstractShape>::ShapeVectorType;
        type IteratorVectorType = <Shape<3> as AbstractShape>::IteratorVectorType;
        type ShapeWithStorageOrder<SO: crate::StorageOrderTrait> = Shape<3, SO>;

        fn zero(&self) -> Self::VectorType { self.0.zero() }
        fn len(&self) -> usize { self.0.len() }
        fn shape(&self) -> &impl crate::Vector { self.0.shape() }
        fn offsets(&self) -> &impl crate::Vector { self.0.offsets() }
        fn storage_order(&self) -> StorageOrder { self.0.storage_order() }
        fn from_shape_vector(shape: &Self::ShapeVectorType) -> Self { Self(Shape::new(*shape)) }
        fn from_shape_vector_with_order<SO: crate::StorageOrderTrait>(shape: &Self::ShapeVectorType, order: SO) -> Shape<3, SO> {
            Shape::new_with_order(*shape, order)
        }
        fn with_storage_order<SO: crate::StorageOrderTrait>(&self, order: SO) -> Shape<3, SO> { self.0.with_storage_order(order) }
        fn dummy_to_map_vector(dummy: &Self::DummyVectorType) -> Self::MapVectorType { Shape::<3>::dummy_to_map_vector(dummy) }
        fn map_to_dummy_vector(map: &Self::MapVectorType) -> Self::DummyVectorType { Shape::<3>::map_to_dummy_vector(map) }
        fn dummy_to_iterator_vector(&self, dummy: &Self::DummyVectorType) -> Self::IteratorVectorType { self.0.dummy_to_iterator_vector(dummy) }
        fn map_to_iterator_vector(&self, map: &Self::MapVectorType) -> Self::IteratorVectorType { self.0.map_to_iterator_vector(map) }
        fn vector_of(&self, index: usize) -> Result<Self::VectorType, crate::IndexCalculationError> {
            let mut vector = self.0.vector_of(index)?;
            vector[0] = 2 - vector[0];
            Ok(vector)
        }
    }

    #[test]
    fn reducing_all_dimensions_preserves_dynamic_scalar_result() {
        let array = MultiArray::<f64, _>::new_with(Shape::<2>::new([2, 3]), 1.0);
        let result = array.sum_axes(&[0, 1]).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], 6.0);
        assert_eq!(reference_sum_axes(&array, &[0, 1])[0], result[0]);
        let array = MultiArray::<f64, _>::new_with(Shape::<2>::new([2, 0]), 1.0);
        let result = array.sum_axes(&[0, 1, 0]).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], 0.0);
    }

    #[test]
    fn axis_reduction_borrows_symbolic_values_and_reuses_dynamic_coordinate_storage() {
        struct Trace {
            values: Vec<usize>,
            input: bool,
        }
        impl Clone for Trace {
            fn clone(&self) -> Self {
                assert!(!self.input, "source elements must only be borrowed");
                Self { values: self.values.clone(), input: false }
            }
        }
        impl std::ops::Add for Trace {
            type Output = Self;
            fn add(mut self, other: Self) -> Self { self.values.extend(other.values); self }
        }
        impl Zero for Trace {
            fn zero() -> Self { Self { values: Vec::new(), input: false } }
            fn is_zero(&self) -> bool { self.values.is_empty() }
        }
        impl AddAssign<&Trace> for Trace {
            fn add_assign(&mut self, other: &Trace) { self.values.extend_from_slice(&other.values); }
        }
        let shape: DynShape = DynShape::new_with_order(vec![2, 3, 2], StorageOrder::ColumnMajor);
        let mut coordinates = shape.zero();
        let pointer = coordinates.as_ptr();
        let capacity = coordinates.capacity();
        for index in 0..shape.len() {
            shape.vector_of_into(index, &mut coordinates).unwrap();
            assert_eq!(coordinates, shape.vector_of(index).unwrap());
            assert_eq!(coordinates.as_ptr(), pointer);
            assert_eq!(coordinates.capacity(), capacity);
        }
        let array = MultiArrayBuilder::new_by(shape, |index, _| Trace { values: vec![index], input: true });
        let result = array.sum_axis(1).unwrap();
        assert_eq!(result[0].values, vec![0, 2, 4]);
        assert_eq!(result[1].values, vec![6, 8, 10]);
        assert_eq!(result[2].values, vec![1, 3, 5]);
        assert_eq!(result[3].values, vec![7, 9, 11]);
    }

    #[test]
    #[ignore = "手动 release 性能测量 / Manual release performance measurement"]
    fn buffered_projection_timing() {
        use std::hint::black_box;
        use std::time::Instant;
        for order in [StorageOrder::RowMajor, StorageOrder::ColumnMajor] {
            let array: MultiArray<f64, DynShape> = MultiArrayBuilder::new_by(DynShape::new_with_order(vec![32, 33, 16], order), |index, _| index as f64);
            for axes in [vec![1], vec![0, 2]] {
                for sample in 0..5 {
                    let start = Instant::now();
                    for _ in 0..20 { black_box(reference_sum_axes(black_box(&array), &axes)); }
                    let before = start.elapsed();
                    let start = Instant::now();
                    for _ in 0..20 { black_box(array.sum_axes(black_box(&axes)).unwrap()); }
                    println!("axis projection order={order:?} axes={axes:?} sample={sample} reference={before:?} buffered={:?}", start.elapsed());
                }
            }
        }
    }

    #[test]
    fn test_sum_all() {
        let shape: Shape<2, StorageOrder> = Shape::new([2, 3]);
        let array: MultiArray<f64, _> = MultiArrayBuilder::new_by(shape, |idx, _| idx as f64 + 1.0);

        // 1 + 2 + 3 + 4 + 5 + 6 = 21
        let sum = array.sum_all();
        assert!((sum - 21.0).abs() < 1e-10);
    }

    #[test]
    fn test_sum_all_empty() {
        let shape: Shape<0, StorageOrder> = Shape::new([0usize; 0]);
        let array: MultiArray<f64, _> = MultiArrayBuilder::new_with(shape, 0.0);

        let sum = array.sum_all();
        assert_eq!(sum, 0.0);
    }

    #[test]
    fn test_sum_axis() {
        // 创建 2x3 数组：
        // Create 2x3 array:
        // [[1, 2, 3],
        //  [4, 5, 6]]
        let shape: Shape<2, StorageOrder> = Shape::new([2, 3]);
        let array: MultiArray<f64, _> = MultiArrayBuilder::new_by(shape, |idx, _| idx as f64 + 1.0);

        // 沿轴 0 求和：[1+4, 2+5, 3+6] = [5, 7, 9]
        // Sum along axis 0: [1+4, 2+5, 3+6] = [5, 7, 9]
        let sum_0 = array.sum_axis(0).unwrap();
        assert_eq!(sum_0.len(), 3);
        assert!((sum_0[0] - 5.0).abs() < 1e-10);
        assert!((sum_0[1] - 7.0).abs() < 1e-10);
        assert!((sum_0[2] - 9.0).abs() < 1e-10);

        // 沿轴 1 求和：[1+2+3, 4+5+6] = [6, 15]
        // Sum along axis 1: [1+2+3, 4+5+6] = [6, 15]
        let sum_1 = array.sum_axis(1).unwrap();
        assert_eq!(sum_1.len(), 2);
        assert!((sum_1[0] - 6.0).abs() < 1e-10);
        assert!((sum_1[1] - 15.0).abs() < 1e-10);
    }

    #[test]
    fn test_sum_axis_out_of_bounds() {
        let shape: Shape<2, StorageOrder> = Shape::new([2, 3]);
        let array: MultiArray<f64, _> = MultiArrayBuilder::new_with(shape, 1.0);

        let result = array.sum_axis(2);
        assert!(matches!(result, Err(SumError::AxisOutOfBounds { .. })));
    }

    #[test]
    fn test_sum_axes() {
        // 创建 2x3x2 数组
        // Create 2x3x2 array
        let shape: Shape<3, StorageOrder> = Shape::new([2, 3, 2]);
        let array: MultiArray<f64, _> = MultiArrayBuilder::new_by(shape, |idx, _| (idx + 1) as f64);

        // 沿轴 [0, 2] 求和，结果形状为 [3]
        // Sum along axes [0, 2], result shape is [3]
        let sum = array.sum_axes(&[0, 2]).unwrap();
        assert_eq!(sum.len(), 3);
    }

    #[test]
    fn test_cumsum_axis() {
        // 创建 2x3 数组：
        // Create 2x3 array:
        // [[1, 2, 3],
        //  [4, 5, 6]]
        let shape: Shape<2, StorageOrder> = Shape::new([2, 3]);
        let array: MultiArray<f64, _> = MultiArrayBuilder::new_by(shape, |idx, _| idx as f64 + 1.0);

        // 沿轴 1 累积求和：
        // Cumulative sum along axis 1:
        // [[1, 1+2, 1+2+3], [4, 4+5, 4+5+6]]
        // = [[1, 3, 6], [4, 9, 15]]
        let cumsum = array.cumsum_axis(1).unwrap();

        assert_eq!(cumsum.len(), 6);
        assert!((cumsum[&vec![0, 0]] - 1.0).abs() < 1e-10);
        assert!((cumsum[&vec![0, 1]] - 3.0).abs() < 1e-10);
        assert!((cumsum[&vec![0, 2]] - 6.0).abs() < 1e-10);
        assert!((cumsum[&vec![1, 0]] - 4.0).abs() < 1e-10);
        assert!((cumsum[&vec![1, 1]] - 9.0).abs() < 1e-10);
        assert!((cumsum[&vec![1, 2]] - 15.0).abs() < 1e-10);
    }
}
