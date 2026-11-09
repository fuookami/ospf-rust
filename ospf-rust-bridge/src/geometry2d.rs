//! 共享二维几何的有限 f64/单位边界，计算委托原内核。
//! Finite f64/unit boundary for shared 2D geometry, delegating calculations to the original kernel.

use serde::{Deserialize, Serialize};
use ts_rs::TS;
use ospf_rust_quantities::quantity::Quantity;
use ospf_rust_quantities::unit::{CTUnit, Unit};
use ospf_rust_quantities::unit::derived::Meter;
use ospf_rust_framework_geometry2d as geometry;
use crate::{BridgeError, PhysicalQuantity, QuantityUnit, Result, invalid};

/// 点或向量的带单位分量。 / Unit-bearing point or vector components.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Geometry2dPoint {
    /// x 分量。 / x component.
    pub x: PhysicalQuantity,
    /// y 分量。 / y component.
    pub y: PhysicalQuantity,
}

/// 原内核支持的矩形和圆。 / Rectangles and circles supported by the original kernel.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum Geometry2dShape {
    /// 轴对齐矩形。 / Axis-aligned rectangle.
    Rectangle {
        /// 正宽度。 / Positive width.
        width: PhysicalQuantity,
        /// 正高度。 / Positive height.
        height: PhysicalQuantity,
    },
    /// 圆形，盒判定仍使用其外接正方形。 / Circle, with box predicates still using its enclosing square.
    Circle {
        /// 正半径。 / Positive radius.
        radius: PhysicalQuantity,
    },
}

/// 原点为左下角的包围盒。 / Bounding geometry anchored at its lower-left corner.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Geometry2dBox {
    /// 形状。 / Shape.
    pub shape: Geometry2dShape,
    /// 左下角。 / Lower-left corner.
    pub position: Geometry2dPoint,
}

/// 形状已体现实际朝向，rotated 仅为原领域元数据。 / Shape already carries physical orientation; rotated is original-domain metadata only.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Geometry2dPlacement {
    /// 实际形状。 / Physical shape.
    pub shape: Geometry2dShape,
    /// 左下角。 / Lower-left corner.
    pub position: Geometry2dPoint,
    /// 原旋转标记，不再次交换宽高。 / Original rotation flag, without swapping dimensions again.
    pub rotated: bool,
    /// 可选物品标识。 / Optional item identifier.
    pub item_id: Option<String>,
}

/// 原点起算的矩形板材。 / Rectangular sheet anchored at the origin.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Geometry2dSheet {
    /// 正宽度。 / Positive width.
    pub width: PhysicalQuantity,
    /// 正高度。 / Positive height.
    pub height: PhysicalQuantity,
}

/// 原内核计算的包围尺寸与实际/包围面积。 / Bounding size and actual/bounding areas computed by the original kernel.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Geometry2dShapeMetrics {
    /// 包围宽度。 / Bounding width.
    pub width: PhysicalQuantity,
    /// 包围高度。 / Bounding height.
    pub height: PhysicalQuantity,
    /// 实际面积。 / Actual area.
    pub area: PhysicalQuantity,
    /// 外接矩形面积。 / Enclosing rectangle area.
    pub bounding_area: PhysicalQuantity,
}

/// 包围盒闭区间边界。 / Closed-interval bounding-box boundaries.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Geometry2dBoxMetrics {
    /// 左下角。 / Lower-left corner.
    pub min: Geometry2dPoint,
    /// 右上角。 / Upper-right corner.
    pub max: Geometry2dPoint,
}

/// 原 candidate_shapes 返回的朝向。 / Orientation returned by original candidate_shapes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Geometry2dShapeCandidate {
    /// 实际形状。 / Physical shape.
    pub shape: Geometry2dShape,
    /// 是否旋转。 / Whether rotated.
    pub rotated: bool,
}

type Shape = geometry::PackingShape2<f64, Meter>;
type BoxGeometry = geometry::Box2Geometry<f64, Meter>;
type Point = geometry::QuantityPoint2<f64, Meter>;
type Vector = geometry::QuantityVector2<f64, Meter>;
type Placement = geometry::Placement2<f64, Meter>;

