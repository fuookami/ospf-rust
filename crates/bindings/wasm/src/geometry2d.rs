//! 共享二维几何 JSON/TypedArray 边界，不导出原泛型类型。
//! Shared 2D geometry JSON/TypedArray boundary, without exporting original generic types.

use serde::{Serialize, de::DeserializeOwned};
use wasm_bindgen::prelude::*;
use ospf_rust_bridge::{Geometry2d as Geometry, BridgeError};
use crate::{bridge_error, error};

fn parse<T: DeserializeOwned>(text: &str) -> std::result::Result<T, JsValue> {
    if text.len() > 8 * 1024 * 1024 { return Err(bridge_error(BridgeError::new("PayloadTooLarge", "geometry2d text exceeds 8 MiB"))); }
    serde_json::from_str(text).map_err(error)
}

fn json<T: Serialize>(value: ospf_rust_bridge::Result<T>) -> std::result::Result<String, JsValue> {
    serde_json::to_string(&value.map_err(bridge_error)?).map_err(error)
}

/// 独立归一化上下文，返回数据不借用其生命周期。 / Independent normalization context; returned data never borrows its lifetime.
#[wasm_bindgen]
pub struct Geometry2d { value: Geometry }

#[wasm_bindgen]
impl Geometry2d {
    /// 选择输出长度单位；内部复用原 Meter 泛型实现。 / Select the output length unit; internally reuse the original Meter specialization.
    #[wasm_bindgen(constructor)]
    pub fn new(unit: &str) -> std::result::Result<Geometry2d, JsValue> {
        let unit = serde_json::from_value(serde_json::Value::String(unit.into())).map_err(error)?;
        Ok(Self { value: Geometry::new(unit).map_err(bridge_error)? })
    }

    /// 输出单位的生成契约值。 / Generated-contract value of the output unit.
    #[wasm_bindgen(js_name = lengthUnitJson)]
    pub fn length_unit_json(&self) -> std::result::Result<String, JsValue> { json(Ok(self.value.length_unit())) }

    /// 归一化坐标。 / Normalize coordinates.
    pub fn point(&self, value: &str) -> std::result::Result<String, JsValue> { json(self.value.point(&parse(value)?)) }

    /// 点加向量。 / Add a vector to a point.
    #[wasm_bindgen(js_name = addPoint)]
    pub fn add_point(&self, point: &str, offset: &str) -> std::result::Result<String, JsValue> { json(self.value.add_point(&parse(point)?, &parse(offset)?)) }

    /// 点减向量。 / Subtract a vector from a point.
    #[wasm_bindgen(js_name = subPoint)]
    pub fn sub_point(&self, point: &str, offset: &str) -> std::result::Result<String, JsValue> { json(self.value.sub_point(&parse(point)?, &parse(offset)?)) }

    /// 向量相加。 / Add vectors.
    #[wasm_bindgen(js_name = addVectors)]
    pub fn add_vectors(&self, first: &str, second: &str) -> std::result::Result<String, JsValue> { json(self.value.add_vectors(&parse(first)?, &parse(second)?)) }

    /// 向量相减。 / Subtract vectors.
    #[wasm_bindgen(js_name = subVectors)]
    pub fn sub_vectors(&self, first: &str, second: &str) -> std::result::Result<String, JsValue> { json(self.value.sub_vectors(&parse(first)?, &parse(second)?)) }

    /// 按 y 再 x 比较。 / Compare by y, then x.
    #[wasm_bindgen(js_name = comparePoints)]
    pub fn compare_points(&self, first: &str, second: &str) -> std::result::Result<i32, JsValue> { self.value.compare_points(&parse(first)?, &parse(second)?).map_err(bridge_error) }

    /// 归一化形状。 / Normalize a shape.
    pub fn shape(&self, value: &str) -> std::result::Result<String, JsValue> { json(self.value.shape(&parse(value)?)) }

