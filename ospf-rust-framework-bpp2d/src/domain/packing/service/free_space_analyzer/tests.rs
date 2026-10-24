use super::*;
use ospf_rust_quantities::unit::derived::Meter;
use ospf_rust_framework_geometry2d::{PackingShape2, QuantityPoint2, circle_shape2, rectangle_shape2};
use crate::domain::item::model::SheetTypeId;

fn meters(value: f64) -> Quantity<f64, Meter> { Quantity::new_ct(value) }

fn sheet() -> SheetType<f64, Meter> {
    SheetType::new(meters(100.0), meters(100.0), meters(100.0), SheetTypeId::new("s"))
}

fn placement(x: f64, y: f64, width: f64, height: f64) -> Placement2<f64, Meter> {
    Placement2::new(PackingShape2::Rectangle(rectangle_shape2(meters(width), meters(height)).unwrap()),
        QuantityPoint2 { x: meters(x), y: meters(y) })
}

fn signature(remnants: &[Remnant2<f64, Meter>]) -> Vec<(String, [u64; 5])> {
    remnants.iter().map(|remnant| (remnant.sheet_name.clone(), [
        remnant.rectangle.x.value.to_bits(), remnant.rectangle.y.value.to_bits(),
        remnant.rectangle.width().value.to_bits(), remnant.rectangle.height().value.to_bits(),
        remnant.area.value.to_bits(),
    ])).collect()
}

#[test]
fn slice_events_preserve_reference_rectangles_order_and_tolerance() {
    let analyzer = FreeSpaceAnalyzer::new();
    let mut state = 834651u64;
    for case in 0..160 {
        let mut placements = Vec::new();
        for index in 0..case % 45 {
            let mut random = || {
                state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
                (state >> 32) as f64 / u32::MAX as f64
            };
            let x = if index % 3 == 0 { 10.0 } else { random() * 110.0 - 5.0 };
            let y = if index % 4 == 0 { 10.0 + (index % 3) as f64 * 0.5e-9 } else { random() * 110.0 - 5.0 };
            let width = random() * 10.0 + 1e-10;
            let height = if index % 5 == 0 { 1e-10 } else { random() * 10.0 };
            let mut item = placement(x, y, width, height);
            if index % 7 == 0 { item.shape = PackingShape2::Circle(circle_shape2(meters(width * 0.5)).unwrap()); }
            placements.push(item);
        }
        let expected = analyzer.analyze_sheet_reference("s", &sheet(), &placements);
        let actual = analyzer.analyze_sheet("s", &sheet(), &placements);
        assert_eq!(signature(&actual), signature(&expected), "case={case}");
    }
    for special in [f64::INFINITY, f64::NEG_INFINITY, f64::NAN, -0.0, 1e-9, 1e-9 + f64::EPSILON] {
        let mut placements = vec![placement(2.0, 3.0, 2.0, 4.0), placement(8.0, 9.0, 2.0, 3.0)];
        placements[0].position.y.value = special;
        assert_eq!(signature(&analyzer.analyze_sheet("s", &sheet(), &placements)),
            signature(&analyzer.analyze_sheet_reference("s", &sheet(), &placements)));
    }
    for dimension in [-2.0, 0.0, f64::INFINITY, f64::NAN] {
        let mut placements = vec![placement(2.0, 3.0, 2.0, 4.0), placement(8.0, 9.0, 2.0, 3.0)];
        if let PackingShape2::Rectangle(shape) = &mut placements[0].shape { shape.height.value = dimension; }
        assert_eq!(signature(&analyzer.analyze_sheet("s", &sheet(), &placements)),
            signature(&analyzer.analyze_sheet_reference("s", &sheet(), &placements)));
    }
}

#[test]
#[ignore = "manual release timing; run with --ignored --nocapture --test-threads=1"]
fn performance_b11_remnant_slices() {
    let analyzer = FreeSpaceAnalyzer::new();
    for count in [32, 128, 512] {
        let placements = (0..count).map(|i| placement((i % 7) as f64 * 10.0,
            i as f64 * 90.0 / count as f64, 5.0, 20.0 / count as f64)).collect::<Vec<_>>();
        for sample in 0..5 {
            let start = std::time::Instant::now();
            let result = analyzer.analyze_sheet("s", &sheet(), std::hint::black_box(&placements));
            let elapsed = start.elapsed();
            eprintln!("remnant_slices count={count} sample={sample} elapsed={elapsed:?} results={}", result.len());
            std::hint::black_box(result);
        }
    }
}