/// 显式输出单位的几何门面；内部归一为 Meter 后调用原泛型内核。
/// Geometry facade with explicit output units, normalizing to Meter before calling the original generic kernel.
#[derive(Debug, Clone, Copy)]
pub struct Geometry2d {
    length_unit: QuantityUnit,
    area_unit: QuantityUnit,
}

impl Geometry2d {
    /// 仅接受已开放的长度单位。 / Accept only exposed length units.
    pub fn new(length_unit: QuantityUnit) -> Result<Self> {
        let area_unit = match length_unit {
            QuantityUnit::Meter => QuantityUnit::SquareMeter, QuantityUnit::Kilometer => QuantityUnit::SquareKilometer,
            QuantityUnit::Centimeter => QuantityUnit::SquareCentimeter, QuantityUnit::Millimeter => QuantityUnit::SquareMillimeter,
            _ => return Err(invalid("geometry2d normalization unit must be a length unit")),
        };
        Ok(Self { length_unit, area_unit })
    }

    /// 输出长度单位。 / Output length unit.
    pub fn length_unit(&self) -> QuantityUnit { self.length_unit }

    pub(crate) fn length(&self, value: &PhysicalQuantity, positive: bool) -> Result<Quantity<f64, Meter>> {
        let converted = value.quantity()?.to_unit(&Meter::INSTANT).map_err(invalid)?;
        if !converted.value.is_finite() || (value.value != 0.0 && converted.value == 0.0) || (positive && converted.value <= 0.0) {
            return Err(invalid("geometry2d requires finite coordinates and positive extents"));
        }
        Ok(Quantity::new_ct(converted.value))
    }

    fn output(&self, value: Quantity<f64, Unit>, area: bool) -> Result<PhysicalQuantity> {
        let unit = if area { self.area_unit } else { self.length_unit };
        let converted = value.to_unit(&unit.unit()).map_err(invalid)?;
        if !converted.value.is_finite() || (value.value != 0.0 && converted.value == 0.0) || (area && converted.value <= 0.0) { return Err(invalid("geometry2d output conversion overflow or underflow")); }
        Ok(PhysicalQuantity { value: converted.value, unit })
    }

    fn read_point(&self, point: &Geometry2dPoint) -> Result<Point> {
        Ok(geometry::point2_of(self.length(&point.x, false)?, self.length(&point.y, false)?))
    }

    fn write_point(&self, point: Point) -> Result<Geometry2dPoint> {
        Ok(Geometry2dPoint { x: self.output(point.x.to_runtime(), false)?, y: self.output(point.y.to_runtime(), false)? })
    }

    pub(crate) fn read_shape(&self, shape: &Geometry2dShape) -> Result<Shape> {
        let shape = match shape {
            Geometry2dShape::Rectangle { width, height } => Shape::Rectangle(geometry::rectangle_shape2(self.length(width, true)?, self.length(height, true)?).map_err(invalid)?),
            Geometry2dShape::Circle { radius } => Shape::Circle(geometry::circle_shape2(self.length(radius, true)?).map_err(invalid)?),
        };
        // 防止溢出或下溢的面积/直径进入返回值及后续比较。
        // Keep overflowed/underflowed areas and diameters out of outputs and subsequent comparisons.
        for value in [shape.bounding_width().value, shape.bounding_height().value, shape.area().value, shape.bounding_area().value] {
            if !value.is_finite() || value <= 0.0 { return Err(invalid("geometry2d shape arithmetic overflow or underflow")); }
        }
        Ok(shape)
    }

    fn write_shape(&self, shape: Shape) -> Result<Geometry2dShape> {
        let shape = match shape {
            Shape::Rectangle(rectangle) => Geometry2dShape::Rectangle { width: self.output(rectangle.width.to_runtime(), false)?, height: self.output(rectangle.height.to_runtime(), false)? },
            Shape::Circle(circle) => Geometry2dShape::Circle { radius: self.output(circle.radius.to_runtime(), false)? },
        };
        self.read_shape(&shape)?;
        Ok(shape)
    }

    fn read_box(&self, value: &Geometry2dBox) -> Result<BoxGeometry> {
        let point = self.read_point(&value.position)?;
        let result = BoxGeometry::new(point.x, point.y, self.read_shape(&value.shape)?);
        let max = result.max().map_err(invalid)?;
        if !max.x.value.is_finite() || !max.y.value.is_finite() || max.x.value <= result.x.value || max.y.value <= result.y.value {
            return Err(invalid("geometry2d bounds overflow or collapse at coordinate precision"));
        }
        Ok(result)
    }

