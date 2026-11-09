use super::*;

fn q(value: f64, unit: QuantityUnit) -> PhysicalQuantity { PhysicalQuantity { value, unit } }
fn point(x: f64, y: f64) -> Geometry2dPoint { Geometry2dPoint { x: q(x, QuantityUnit::Meter), y: q(y, QuantityUnit::Meter) } }
fn rectangle(width: f64, height: f64) -> Geometry2dShape { Geometry2dShape::Rectangle { width: q(width, QuantityUnit::Meter), height: q(height, QuantityUnit::Meter) } }
fn bounds(x: f64, y: f64) -> Geometry2dBox { Geometry2dBox { shape: rectangle(2.0, 3.0), position: point(x, y) } }

#[test]
fn mixed_units_shapes_rotations_and_squared_areas_reuse_original_kernel() {
    let api = Geometry2d::new(QuantityUnit::Centimeter).unwrap();
    let shape = Geometry2dShape::Rectangle { width: q(2000.0, QuantityUnit::Millimeter), height: q(300.0, QuantityUnit::Centimeter) };
    let metrics = api.shape_metrics(&shape).unwrap();
    assert_eq!(metrics.width, q(200.0, QuantityUnit::Centimeter));
    assert!((metrics.area.value - 60000.0).abs() < 1e-9); assert_eq!(metrics.area.unit, QuantityUnit::SquareCentimeter);
    assert_eq!(metrics.area, metrics.bounding_area);
    let candidates = api.candidate_shapes(&shape, true).unwrap(); assert_eq!(candidates.len(), 2); assert!(candidates[1].rotated);
    assert_eq!(api.shape_metrics(&candidates[1].shape).unwrap().width, metrics.height);
    assert_eq!(api.rotated(&api.rotated(&shape).unwrap()).unwrap(), api.shape(&shape).unwrap());
    assert_eq!(api.candidate_shapes(&rectangle(2.0, 2.0), true).unwrap().len(), 1);
    let circle = Geometry2dShape::Circle { radius: q(2.0, QuantityUnit::Meter) };
    let metrics = api.shape_metrics(&circle).unwrap();
    assert!((metrics.area.value - std::f64::consts::PI * 40000.0).abs() < 1e-9);
    assert!((metrics.bounding_area.value - 160000.0).abs() < 1e-9);
    assert_eq!(api.candidate_shapes(&circle, true).unwrap().len(), 1);
    let area = metrics.area.quantity().unwrap().to_unit(&QuantityUnit::SquareMeter.unit()).unwrap();
    assert!((area.value - 4.0 * std::f64::consts::PI).abs() < 1e-12);
    assert!(metrics.area.quantity().unwrap().to_unit(&QuantityUnit::Meter.unit()).is_err());
}

#[test]
fn points_vectors_bounds_and_closed_containment_match_original_calls() {
    let api = Geometry2d::new(QuantityUnit::Meter).unwrap();
    let offset = Geometry2dPoint { x: q(100.0, QuantityUnit::Centimeter), y: q(-2000.0, QuantityUnit::Millimeter) };
    assert_eq!(api.add_point(&point(2.0, 3.0), &offset).unwrap(), point(3.0, 1.0));
    assert_eq!(api.sub_point(&point(2.0, 3.0), &offset).unwrap(), point(1.0, 5.0));
    assert_eq!(api.add_vectors(&point(2.0, 3.0), &offset).unwrap(), point(3.0, 1.0));
    assert_eq!(api.sub_vectors(&point(2.0, 3.0), &offset).unwrap(), point(1.0, 5.0));
    assert_eq!(api.compare_points(&point(100.0, 0.0), &point(-100.0, 1.0)).unwrap(), -1);
    assert_eq!(api.compare_points(&point(2.0, 1.0), &point(1.0, 1.0)).unwrap(), 1);
    let first = bounds(-1.0, -2.0);
    assert_eq!(api.box_metrics(&first).unwrap(), Geometry2dBoxMetrics { min: point(-1.0, -2.0), max: point(1.0, 1.0) });
    assert!(api.contains(&first, &point(1.0, 1.0)).unwrap()); assert!(!api.contains(&first, &point(1.01, 1.0)).unwrap());
    assert!(!api.overlapped(&first, &bounds(1.0, -2.0)).unwrap()); assert!(api.overlapped(&first, &bounds(0.5, -2.0)).unwrap());
    assert_eq!(api.translated(&first, &point(1.0, 2.0)).unwrap(), bounds(0.0, 0.0));
    assert_eq!(api.at(&first, &point(0.0, 0.0)).unwrap(), bounds(0.0, 0.0));
    let sheet = Geometry2dSheet { width: q(2.0, QuantityUnit::Meter), height: q(3.0, QuantityUnit::Meter) };
    assert_eq!(api.origin_box(&sheet).unwrap(), bounds(0.0, 0.0));
    assert!(api.inside_sheet(&bounds(0.0, 0.0), &sheet).unwrap()); assert!(!api.inside_sheet(&first, &sheet).unwrap());
}

