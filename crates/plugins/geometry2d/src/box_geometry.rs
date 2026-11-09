//! 二维包围盒几何 / 2D bounding box geometry
//!
//! 在量纲包围盒之上提供角点、包含与相交能力。圆形件以外接正方形表示，圆心位于该正方形中心，
//! 因此圆×圆、圆×矩形、矩形×矩形的重叠判定都走同一套包围盒逻辑。
//! Adds corners, containment and intersection on top of a quantity bounding box. A circle is represented by
//! its enclosing square with the circle centre at its centre, so circle-circle, circle-rectangle and
//! rectangle-rectangle overlap tests all share one bounding-box implementation.
//!
//! # 严格不等式 / Strict inequalities
//!
//! 重叠统一为**包围盒内部重叠**：包围盒贴边不算重叠。圆使用外接正方形，
//! 因此不能据此判断斜向分离或相切圆盘的精确碰撞；包含同样按包围盒闭区间。
//! Overlap means **interior bounding-box overlap**: shared box edges do not count. Circles use
//! enclosing squares, so this is not exact collision for diagonally separated or tangent disks;
//! containment likewise uses the closed bounding box.

use ospf_rust_base::error::Ret;
use ospf_rust_math::algebra::concept::Field;
use ospf_rust_quantities::quantity::Quantity;
use ospf_rust_quantities::unit::concept::UnitTrait;
use ospf_rust_quantities::unit::physical_unit::CTUnit;

use crate::point::{QuantityPoint2, QuantityVector2};
use crate::quantity_core::{half_value, quantity_minus_safe, quantity_ord_or_null, quantity_plus_safe, quantity_scaled_by};
use crate::shape::PackingShape2;
use std::cmp::Ordering;
use std::ops::Add;

// ============================================================================
// Box2Geometry - 二维包围盒 / 2D bounding box
// ============================================================================

/// 二维包围盒 / 2D bounding box
///
/// 用于装载形状之间的重叠与包含判定。
/// Used for overlap and containment tests between packing shapes.
#[derive(Debug, Clone)]
pub struct Box2Geometry<V, U: UnitTrait> {
    /// 左下角 x 坐标 / Lower-left x coordinate
    pub x: Quantity<V, U>,
    /// 左下角 y 坐标 / Lower-left y coordinate
    pub y: Quantity<V, U>,
    /// 包围形状 / Bounding shape
    pub shape: PackingShape2<V, U>,
}

impl<V, U> Box2Geometry<V, U>
where
    V: Clone + PartialOrd + Add<Output = V>,
    U: UnitTrait,
{
    /// 已归一化矩形的内部重叠，无 Float 或 solver 转换要求。
    /// Interior overlap of normalized rectangles, without Float or solver conversion requirements.
    ///
    /// 非矩形、单位未统一或不可比较时返回 None；调用方先验证有限性和正尺寸。
    /// Returns None for nonrectangles, inconsistent units or incomparable values; callers first validate finite positive extents.
    pub fn rectangle_overlapped(&self, other: &Self) -> Option<bool> {
        let (PackingShape2::Rectangle(first), PackingShape2::Rectangle(second)) = (&self.shape, &other.shape) else { return None; };
        let quantities = [&self.x, &self.y, &first.width, &first.height, &other.x, &other.y, &second.width, &second.height];
        let unit = &self.x.unit;
        if quantities.iter().any(|quantity| quantity.unit.dimension_symbol() != unit.dimension_symbol()
            || quantity.unit.scale_value() != unit.scale_value()) { return None; }
        let first_max_x = self.x.value.clone() + first.width.value.clone();
        let first_max_y = self.y.value.clone() + first.height.value.clone();
        let second_max_x = other.x.value.clone() + second.width.value.clone();
        let second_max_y = other.y.value.clone() + second.height.value.clone();
        Some(first_max_x.partial_cmp(&other.x.value)? == Ordering::Greater
            && second_max_x.partial_cmp(&self.x.value)? == Ordering::Greater
            && first_max_y.partial_cmp(&other.y.value)? == Ordering::Greater
            && second_max_y.partial_cmp(&self.y.value)? == Ordering::Greater)
    }
}