    pub(crate) fn write_box(&self, value: BoxGeometry) -> Result<Geometry2dBox> {
        let result = Geometry2dBox { position: self.write_point(value.min())?, shape: self.write_shape(value.shape)? };
        self.read_box(&result)?;
        Ok(result)
    }

    fn spacing(&self, value: &PhysicalQuantity) -> Result<Quantity<f64, Meter>> {
        let value = self.length(value, false)?;
        if value.value < 0.0 { return Err(invalid("geometry2d spacing must be nonnegative")); }
        Ok(value)
    }

    pub(crate) fn read_placement(&self, value: &Geometry2dPlacement) -> Result<Placement> {
        if value.item_id.as_ref().is_some_and(|id| id.is_empty() || id.len() > 1024) { return Err(invalid("geometry2d itemId must contain 1..1024 UTF-8 bytes")); }
        let bounds = self.read_box(&Geometry2dBox { shape: value.shape.clone(), position: value.position.clone() })?;
        Ok(Placement::new(bounds.shape, geometry::point2_of(bounds.x, bounds.y)).with_rotated(value.rotated).with_item_id_ref(value.item_id.clone()))
    }

    pub(crate) fn write_placement(&self, value: Placement) -> Result<Geometry2dPlacement> {
        let result = Geometry2dPlacement { shape: self.write_shape(value.shape)?, position: self.write_point(value.position)?, rotated: value.rotated, item_id: value.item_id };
        self.read_placement(&result)?;
        Ok(result)
    }

    /// 归一化坐标；输出为独立数据。 / Normalize coordinates into independent output data.
    pub fn point(&self, value: &Geometry2dPoint) -> Result<Geometry2dPoint> { self.write_point(self.read_point(value)?) }

    /// 点加向量，复用原 plus。 / Add a vector to a point using original plus.
    pub fn add_point(&self, point: &Geometry2dPoint, offset: &Geometry2dPoint) -> Result<Geometry2dPoint> {
        let offset = self.read_point(offset)?;
        self.write_point(self.read_point(point)?.plus(&geometry::vector2_of(offset.x, offset.y)).map_err(invalid)?)
    }

    /// 点减向量，复用原 minus。 / Subtract a vector from a point using original minus.
    pub fn sub_point(&self, point: &Geometry2dPoint, offset: &Geometry2dPoint) -> Result<Geometry2dPoint> {
        let offset = self.read_point(offset)?;
        self.write_point(self.read_point(point)?.minus(&geometry::vector2_of(offset.x, offset.y)).map_err(invalid)?)
    }

    /// 向量运算复用原向量类型。 / Vector arithmetic reuses the original vector type.
    pub fn add_vectors(&self, first: &Geometry2dPoint, second: &Geometry2dPoint) -> Result<Geometry2dPoint> {
        let first = self.read_point(first)?; let second = self.read_point(second)?;
        let sum: Vector = geometry::vector2_of(first.x, first.y).plus(&geometry::vector2_of(second.x, second.y)).map_err(invalid)?;
        self.write_point(geometry::point2_of(sum.x, sum.y))
    }

    /// 向量减法复用原 minus。 / Vector subtraction reuses original minus.
    pub fn sub_vectors(&self, first: &Geometry2dPoint, second: &Geometry2dPoint) -> Result<Geometry2dPoint> {
        let first = self.read_point(first)?; let second = self.read_point(second)?;
        let difference = geometry::vector2_of(first.x, first.y).minus(&geometry::vector2_of(second.x, second.y)).map_err(invalid)?;
        self.write_point(geometry::point2_of(difference.x, difference.y))
    }

    /// 按 y 再 x 排序。 / Order by y, then x.
    pub fn compare_points(&self, first: &Geometry2dPoint, second: &Geometry2dPoint) -> Result<i32> {
        Ok(match self.read_point(first)?.ord_or_null(&self.read_point(second)?).ok_or_else(|| invalid("unordered geometry2d point"))? {
            std::cmp::Ordering::Less => -1, std::cmp::Ordering::Equal => 0, std::cmp::Ordering::Greater => 1,
        })
    }

