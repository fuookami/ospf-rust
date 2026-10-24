use super::*;
use ospf_rust_quantities::unit::derived::Meter;
use ospf_rust_framework_geometry2d::{QuantityPoint2, circle_shape2, rectangle_shape2};

fn meters(value: f64) -> Quantity<f64, Meter> { Quantity::new_ct(value) }

fn placement(x: f64, y: f64, circle: bool) -> Placement2<f64, Meter> {
    let shape = if circle { PackingShape2::Circle(circle_shape2(meters(0.5)).unwrap()) }
        else { PackingShape2::Rectangle(rectangle_shape2(meters(1.0), meters(1.0)).unwrap()) };
    Placement2::new(shape, QuantityPoint2 { x: meters(x), y: meters(y) })
}

fn check(placements: &[Placement2<f64, Meter>], kerf: Option<&Quantity<f64, Meter>>) {
    assert_eq!(PackingGeometryGuard::first_overlap(placements, kerf),
        PackingGeometryGuard::first_overlap_pairs(placements, kerf));
    let sheet = Sheet2Shape::new(meters(200.0), meters(200.0));
    for rule in [PackingRotationRule::AllowAll, PackingRotationRule::SymmetricOnly] {
        let expected = PackingGeometryGuard::require_sheet_geometry_with_rule_reference(
            &sheet, placements, "s", "test", kerf, &rule).map_err(|error| error.to_string());
        let actual = PackingGeometryGuard::require_sheet_geometry_with_rule(
            &sheet, placements, "s", "test", kerf, &rule).map_err(|error| error.to_string());
        assert_eq!(actual, expected);
    }
}

#[test]
fn overlap_sweep_preserves_exact_pairs_kerf_and_error_priority() {
    let mut state = 731946u64;
    for case in 0..160 {
        let mut placements = Vec::new();
        for index in 0..case % 65 {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
            let x = (state >> 32) as f64 / u32::MAX as f64 * 20.0;
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
            let y = (state >> 32) as f64 / u32::MAX as f64 * 20.0;
            placements.push(placement(x, y, index % 3 == 0));
        }
        for kerf in [None, Some(meters(0.0)), Some(meters(-1.0)), Some(meters(0.125)), Some(meters(f64::INFINITY))] {
            check(&placements, kerf.as_ref());
        }
    }
    let mut placements = (0..40).map(|i| placement(i as f64 * 3.0, 0.0, i % 2 == 0)).collect::<Vec<_>>();
    placements[35].position.x.value = placements[36].position.x.value;
    placements[1].position.x.value = placements[5].position.x.value;
    check(&placements, None);
    for gap in [0.0, 0.125, 0.125 - f64::EPSILON, 0.125 + f64::EPSILON] {
        placements[1].position.x.value = 1.0 + gap;
        check(&placements, Some(&meters(0.125)));
    }
    placements[39].position.x.value = -1.0;
    check(&placements, None);
    placements[39].position.x.value = f64::NAN;
    check(&placements, None);
    placements[39].position.x.value = 100.0;
    if let PackingShape2::Rectangle(shape) = &mut placements[1].shape { shape.width.value = -1.0; }
    check(&placements, None);
}

#[test]
fn overlap_sweep_keeps_float32_and_rotation_error_precedence() {
    let mut placements = (0..40).map(|index| Placement2::new(
        PackingShape2::Rectangle(rectangle_shape2(Quantity::<f32, Meter>::new_ct(2.0), Quantity::new_ct(1.0)).unwrap()),
        QuantityPoint2 { x: Quantity::new_ct(index as f32 * 3.0), y: Quantity::new_ct(0.0) },
    )).collect::<Vec<_>>();
    let sheet = Sheet2Shape::new(Quantity::new_ct(200.0), Quantity::new_ct(20.0));
    for step in [0.0, 0.125, 0.125 - f32::EPSILON, 0.125 + f32::EPSILON] {
        placements[1].position.x.value = 2.0 + step;
        let kerf = Some(Quantity::new_ct(0.125));
        assert_eq!(PackingGeometryGuard::first_overlap(&placements, kerf.as_ref()),
            PackingGeometryGuard::first_overlap_pairs(&placements, kerf.as_ref()));
        for rotated in [false, true] {
            placements[39].rotated = rotated;
            let expected = PackingGeometryGuard::require_sheet_geometry_with_rule_reference(
                &sheet, &placements, "s", "test", kerf.as_ref(), &PackingRotationRule::SymmetricOnly)
                .map_err(|error| error.to_string());
            let actual = PackingGeometryGuard::require_sheet_geometry_with_rule(
                &sheet, &placements, "s", "test", kerf.as_ref(), &PackingRotationRule::SymmetricOnly)
                .map_err(|error| error.to_string());
            assert_eq!(actual, expected);
        }
    }
}

