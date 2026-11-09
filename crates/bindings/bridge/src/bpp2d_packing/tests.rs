use super::*;
use crate::bpp2d::tests::{layer, meters, problem};
use crate::{Bpp2dDemand, encode_bpp2d_problem, typescript_declarations};

#[test]
fn compressed_packing_reuses_original_order_remnants_renderer_and_exact_material_totals() {
    let source = encode_bpp2d_problem(problem()).unwrap();
    let mut physical = layer(2);
    physical.placements.reverse();
    let packing = pack_bpp2d_layers(&source, &[physical], &[Bpp2dUsage { column_index: 0, count: "9007199254740991".into() }]).unwrap();
    assert_eq!(packing.sheet_count, "9007199254740991");
    assert_eq!(packing.item_count, "18014398509481982");
    assert_eq!(packing.material_counts["a"], "18014398509481982");
    assert_eq!(packing.patterns.len(), 1);
    let pattern = &packing.patterns[0];
    assert_eq!(pattern.render.placements.iter().map(|placement| placement.loading_order).collect::<Vec<_>>(), vec![1, 0]);
    assert_eq!(pattern.render.placements[0].placement, pattern.layer.placements[0]);
    assert_eq!(pattern.render.loading_rate, 1.0);
    assert!(pattern.remnants.is_empty());
    let packing = pack_bpp2d_layers(&source, &[layer(1)], &[Bpp2dUsage { column_index: 0, count: "1".into() }]).unwrap();
    assert_eq!(packing.occupied_area.value, 1.0);
    assert_eq!(packing.consumed_area.value, 2.0);
    assert_eq!(packing.remnant_area.value, 1.0);
    assert_eq!(packing.patterns[0].remnants[0].position.x, meters(1.0));
    // 物理副本不足源需求仍可返回，不冒充生产验收。 / A physical copy may be below source demand without masquerading as production acceptance.
    assert_eq!(packing.item_count, "1");
    assert!(typescript_declarations().contains("materialCounts: { [key in string]?: string }"));
}

#[test]
fn circles_keep_actual_area_and_conservative_bounding_square_remnants() {
    let mut problem = problem();
    problem.items[0].shape = Geometry2dShape::Circle { radius: meters(0.5) };
    problem.initial_layers[0].placements[0].shape = problem.items[0].shape.clone();
    let columns = problem.initial_layers.clone();
    let source = encode_bpp2d_problem(problem).unwrap();
    let packed = pack_bpp2d_layers(&source, &columns, &[Bpp2dUsage { column_index: 0, count: "2".into() }]).unwrap();
    assert!((packed.occupied_area.value - std::f64::consts::PI * 0.5).abs() < 1e-12);
    assert_eq!(packed.remnant_area.value, 2.0);
    assert_ne!(packed.consumed_area.value - packed.occupied_area.value, packed.remnant_area.value);
    assert_eq!(packed.patterns[0].render.placements[0].placement.shape, Geometry2dShape::Circle { radius: meters(0.5) });
}

#[test]
fn tiny_distinct_origins_keep_unique_loading_ranks_without_coordinate_quantization() {
    let mut problem = problem();
    problem.items[0].shape = Geometry2dShape::Rectangle { width: meters(1e-7), height: meters(1e-7) };
    problem.sheets[0].geometry = Geometry2dSheet { width: meters(2e-7), height: meters(1e-7) };
    problem.initial_layers.clear();
    let mut columns = vec![layer(2)];
    columns[0].envelope = problem.sheets[0].geometry.clone();
    for (index, placement) in columns[0].placements.iter_mut().enumerate() {
        placement.shape = problem.items[0].shape.clone();
        placement.position.x = meters(index as f64 * 1e-7);
    }
    let packed = pack_bpp2d_layers(&encode_bpp2d_problem(problem).unwrap(), &columns, &[Bpp2dUsage { column_index: 0, count: "1".into() }]).unwrap();
    assert_eq!(packed.patterns[0].render.placements.iter().map(|item| item.loading_order).collect::<Vec<_>>(), vec![0, 1]);
}

#[test]
fn packing_rejects_invalid_source_geometry_usage_and_resource_limits_without_partial_outputs() {
    let source = encode_bpp2d_problem(problem()).unwrap();
    for usages in [vec![Bpp2dUsage { column_index: 1, count: "1".into() }], vec![Bpp2dUsage { column_index: 0, count: "0".into() }],
        vec![Bpp2dUsage { column_index: 0, count: "01".into() }], vec![Bpp2dUsage { column_index: 0, count: "9007199254740992".into() }],
        vec![Bpp2dUsage { column_index: 0, count: "1".into() }, Bpp2dUsage { column_index: 0, count: "1".into() }]] {
        assert!(pack_bpp2d_layers(&source, &[layer(1)], &usages).is_err());
    }
    let mut foreign = source.clone(); foreign.fingerprint = "foreign".into();
    assert_eq!(pack_bpp2d_layers(&foreign, &[], &[]).unwrap_err().code, "FingerprintMismatch");
    let mut columns = vec![layer(1)]; columns[0].placements[0].item_id = Some("foreign".into());
    assert!(pack_bpp2d_layers(&source, &columns, &[]).is_err());
    let mut problem = problem(); problem.initial_layers.clear();
    problem.sheets[0].geometry.width = meters(129.0);
    problem.demands = vec![Bpp2dDemand::Count { item_id: "a".into(), count: "1".into(), upper_bound: None }];
    let source = encode_bpp2d_problem(problem).unwrap();
    assert_eq!(pack_bpp2d_layers(&source, &[layer(129)], &[Bpp2dUsage { column_index: 0, count: "1".into() }]).unwrap_err().code, "PayloadTooLarge");
    assert_eq!(pack_bpp2d_layers(&source, &[layer(128), layer(128)], &[Bpp2dUsage { column_index: 0, count: "1".into() },
        Bpp2dUsage { column_index: 1, count: "1".into() }]).unwrap_err().code, "PayloadTooLarge");
    assert_eq!(pack_bpp2d_layers(&source, &[], &[]).unwrap().sheet_count, "0");
}