impl FreeSpaceAnalyzer {
    fn analyze_sheet_reference<V, U>(
        &self,
        name: &str,
        sheet: &SheetType<V, U>,
        placements: &[Placement2<V, U>],
    ) -> Vec<Remnant2<V, U>>
    where
        V: ospf_rust_math::algebra::concept::Field + Clone + PartialOrd + num_traits::Float,
        U: UnitTrait + CTUnit + Clone,
    {
        let width = sheet.width.value.to_f64().unwrap_or(0.0);
        let height = sheet.height.value.to_f64().unwrap_or(0.0);
        if width <= 0.0 || height <= 0.0 {
            return Vec::new();
        }

        if placements.is_empty() {
            let whole = origin_box(sheet.width.clone(), sheet.height.clone());
            return vec![Remnant2 {
                sheet_name: name.to_string(),
                area: whole.width().to_runtime() * whole.height().to_runtime(),
                rectangle: whole,
            }];
        }

        // 收集水平切分点：板材底、所有放置的上下边界、板材顶。
        // Collect the horizontal cut points: sheet bottom, every placement's bottom and top, sheet top.
        let mut cuts: Vec<f64> = vec![0.0, height];
        for placement in placements {
            let Some(bottom) = placement.y().value.to_f64() else {
                continue;
            };
            let Some(top) = placement.max_y().ok().and_then(|top| top.value.to_f64()) else {
                continue;
            };
            cuts.push(bottom.clamp(0.0, height));
            cuts.push(top.clamp(0.0, height));
        }
        cuts.sort_by(|lhs, rhs| lhs.partial_cmp(rhs).unwrap_or(std::cmp::Ordering::Equal));
        cuts.dedup_by(|lhs, rhs| (*lhs - *rhs).abs() < 1e-9);

        let mut remnants = Vec::new();
        for window in cuts.windows(2) {
            let bottom = window[0];
            let top = window[1];
            if top - bottom <= 1e-9 {
                continue;
            }
            // 与本水平切片相交的放置，按左边界排序后扣减其 x 区间。
            // Placements intersecting this slice, ordered by their left edge, with their x-interval subtracted.
            let mut used: Vec<(f64, f64)> = placements
                .iter()
                .filter_map(|placement| {
                    let place_bottom = placement.y().value.to_f64()?;
                    let place_top = placement.max_y().ok()?.value.to_f64()?;
                    if place_top <= bottom + 1e-9 || place_bottom >= top - 1e-9 {
                        return None;
                    }
                    let left = placement.x().value.to_f64()?;
                    let right = placement.max_x().ok()?.value.to_f64()?;
                    Some((left.clamp(0.0, width), right.clamp(0.0, width)))
                })
                .collect();
            used.sort_by(|lhs, rhs| lhs.0.partial_cmp(&rhs.0).unwrap_or(std::cmp::Ordering::Equal));

            let mut cursor = 0.0f64;
            let mut free_intervals: Vec<(f64, f64)> = Vec::new();
            for (left, right) in used {
                if left > cursor + 1e-9 {
                    free_intervals.push((cursor, left));
                }
                cursor = cursor.max(right);
            }
            if cursor < width - 1e-9 {
                free_intervals.push((cursor, width));
            }

            for (left, right) in free_intervals {
                let free_width = right - left;
                if free_width <= 1e-9 {
                    continue;
                }
                let rectangle = Box2Geometry::of_rectangle(
                    Quantity::new(V::from(left).unwrap_or_else(V::zero), sheet.width.unit.clone()),
                    Quantity::new(V::from(bottom).unwrap_or_else(V::zero), sheet.height.unit.clone()),
                    Quantity::new(V::from(free_width).unwrap_or_else(V::zero), sheet.width.unit.clone()),
                    Quantity::new(V::from(top - bottom).unwrap_or_else(V::zero), sheet.height.unit.clone()),
                );
                remnants.push(Remnant2 {
                    sheet_name: name.to_string(),
                    area: rectangle.width().to_runtime() * rectangle.height().to_runtime(),
                    rectangle,
                });
            }
        }
        // 按面积降序，方便调用方优先复用大块余料。
        // Sorted by descending area so callers can reuse the largest remnants first.
        remnants.sort_by(|lhs, rhs| {
            rhs.area
                .value
                .to_f64()
                .unwrap_or(0.0)
                .partial_cmp(&lhs.area.value.to_f64().unwrap_or(0.0))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        remnants
    }

}