#[test]
#[ignore = "manual release timing; run with --ignored --nocapture --test-threads=1"]
fn performance_b11_geometry_guard() {
    for count in [32, 128, 512] {
        let placements = (0..count).map(|i| placement(i as f64 * 2.0, 0.0, i % 3 == 0)).collect::<Vec<_>>();
        let sheet = Sheet2Shape::new(meters(count as f64 * 2.0), meters(10.0));
        for sample in 0..5 {
            let start = std::time::Instant::now();
            let result = PackingGeometryGuard::require_sheet_geometry(&sheet,
                std::hint::black_box(&placements), "s", "test", Some(&meters(0.125)));
            let elapsed = start.elapsed();
            result.unwrap();
            eprintln!("geometry_guard count={count} sample={sample} elapsed={elapsed:?}");
        }
    }
}

impl PackingGeometryGuard {
    fn require_sheet_geometry_with_rule_reference<V, U>(
        sheet: &Sheet2Shape<V, U>,
        placements: &[Placement2<V, U>],
        sheet_name: &str,
        source: &str,
        kerf: Option<&Quantity<V, U>>,
        rotation_rule: &PackingRotationRule,
    ) -> Ret<()>
    where
        V: ospf_rust_math::algebra::concept::Field + Clone + PartialOrd + num_traits::Float,
        U: UnitTrait + CTUnit + Clone,
    {
        let sheet_box = origin_box(sheet.width.clone(), sheet.height.clone());
        let mixed_diagnostic = mixed_shape_diagnostic(placements);

        for (index, placement) in placements.iter().enumerate() {
            let placement_box =
                Box2Geometry::new(placement.x(), placement.y(), placement.shape.clone());
            if !inside_sheet(&placement_box, &sheet_box) {
                return Err(validation_error(format!(
                    "{} 在 {} 的第 {} 个放置越出板材（{}）/ placement {} falls outside sheet {} ({}); {}",
                    source,
                    sheet_name,
                    index,
                    mixed_diagnostic,
                    index,
                    sheet_name,
                    placement_shape_diagnostic(placement),
                    mixed_diagnostic
                )));
            }
            if !rotation_rule.allow(placement) {
                return Err(validation_error(format!(
                    "{} 在 {} 的第 {} 个放置使用了未允许的旋转（{}）/ placement {} in sheet {} uses a rotation the item does not allow ({})",
                    source,
                    sheet_name,
                    index,
                    placement_shape_diagnostic(placement),
                    index,
                    sheet_name,
                    placement_shape_diagnostic(placement)
                )));
            }
        }

        for lhs_index in 0..placements.len() {
            for rhs_index in (lhs_index + 1)..placements.len() {
                if Self::overlapped_with_kerf(
                    &placements[lhs_index],
                    &placements[rhs_index],
                    kerf,
                ) {
                    return Err(validation_error(format!(
                        "{} 在 {} 的第 {} 与第 {} 个放置重叠（{}；{}）/ placements {} and {} in sheet {} overlap ({}; {})",
                        source,
                        sheet_name,
                        lhs_index,
                        rhs_index,
                        placement_shape_diagnostic(&placements[lhs_index]),
                        placement_shape_diagnostic(&placements[rhs_index]),
                        lhs_index,
                        rhs_index,
                        sheet_name,
                        placement_shape_diagnostic(&placements[lhs_index]),
                        placement_shape_diagnostic(&placements[rhs_index])
                    )));
                }
            }
        }
        Ok(())
    }

}