    /// 旋转 90 度。 / Rotate by 90 degrees.
    pub fn rotated(&self, value: &str) -> std::result::Result<String, JsValue> { json(self.value.rotated(&parse(value)?)) }

    /// 原候选方向。 / Original orientation candidates.
    #[wasm_bindgen(js_name = candidateShapes)]
    pub fn candidate_shapes(&self, value: &str, allow_rotate: bool) -> std::result::Result<String, JsValue> { json(self.value.candidate_shapes(&parse(value)?, allow_rotate)) }

    /// 原形状刀缝膨胀。 / Original shape kerf inflation.
    #[wasm_bindgen(js_name = inflatedByShape)]
    pub fn inflated_by_shape(&self, value: &str, spacing: &str) -> std::result::Result<String, JsValue> { json(self.value.inflated_shape(&parse(value)?, &parse(spacing)?)) }

    /// 原网格节距。 / Original grid pitch.
    #[wasm_bindgen(js_name = pitchOf)]
    pub fn pitch_of(&self, value: &str, spacing: &str) -> std::result::Result<String, JsValue> { json(self.value.pitch(&parse(value)?, &parse(spacing)?)) }

    /// 实际与包围面积。 / Actual and bounding areas.
    #[wasm_bindgen(js_name = shapeMetrics)]
    pub fn shape_metrics(&self, value: &str) -> std::result::Result<String, JsValue> { json(self.value.shape_metrics(&parse(value)?)) }

    /// 归一化包围盒。 / Normalize bounding geometry.
    pub fn bounds(&self, value: &str) -> std::result::Result<String, JsValue> { json(self.value.bounds(&parse(value)?)) }

    /// 最小/最大边界。 / Minimum/maximum boundaries.
    #[wasm_bindgen(js_name = boxMetrics)]
    pub fn box_metrics(&self, value: &str) -> std::result::Result<String, JsValue> { json(self.value.box_metrics(&parse(value)?)) }

    /// 包围盒闭区间点包含。 / Closed bounding-box point containment.
    pub fn contains(&self, bounds: &str, point: &str) -> std::result::Result<bool, JsValue> { self.value.contains(&parse(bounds)?, &parse(point)?).map_err(bridge_error) }

    /// 包围盒包含。 / Bounding-box containment.
    #[wasm_bindgen(js_name = containsBox)]
    pub fn contains_box(&self, first: &str, second: &str) -> std::result::Result<bool, JsValue> { self.value.contains_box(&parse(first)?, &parse(second)?).map_err(bridge_error) }

    /// 包围盒内部重叠，不是精确圆盘碰撞。 / Interior bounding-box overlap, not exact disk collision.
    pub fn overlapped(&self, first: &str, second: &str) -> std::result::Result<bool, JsValue> { self.value.overlapped(&parse(first)?, &parse(second)?).map_err(bridge_error) }

    /// 精确圆盘相交；任一侧非圆返回 null，不按外接方冒充。 / Exact disk intersection; null when either side is not a circle instead of faking it with the enclosing square.
    #[wasm_bindgen(js_name = diskOverlaps)]
    pub fn disk_overlaps(&self, first: &str, second: &str) -> std::result::Result<Option<bool>, JsValue> {
        self.value.disk_overlaps(&parse(first)?, &parse(second)?).map_err(bridge_error)
    }

    /// 沿一个方向能容纳多少个单元；非正尺寸与量纲不符按原内核返回 0，不报错。
    /// How many cells fit along one direction; non-positive sizes and mismatched dimensions return 0 as the original
    /// kernel does, without failing.
    #[wasm_bindgen(js_name = countFitting)]
    pub fn count_fitting(&self, cell: &str, container: &str) -> std::result::Result<u32, JsValue> {
        self.value.count_fitting(&parse(cell)?, &parse(container)?).map_err(bridge_error)
    }