    /// 归一化合法形状。 / Normalize a valid shape.
    pub fn shape(&self, value: &Geometry2dShape) -> Result<Geometry2dShape> { self.write_shape(self.read_shape(value)?) }

    /// 原生 90 度旋转。 / Original 90-degree rotation.
    pub fn rotated(&self, value: &Geometry2dShape) -> Result<Geometry2dShape> { self.write_shape(self.read_shape(value)?.rotated()) }

    /// 原候选朝向，正方形及圆不重复。 / Original orientation candidates, without duplicate squares or circles.
    pub fn candidate_shapes(&self, value: &Geometry2dShape, allow_rotate: bool) -> Result<Vec<Geometry2dShapeCandidate>> {
        geometry::candidate_shapes(&self.read_shape(value)?, allow_rotate).into_iter().map(|candidate| Ok(Geometry2dShapeCandidate {
            shape: self.write_shape(candidate.shape)?, rotated: candidate.rotated,
        })).collect()
    }

    /// 原形状膨胀，尺寸各增加 spacing。 / Original shape inflation, adding spacing to each bounding dimension.
    pub fn inflated_shape(&self, value: &Geometry2dShape, spacing: &PhysicalQuantity) -> Result<Geometry2dShape> {
        self.write_shape(geometry::inflated_by_shape(&self.read_shape(value)?, &self.spacing(spacing)?))
    }

    /// 包围尺寸加刀缝。 / Bounding size plus kerf.
    pub fn pitch(&self, value: &Geometry2dShape, spacing: &PhysicalQuantity) -> Result<Geometry2dPoint> {
        let (x, y) = geometry::pitch_of(&self.read_shape(value)?, &self.spacing(spacing)?);
        self.write_point(geometry::point2_of(x, y))
    }

    /// 面积保持平方长度量纲。 / Areas retain squared-length dimensions.
    pub fn shape_metrics(&self, value: &Geometry2dShape) -> Result<Geometry2dShapeMetrics> {
        let shape = self.read_shape(value)?;
        Ok(Geometry2dShapeMetrics { width: self.output(shape.bounding_width().to_runtime(), false)?, height: self.output(shape.bounding_height().to_runtime(), false)?,
            area: self.output(shape.area(), true)?, bounding_area: self.output(shape.bounding_area(), true)? })
    }

    /// 沿一个方向能容纳多少个单元：单元尺寸非正或量纲不符时按原内核返回 0，不报错。
    /// How many cells fit along one direction; the original kernel returns 0 for non-positive cell sizes or
    /// mismatched dimensions instead of failing.
    pub fn count_fitting(&self, cell: &PhysicalQuantity, container: &PhysicalQuantity) -> Result<u32> {
        let cell = self.length(cell, false)?;
        let container = self.length(container, false)?;
        u32::try_from(ospf_rust_framework_geometry2d::quantity_core::count_fitting(&cell, &container))
            .map_err(|_| invalid("count fitting exceeds the u32 range"))
    }

    /// 归一化包围盒。 / Normalize bounding geometry.
    pub fn bounds(&self, value: &Geometry2dBox) -> Result<Geometry2dBox> { self.write_box(self.read_box(value)?) }

    /// 左下与右上边界。 / Lower-left and upper-right boundaries.
    pub fn box_metrics(&self, value: &Geometry2dBox) -> Result<Geometry2dBoxMetrics> {
        let value = self.read_box(value)?;
        Ok(Geometry2dBoxMetrics { min: self.write_point(value.min())?, max: self.write_point(value.max().map_err(invalid)?)? })
    }

    /// 包围盒闭区间包含；圆按外接正方形，不是精确圆盘。 / Closed box containment; circles use enclosing squares, not exact disks.
    pub fn contains(&self, bounds: &Geometry2dBox, point: &Geometry2dPoint) -> Result<bool> { Ok(self.read_box(bounds)?.contains(&self.read_point(point)?)) }

    /// 原包围盒包含。 / Original bounding-box containment.
    pub fn contains_box(&self, first: &Geometry2dBox, second: &Geometry2dBox) -> Result<bool> { Ok(self.read_box(first)?.contains_box(&self.read_box(second)?)) }

    /// 原包围盒内部重叠；贴边不重叠。 / Original interior bounding-box overlap; shared edges do not overlap.
    pub fn overlapped(&self, first: &Geometry2dBox, second: &Geometry2dBox) -> Result<bool> { Ok(self.read_box(first)?.overlapped(&self.read_box(second)?)) }