impl<V, U> Box2Geometry<V, U>
where
    U: UnitTrait,
{
    /// 由角点与形状构造 / Build a box from an origin and a shape
    pub fn new(x: Quantity<V, U>, y: Quantity<V, U>, shape: PackingShape2<V, U>) -> Self {
        Self { x, y, shape }
    }

    /// 由角点与尺寸构造矩形包围盒 / Build a rectangle box from an origin and a size
    pub fn of_rectangle(
        x: Quantity<V, U>,
        y: Quantity<V, U>,
        width: Quantity<V, U>,
        height: Quantity<V, U>,
    ) -> Self {
        Self::new(
            x,
            y,
            PackingShape2::Rectangle(crate::shape::RectanglePackingShape2 { width, height }),
        )
    }
}

impl<V, U> Box2Geometry<V, U>
where
    V: Field + Clone + PartialOrd + num_traits::Float,
    U: UnitTrait + CTUnit + Clone,
{

    /// 包围宽度 / Bounding width
    pub fn width(&self) -> Quantity<V, U> {
        self.shape.bounding_width()
    }

    /// 包围高度 / Bounding height
    pub fn height(&self) -> Quantity<V, U> {
        self.shape.bounding_height()
    }

    /// 左下角 / Lower-left corner
    pub fn min(&self) -> QuantityPoint2<V, U> {
        QuantityPoint2 {
            x: self.x.clone(),
            y: self.y.clone(),
        }
    }

    /// 右上角 / Top-right corner
    pub fn max(&self) -> Ret<QuantityPoint2<V, U>> {
        let x = quantity_plus_safe(&self.x, &self.width())?;
        let y = quantity_plus_safe(&self.y, &self.height())?;
        Ok(QuantityPoint2 { x, y })
    }

    /// 是否包含另一点 / Whether the box contains a point
    ///
    /// 使用闭区间：边界上的点属于该盒，与「相切不算重叠」并不冲突——包含与重叠是不同问题。
    /// Uses closed intervals: a point on the boundary belongs to the box. This does not conflict with
    /// "tangency is not overlap", since containment and overlap are different questions.
    pub fn contains(&self, point: &QuantityPoint2<V, U>) -> bool {
        let (Ok(max_x), Ok(max_y)) = (self.max_x(), self.max_y()) else {
            return false;
        };
        matches!(
            quantity_ord_or_null(&point.x, &self.x),
            Some(Ordering::Greater) | Some(Ordering::Equal)
        ) && matches!(
            quantity_ord_or_null(&point.x, &max_x),
            Some(Ordering::Less) | Some(Ordering::Equal)
        ) && matches!(
            quantity_ord_or_null(&point.y, &self.y),
            Some(Ordering::Greater) | Some(Ordering::Equal)
        ) && matches!(
            quantity_ord_or_null(&point.y, &max_y),
            Some(Ordering::Less) | Some(Ordering::Equal)
        )
    }

    /// 是否包含另一个盒 / Whether the box contains another box
    pub fn contains_box(&self, other: &Self) -> bool {
        let (Ok(other_max_x), Ok(other_max_y)) = (other.max_x(), other.max_y()) else {
            return false;
        };
        let (Ok(self_max_x), Ok(self_max_y)) = (self.max_x(), self.max_y()) else {
            return false;
        };
        matches!(
            quantity_ord_or_null(&other.x, &self.x),
            Some(Ordering::Greater) | Some(Ordering::Equal)
        ) && matches!(
            quantity_ord_or_null(&other.y, &self.y),
            Some(Ordering::Greater) | Some(Ordering::Equal)
        ) && matches!(
            quantity_ord_or_null(&other_max_x, &self_max_x),
            Some(Ordering::Less) | Some(Ordering::Equal)
        ) && matches!(
            quantity_ord_or_null(&other_max_y, &self_max_y),
            Some(Ordering::Less) | Some(Ordering::Equal)
        )
    }

    /// 是否内部重叠 / Whether the boxes overlap in their interiors
    ///
    /// 包围盒任一侧边贴合**不算**重叠。圆使用外接正方形，不是精确圆盘相交。
    /// Shared bounding-box edges do **not** overlap. Circles use enclosing squares, not exact disk intersection.
    pub fn overlapped(&self, other: &Self) -> bool {
        let (Ok(self_max_x), Ok(self_max_y)) = (self.max_x(), self.max_y()) else {
            return false;
        };
        let (Ok(other_max_x), Ok(other_max_y)) = (other.max_x(), other.max_y()) else {
            return false;
        };
        let strictly_greater_left = matches!(
            quantity_ord_or_null(&self_max_x, &other.x),
            Some(Ordering::Greater)
        );
        let strictly_greater_right = matches!(
            quantity_ord_or_null(&other_max_x, &self.x),
            Some(Ordering::Greater)
        );
        let strictly_greater_bottom = matches!(
            quantity_ord_or_null(&self_max_y, &other.y),
            Some(Ordering::Greater)
        );
        let strictly_greater_top = matches!(
            quantity_ord_or_null(&other_max_y, &self.y),
            Some(Ordering::Greater)
        );
        strictly_greater_left && strictly_greater_right && strictly_greater_bottom && strictly_greater_top
    }

    /// 精确圆盘相交 / Exact disk intersection
    ///
    /// 圆心位于包围盒中心，半径为形状半径；判定为圆心距 < 半径之和，相切不算相交。
    /// 两个形状都必须含圆半径，否则返回 None——矩形没有精确圆盘语义，不能用外接方阵冒充。
    /// The centre is the bounding-box centre and the radius is the shape radius; intersection means the
    /// centre distance is less than the sum of radii, and tangency does not intersect. Returns None unless
    /// both shapes carry a circle radius: a rectangle has no exact disk semantics and must not be faked
    /// with its enclosing square.
    pub fn disk_overlaps(&self, other: &Self) -> Option<bool> {
        let self_radius = self.shape.circle()?.radius.clone();
        let other_radius = other.shape.circle()?.radius.clone();
        let self_centre = self.centre()?;
        let other_centre = other.centre()?;
        let dx = quantity_minus_safe(&self_centre.x, &other_centre.x).ok()?;
        let dy = quantity_minus_safe(&self_centre.y, &other_centre.y).ok()?;
        let distance_squared = dx.value.clone() * dx.value.clone() + dy.value.clone() * dy.value.clone();
        let radii = quantity_plus_safe(&self_radius, &other_radius).ok()?;
        // 平方比较避免开方误差：distance² < (r1 + r2)² 等价于 distance < r1 + r2，两边均非负。
        // Compare squares to avoid a square-root rounding step: distance² < (r1 + r2)² is equivalent
        // to distance < r1 + r2 because both sides are non-negative.
        let limit_squared = radii.value.clone() * radii.value.clone();
        Some(distance_squared.partial_cmp(&limit_squared) == Some(Ordering::Less))
    }

    /// 圆心 / Circle centre
    ///
    /// 圆以外接正方形表示，因此圆心是包围盒中心；非圆形状返回 None。
    /// A circle is represented by its enclosing square, so the centre is the bounding-box centre;
    /// non-circle shapes return None.
    pub fn centre(&self) -> Option<QuantityPoint2<V, U>> {
        self.shape.circle()?;
        let max_x = self.max_x().ok()?;
        let max_y = self.max_y().ok()?;
        let half = half_value::<V>();
        let x = quantity_scaled_by(&quantity_plus_safe(&self.x, &max_x).ok()?, &half);
        let y = quantity_scaled_by(&quantity_plus_safe(&self.y, &max_y).ok()?, &half);
        Some(QuantityPoint2 { x, y })
    }

    /// 是否与任意其他盒重叠 / Whether the box overlaps any of the given boxes
    pub fn overlaps_any(&self, others: &[Self]) -> bool {
        others.iter().any(|other| self.overlapped(other))
    }

    /// 按向量平移 / Translate by a vector
    pub fn translated(&self, offset: &QuantityVector2<V, U>) -> Ret<Self> {
        Ok(Self {
            x: quantity_plus_safe(&self.x, &offset.x)?,
            y: quantity_plus_safe(&self.y, &offset.y)?,
            shape: self.shape.clone(),
        })
    }

    /// 平移到指定角点 / Move to the given lower-left corner
    pub fn at(&self, x: Quantity<V, U>, y: Quantity<V, U>) -> Self {
        Self {
            x,
            y,
            shape: self.shape.clone(),
        }
    }

    /// 右边界 x 坐标 / Right boundary x coordinate
    pub fn max_x(&self) -> Ret<Quantity<V, U>> {
        quantity_plus_safe(&self.x, &self.width())
    }

    /// 上边界 y 坐标 / Upper boundary y coordinate
    pub fn max_y(&self) -> Ret<Quantity<V, U>> {
        quantity_plus_safe(&self.y, &self.height())
    }

    /// 宽度差 / Width difference
    pub fn width_delta(&self, other: &Self) -> Ret<Quantity<V, U>> {
        quantity_minus_safe(&self.width(), &other.width())
    }
}