    /// 圆心，仅对圆形盒有效；非圆返回 null。 / Circle centre, valid only for circular boxes; null otherwise.
    pub fn centre(&self, value: &str) -> std::result::Result<Option<String>, JsValue> {
        match self.value.centre(&parse(value)?).map_err(bridge_error)? { Some(point) => Ok(Some(serde_json::to_string(&point).map_err(error)?)), None => Ok(None) }
    }

    /// 向量平移。 / Vector translation.
    pub fn translated(&self, bounds: &str, offset: &str) -> std::result::Result<String, JsValue> { json(self.value.translated(&parse(bounds)?, &parse(offset)?)) }

    /// 移至指定位置。 / Move to a specified position.
    pub fn at(&self, bounds: &str, position: &str) -> std::result::Result<String, JsValue> { json(self.value.at(&parse(bounds)?, &parse(position)?)) }

    /// 原点板材包围盒。 / Origin-anchored sheet bounding box.
    #[wasm_bindgen(js_name = originBox)]
    pub fn origin_box(&self, sheet: &str) -> std::result::Result<String, JsValue> { json(self.value.origin_box(&parse(sheet)?)) }

    /// 板材包含，贴边合法。 / Sheet containment allowing boundary contact.
    #[wasm_bindgen(js_name = insideSheet)]
    pub fn inside_sheet(&self, bounds: &str, sheet: &str) -> std::result::Result<bool, JsValue> { self.value.inside_sheet(&parse(bounds)?, &parse(sheet)?).map_err(bridge_error) }

    /// 归一化放置。 / Normalize a placement.
    pub fn placement(&self, value: &str) -> std::result::Result<String, JsValue> { json(self.value.placement(&parse(value)?)) }

    /// 保持原左下角的放置膨胀。 / Placement inflation preserving the original lower-left corner.
    #[wasm_bindgen(js_name = inflatedByPlacement)]
    pub fn inflated_by_placement(&self, value: &str, spacing: &str) -> std::result::Result<String, JsValue> { json(self.value.inflated_placement(&parse(value)?, &parse(spacing)?)) }

    /// 交错成对索引的复制 TypedArray。 / Copied TypedArray of interleaved pair indices.
    #[wasm_bindgen(js_name = overlapPairs)]
    pub fn overlap_pairs(&self, values: &str) -> std::result::Result<Vec<u32>, JsValue> { self.value.overlap_pairs(&parse::<Vec<_>>(values)?).map_err(bridge_error) }
}

#[cfg(all(test, target_arch = "wasm32"))]
mod tests {
    use super::*;
    use wasm_bindgen_test::wasm_bindgen_test;

    #[wasm_bindgen_test]
    fn geometry_calls_use_original_kernel_without_native_clocks_and_return_independent_data() {
        let geometry = Geometry2d::new("cm").unwrap();
        let shape = r#"{"kind":"circle","radius":{"value":10,"unit":"mm"}}"#;
        let metrics = geometry.shape_metrics(shape).unwrap();
        let bounds = format!(r#"{{"shape":{shape},"position":{{"x":{{"value":0,"unit":"m"}},"y":{{"value":0,"unit":"m"}}}}}}"#);
        assert!(geometry.overlapped(&bounds, &bounds).unwrap());
        assert_eq!(geometry.overlap_pairs(&format!("[{bounds},{bounds}]")).unwrap(), vec![0, 1]);
        let oversized = geometry.point(&" ".repeat(8 * 1024 * 1024 + 1)).unwrap_err();
        assert!(oversized.as_string().unwrap().contains("PayloadTooLarge"));
        assert!(Geometry2d::new("kg").is_err()); drop(geometry);
        let metrics: ospf_rust_bridge::Geometry2dShapeMetrics = serde_json::from_str(&metrics).unwrap();
        assert_eq!(metrics.area.unit, ospf_rust_bridge::QuantityUnit::SquareCentimeter);
        assert!((metrics.area.value - std::f64::consts::PI).abs() < 1e-12);
    }
}