    /// 精确圆盘相交：圆心距小于半径之和才相交，相切不算。任一侧非圆返回 None。
    /// Exact disk intersection: the centre distance must be less than the sum of radii, and tangency does
    /// not intersect. Returns None when either side is not a circle.
    pub fn disk_overlaps(&self, first: &Geometry2dBox, second: &Geometry2dBox) -> Result<Option<bool>> {
        Ok(self.read_box(first)?.disk_overlaps(&self.read_box(second)?))
    }

    /// 圆心，仅对圆形盒有效 / Circle centre, only for circular boxes.
    pub fn centre(&self, value: &Geometry2dBox) -> Result<Option<Geometry2dPoint>> {
        let Some(centre) = self.read_box(value)?.centre() else { return Ok(None) };
        Ok(Some(self.write_point(centre)?))
    }

    /// 原向量平移。 / Original vector translation.
    pub fn translated(&self, bounds: &Geometry2dBox, offset: &Geometry2dPoint) -> Result<Geometry2dBox> {
        let offset = self.read_point(offset)?;
        self.write_box(self.read_box(bounds)?.translated(&geometry::vector2_of(offset.x, offset.y)).map_err(invalid)?)
    }

    /// 移至指定左下角。 / Move to a specified lower-left corner.
    pub fn at(&self, bounds: &Geometry2dBox, position: &Geometry2dPoint) -> Result<Geometry2dBox> {
        let position = self.read_point(position)?;
        self.write_box(self.read_box(bounds)?.at(position.x, position.y))
    }

    /// 板材从原点构造，复用原 sheet2_shape。 / Build an origin-anchored sheet using original sheet2_shape.
    pub fn origin_box(&self, sheet: &Geometry2dSheet) -> Result<Geometry2dBox> {
        self.write_box(geometry::sheet2_shape(self.length(&sheet.width, true)?, self.length(&sheet.height, true)?).map_err(invalid)?.origin_box())
    }

    /// 原板材包含，边界贴合合法。 / Original sheet containment, allowing boundary contact.
    pub fn inside_sheet(&self, bounds: &Geometry2dBox, sheet: &Geometry2dSheet) -> Result<bool> {
        let sheet = geometry::sheet2_shape(self.length(&sheet.width, true)?, self.length(&sheet.height, true)?).map_err(invalid)?;
        let sheet = sheet.origin_box();
        // 即使只调用谓词，也不允许无效板材面积被静默忽略。
        // Predicate-only calls must not silently ignore an invalid sheet area.
        self.write_box(sheet.clone())?;
        Ok(geometry::inside_sheet(&self.read_box(bounds)?, &sheet))
    }

    /// 归一化放置及原元数据。 / Normalize a placement and its original metadata.
    pub fn placement(&self, value: &Geometry2dPlacement) -> Result<Geometry2dPlacement> { self.write_placement(self.read_placement(value)?) }

    /// 原实现保持左下角不变；不是中心不变的膨胀。 / Original implementation preserves the lower-left corner, not centred inflation.
    pub fn inflated_placement(&self, value: &Geometry2dPlacement, spacing: &PhysicalQuantity) -> Result<Geometry2dPlacement> {
        self.write_placement(geometry::inflated_by_placement(&self.read_placement(value)?, &self.spacing(spacing)?))
    }

    /// 全部包围盒重叠对，返回交错的 u32 索引，最多 512 个输入盒。
    /// All bounding-box overlap pairs as interleaved u32 indices, limited to 512 input boxes.
    pub fn overlap_pairs(&self, values: &[Geometry2dBox]) -> Result<Vec<u32>> {
        if values.len() > 512 { return Err(BridgeError::new("PayloadTooLarge", "geometry2d overlapPairs exceeds 512 boxes")); }
        let values = values.iter().map(|value| self.read_box(value)).collect::<Result<Vec<_>>>()?;
        let mut pairs = Vec::new();
        for (index, first) in values.iter().enumerate() {
            for (other, second) in values.iter().enumerate().skip(index + 1) {
                if first.overlapped(second) { pairs.extend([index as u32, other as u32]); }
            }
        }
        Ok(pairs)
    }
}

#[cfg(test)]
mod tests;
