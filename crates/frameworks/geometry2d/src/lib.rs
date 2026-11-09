//! 共享二维量纲几何 / Shared 2D quantity geometry
//!
//! 本 crate 是 BPP2D 装箱与 CSP2D 套裁共用的二维几何内核：量值算术、点/向量、装载形状、
//! 放置、包围盒重叠判定、板材几何与刀缝。
//! This crate is the shared 2D geometry kernel for BPP2D packing and CSP2D cutting: quantity
//! arithmetic, points/vectors, packing shapes, placements, bounding-box overlap, sheet geometry and kerf.
//!
//! # 与求解器无关 / Solver independent
//!
//! 只依赖 `ospf-rust-base`、`ospf-rust-math` 与 `ospf-rust-quantities`，不引入任何建模或求解依赖。
//! Depends only on `ospf-rust-base`, `ospf-rust-math` and `ospf-rust-quantities`; no modelling or solver
//! dependency is introduced.
//!
//! # 面积口径 / Area convention
//!
//! 矩形 `area = width × height`；圆形 `area = πr²`，`bounding_area = 4r²`。目标函数与 KPI 使用 `area`，
//! 网格生成使用包围尺寸。
//! Rectangles use `area = width × height`; circles use `area = πr²` with `bounding_area = 4r²`.
//! Objectives and KPI use `area`, while grid generation uses the bounding size.
//!
//! # 重叠语义 / Overlap semantics
//!
//! 重叠统一为**包围盒内部重叠**：包围盒贴边不算重叠。圆使用外接正方形，
//! 不是精确圆盘碰撞；斜向分离或相切圆盘的外接正方形仍可能重叠。
//! Overlap means **interior bounding-box overlap**, excluding shared box edges. Circles use
//! enclosing squares, not exact disk collision; diagonally separated or tangent disks may still
//! have overlapping enclosing squares.

pub mod box_geometry;
pub mod extensions;
pub mod kerf;
pub mod placement;
pub mod point;
pub mod quantity_core;
pub mod shape;
pub mod sheet;

pub use box_geometry::Box2Geometry;
pub use extensions::{
    area_of, bounding_area, bounding_height, bounding_width, inside_sheet, max_x, max_y, min_x, min_y,
};
pub use kerf::{inflated_by_placement, inflated_by_shape, pitch_of, zero_spacing_of};
pub use placement::Placement2;
pub use point::{QuantityPoint2, QuantityVector2, point2_of, vector2_of};
pub use quantity_core::{
    DimensionMismatch, count_fitting, half_value, is_same_dimension, pi_value, quantity_equal,
    quantity_greater, quantity_greater_or_equal, quantity_is_non_positive, quantity_is_positive,
    quantity_less, quantity_less_or_equal, quantity_max_of, quantity_max_safe, quantity_min_safe,
    quantity_minus_safe, quantity_multiply, quantity_of, quantity_ord_or_null, quantity_ord_safe,
    quantity_plus_safe, quantity_scaled_by, quantity_sum, quantity_zero_of, scaled_quantity,
    sorted_by_descending_quantity, sorted_by_quantity, sum_of_quantity, two_value,
};
pub use shape::{
    CirclePackingShape2, PackingShape2, PackingShape2Type, RectanglePackingShape2, Shape2Candidate,
    candidate_shapes, circle_shape2, rectangle_shape2,
};
pub use sheet::{Sheet2Geometry, Sheet2Shape, origin_box, sheet2_shape};