#[test]
fn circle_predicates_are_explicitly_bounding_not_exact_disk_tests() {
    let api = Geometry2d::new(QuantityUnit::Meter).unwrap();
    let circle = Geometry2dShape::Circle { radius: q(1.0, QuantityUnit::Meter) };
    let first = Geometry2dBox { shape: circle.clone(), position: point(0.0, 0.0) };
    let diagonal = Geometry2dBox { shape: circle, position: point(1.5, 1.5) };
    // 圆盘不相交但外接正方形相交；桥接不得冒充精确圆盘碰撞算法。
    // The disks are disjoint but their squares overlap; the bridge must not pretend to be an exact disk collision algorithm.
    assert!(api.overlapped(&first, &diagonal).unwrap()); assert!(api.contains(&first, &point(0.0, 0.0)).unwrap());
    assert_eq!(api.overlapped(&first, &diagonal).unwrap(), api.read_box(&first).unwrap().overlapped(&api.read_box(&diagonal).unwrap()));
}

#[test]
fn kerf_preserves_original_origin_and_rotation_is_metadata_not_an_extra_transform() {
    let api = Geometry2d::new(QuantityUnit::Meter).unwrap();
    let placement = Geometry2dPlacement { shape: rectangle(2.0, 3.0), position: point(-2.0, 1.0), rotated: true, item_id: Some("part".into()) };
    let output = api.inflated_placement(&placement, &q(50.0, QuantityUnit::Centimeter)).unwrap();
    assert_eq!(output.position, placement.position); assert_eq!(output.shape, rectangle(2.5, 3.5));
    assert!(output.rotated); assert_eq!(output.item_id, placement.item_id);
    assert_eq!(api.pitch(&placement.shape, &q(0.5, QuantityUnit::Meter)).unwrap(), point(2.5, 3.5));
    assert_eq!(api.placement(&placement).unwrap(), placement);
    assert_eq!(api.inflated_shape(&placement.shape, &q(0.0, QuantityUnit::Meter)).unwrap(), placement.shape);
}

#[test]
fn invalid_dimensions_values_shapes_and_bounded_batches_fail_before_comparison() {
    let api = Geometry2d::new(QuantityUnit::Meter).unwrap();
    for unit in [QuantityUnit::Second, QuantityUnit::Kilogram, QuantityUnit::SquareMeter] { assert!(Geometry2d::new(unit).is_err()); }
    for value in [0.0, -1.0, f64::INFINITY, f64::NAN, 1e200, 1e-200] { assert!(api.shape(&rectangle(value, value)).is_err()); }
    let bad_point = Geometry2dPoint { x: q(1.0, QuantityUnit::Second), y: q(1.0, QuantityUnit::Meter) };
    assert!(api.point(&bad_point).is_err()); assert!(api.contains(&bounds(0.0, 0.0), &bad_point).is_err());
    for spacing in [q(-1.0, QuantityUnit::Meter), q(1.0, QuantityUnit::SquareMeter), q(f64::INFINITY, QuantityUnit::Meter)] {
        assert!(api.inflated_shape(&rectangle(1.0, 1.0), &spacing).is_err());
    }
    assert!(api.bounds(&bounds(1e200, 0.0)).is_err());
    let underflow = Geometry2dPoint { x: q(f64::from_bits(1), QuantityUnit::Millimeter), y: q(0.0, QuantityUnit::Meter) };
    assert!(api.point(&underflow).is_err());
    let kilometers = Geometry2d::new(QuantityUnit::Kilometer).unwrap();
    assert!(kilometers.shape_metrics(&rectangle(1e-160, 1e-160)).is_err());
    assert!(serde_json::from_str::<Geometry2dShape>(r#"{"kind":"rectangle","width":{"value":1,"unit":"m"},"height":{"value":1,"unit":"m"},"radius":{"value":1,"unit":"m"}}"#).is_err());
    let values = vec![bounds(0.0, 0.0), bounds(2.0, 0.0), bounds(1.0, 1.0)];
    assert_eq!(api.overlap_pairs(&values).unwrap(), vec![0, 2, 1, 2]);
    assert_eq!(api.overlap_pairs(&vec![bounds(0.0, 0.0); 513]).unwrap_err().code, "PayloadTooLarge");
    assert!(api.overlap_pairs(&[bounds(f64::NAN, 0.0)]).is_err());
}
