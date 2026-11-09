use super::*;

fn quantity(value: f64, unit: QuantityUnit) -> PhysicalQuantity { PhysicalQuantity { value, unit } }

fn problem() -> Bpp3dProblem {
    let meters = |value| quantity(value, QuantityUnit::Meter);
    Bpp3dProblem {
        items: vec![Bpp3dItem { id: "item".into(), name: "Item".into(), width: quantity(100.0, QuantityUnit::Centimeter),
            height: meters(1.0), depth: meters(1.0), weight: quantity(500.0, QuantityUnit::Gram), amount: "2".into(),
            orientations: vec![Bpp3dOrientation::Upright, Bpp3dOrientation::Side], shape: None }],
        bins: vec![Bpp3dBin { id: "bin-1".into(), type_code: "type".into(), width: meters(2.0), height: meters(1.0), depth: meters(1.0),
            capacity: quantity(1.0, QuantityUnit::Kilogram), is_main: true }],
        initial_layers: vec![Bpp3dLayer { id: "layer".into(), bin_id: "bin-1".into(), depth: meters(1.0), placements: vec![
            Bpp3dPlacement { item_id: "item".into(), position: Bpp3dPoint { x: meters(0.0), y: meters(0.0), z: meters(0.0) }, orientation: Bpp3dOrientation::Upright },
            Bpp3dPlacement { item_id: "item".into(), position: Bpp3dPoint { x: meters(1.0), y: meters(0.0), z: meters(0.0) }, orientation: Bpp3dOrientation::Upright },
        ] }],
        radius_component: None, package_attributes: Vec::new(),
    }
}

/// 连续半径组件必须真的进入 materialized request，且非法声明在边界被拒。
/// The continuous-radius component must really reach the materialized request, and invalid declarations must be
/// rejected at the boundary.
/// 池解码必须从源重建连续半径组件，使状态与源一致（而不是拒绝带组件的 state）。
/// Pool decoding must rebuild the continuous-radius component from the source so state matches the source instead of
/// rejecting a state that carries one.
/// 离散半径候选按原 CSV 语义展开，并驱动圆装箱层生成的首个候选。
/// Discrete radius candidates expand with the original CSV semantics and drive the first candidate used by
/// circle-packing layer generation.
/// 包属性只映射 CSV 路径实际物化的三项，且真实进入 materialized request。
/// Package attributes map only the three fields the CSV path materializes and really reach the materialized request.
#[test]
fn package_attributes_map_the_csv_subset_and_reject_invalid_entries() {
    let meters = |value| quantity(value, QuantityUnit::Meter);
    let base = || Bpp3dProblem {
        items: vec![Bpp3dItem { id: "item".into(), name: "Item".into(), width: meters(1.0), height: meters(1.0), depth: meters(1.0),
            weight: quantity(500.0, QuantityUnit::Gram), amount: "2".into(), orientations: vec![Bpp3dOrientation::Upright], shape: None }],
        bins: vec![Bpp3dBin { id: "bin".into(), type_code: "type".into(), width: meters(2.0), height: meters(1.0), depth: meters(1.0),
            capacity: quantity(2.0, QuantityUnit::Kilogram), is_main: true }],
        initial_layers: vec![Bpp3dLayer { id: "layer".into(), bin_id: "bin".into(), depth: meters(1.0), placements: vec![
            Bpp3dPlacement { item_id: "item".into(), orientation: Bpp3dOrientation::Upright,
                position: Bpp3dPoint { x: meters(0.0), y: meters(0.0), z: meters(0.0) } },
            Bpp3dPlacement { item_id: "item".into(), orientation: Bpp3dOrientation::Upright,
                position: Bpp3dPoint { x: meters(1.0), y: meters(0.0), z: meters(0.0) } }] }],
        radius_component: None, package_attributes: Vec::new(),
    };
    let attribute = |max_stack_layers: Option<u64>| Bpp3dPackageAttribute { item_id: "item".into(),
        allow_mixed_loading: None, max_stack_layers, tags: vec!["fragile".into()] };
    let mut problem = base();
    problem.package_attributes = vec![attribute(Some(2))];
    let (request, _) = problem.domain().unwrap();
    let stored = request.package_attributes.iter().find(|(id, _)| id == "item").map(|(_, value)| value).expect("attribute must reach the request");
    assert_eq!(stored.max_stack_layers, Some(2));
    assert_eq!(stored.tags, vec!["fragile".to_string()]);
    assert_eq!(stored.allow_mixed_loading, None);
    // 未声明的字段保持领域默认值，不偷偷启用其它规则。
    // Undeclared fields keep the domain defaults instead of silently enabling other rules.
    assert!(stored.extra_pair_stacking_rules.is_empty() && stored.extra_orientation_rules.is_empty());
    // 禁止混装由原领域业务规则校验记录为诊断；`domain()` 不做该步，因此不能声称它会拒绝整源。
    // Disallowing mixed loading is recorded as a diagnostic by the original business-rule validation; `domain()`
    // does not run that step, so claiming it rejects the whole source would be wrong.
    let mut problem = base();
    problem.package_attributes = vec![Bpp3dPackageAttribute { item_id: "item".into(), allow_mixed_loading: Some(false),
        max_stack_layers: None, tags: Vec::new() }];
    let (request, _) = problem.domain().unwrap();
    let diagnostics = request.validate_business_rules();
    assert!(diagnostics.iter().any(|line| line.contains("disallows mixed loading")), "{diagnostics:?}");
    // 未知物品、重复条目、零层数与空标签都必须拒绝。 / Unknown items, duplicates, zero layers and empty tags all fail.
    for invalid in [vec![Bpp3dPackageAttribute { item_id: "ghost".into(), allow_mixed_loading: None, max_stack_layers: None, tags: Vec::new() }],
        vec![attribute(None), attribute(None)], vec![attribute(Some(0))],
        vec![Bpp3dPackageAttribute { item_id: "item".into(), allow_mixed_loading: None, max_stack_layers: None, tags: vec![String::new()] }]] {
        let mut problem = base();
        problem.package_attributes = invalid;
        assert!(problem.domain().is_err());
    }
}

#[test]
fn discrete_radius_candidates_expand_and_bound_the_cross_section() {
    let meters = |value| quantity(value, QuantityUnit::Meter);
    let drum = |candidates: Option<Bpp3dRadiusCandidates>| Bpp3dProblem {
        items: vec![Bpp3dItem { id: "drum".into(), name: "Drum".into(), width: meters(1.8), height: meters(1.0), depth: meters(1.8),
            weight: quantity(500.0, QuantityUnit::Gram), amount: "2".into(), orientations: vec![Bpp3dOrientation::Upright],
            shape: Some(Bpp3dShape::Cylinder { axis: Bpp3dAxis::Y, radius: meters(0.5), radius_candidates: candidates }) }],
        bins: vec![Bpp3dBin { id: "bin".into(), type_code: "type".into(), width: meters(3.0), height: meters(2.0), depth: meters(3.0),
            capacity: quantity(2.0, QuantityUnit::Kilogram), is_main: true }],
        initial_layers: vec![Bpp3dLayer { id: "layer".into(), bin_id: "bin".into(), depth: meters(1.8),
            placements: vec![Bpp3dPlacement { item_id: "drum".into(), orientation: Bpp3dOrientation::Upright,
                position: Bpp3dPoint { x: meters(0.2), y: meters(0.0), z: meters(0.0) } },
                Bpp3dPlacement { item_id: "drum".into(), orientation: Bpp3dOrientation::Upright,
                    position: Bpp3dPoint { x: meters(1.0), y: meters(0.0), z: meters(0.8) } }] }],
        radius_component: None, package_attributes: Vec::new(),
    };
    let candidates = |min: f64, max: f64, step: f64| Some(Bpp3dRadiusCandidates {
        min: meters(min), max: meters(max), step: meters(step) });
    // 0.3..=0.5 步长 0.1 展开为 0.3/0.4/0.5，含上界容差。
    // 0.3..=0.5 by 0.1 expands to 0.3/0.4/0.5 with the inclusive tolerance.
    let (request, _) = drum(candidates(0.3, 0.5, 0.1)).domain().unwrap();
    let shape = request.items[0].shape_spec_override.as_ref().unwrap();
    let PackageShapeSpec::Cylinder { radius_candidates, .. } = shape else { panic!("expected cylinder") };
    let values = radius_candidates.as_ref().unwrap().iter().map(|candidate| candidate.value).collect::<Vec<_>>();
    assert_eq!(values, vec![0.3, 0.4, 0.5]);
    // 未给出候选时保持 None，不凭空造列表。 / Without candidates the field stays None.
    let (request, _) = drum(None).domain().unwrap();
    let PackageShapeSpec::Cylinder { radius_candidates, .. } = request.items[0].shape_spec_override.as_ref().unwrap() else { panic!("expected cylinder") };
    assert!(radius_candidates.is_none());
    // 倒序、非正步长、超过横截面、以及超出预算的候选数都必须拒绝。
    // Inverted bounds, non-positive steps, candidates beyond the cross-section and oversized lists must all fail.
    for invalid in [candidates(0.5, 0.3, 0.1), candidates(0.3, 0.5, 0.0), candidates(0.3, 0.5, -0.1), candidates(0.3, 1.2, 0.1), candidates(0.0, 1.0, 0.0001)] {
        let label = format!("{invalid:?}");
        assert!(drum(invalid).domain().is_err(), "{label}");
    }
}

#[test]
fn pool_roundtrip_preserves_the_radius_component() {
    let (source, state, _) = super::super::bpp3d::pool::tests::radius_fixture();
    let text = encode_bpp3d_column_pool(&source, &state).unwrap();
    let decoded = decode_bpp3d_column_pool(&text, &source).unwrap();
    assert!(decoded.continuous_radius_component.is_some());
    assert_eq!(decoded.continuous_radius_component, state.continuous_radius_component);
    // 带组件的 state 若被改动，仍必须被拒绝。 / A state whose component no longer matches the source must still be rejected.
    let mut altered = state.clone();
    altered.continuous_radius_component = None;
    assert!(encode_bpp3d_column_pool(&source, &altered).is_err());
}

#[test]
fn radius_component_reaches_the_materialized_request_and_validates_declarations() {
    let meters = |value| quantity(value, QuantityUnit::Meter);
    let drum = |component: Option<Bpp3dRadiusComponent>| Bpp3dProblem {
        items: vec![Bpp3dItem { id: "drum".into(), name: "Drum".into(), width: meters(1.8), height: meters(1.0), depth: meters(1.8),
            weight: quantity(500.0, QuantityUnit::Gram), amount: "2".into(), orientations: vec![Bpp3dOrientation::Upright],
            shape: Some(Bpp3dShape::Cylinder { axis: Bpp3dAxis::Y, radius: meters(0.5), radius_candidates: None }) },
            Bpp3dItem { id: "box".into(), name: "Box".into(), width: meters(1.0), height: meters(1.0), depth: meters(1.0),
                weight: quantity(500.0, QuantityUnit::Gram), amount: "1".into(), orientations: vec![Bpp3dOrientation::Upright], shape: None }],
        bins: vec![Bpp3dBin { id: "bin".into(), type_code: "type".into(), width: meters(3.0), height: meters(2.0), depth: meters(3.0),
            capacity: quantity(5.0, QuantityUnit::Kilogram), is_main: true }],
        initial_layers: vec![Bpp3dLayer { id: "layer".into(), bin_id: "bin".into(), depth: meters(3.0),
            placements: vec![Bpp3dPlacement { item_id: "drum".into(), orientation: Bpp3dOrientation::Upright,
                position: Bpp3dPoint { x: meters(0.2), y: meters(0.0), z: meters(0.0) } },
                Bpp3dPlacement { item_id: "box".into(), orientation: Bpp3dOrientation::Upright,
                    position: Bpp3dPoint { x: meters(2.0), y: meters(0.0), z: meters(1.0) } }] }],
        radius_component: component, package_attributes: Vec::new(),
    };
    let functions = || vec![Bpp3dRadiusWeightFunction { key: "weight".into(), intercept: 0.0, radius_squared_coefficient: 1.0, objective_weight: 1.0 }];
    let prototype = |item: &str, axis: Bpp3dAxis, lower: f64, upper: f64| Bpp3dRadiusPrototype {
        item_id: item.into(), source: "weight".into(), axis,
        lower_bound: quantity(lower, QuantityUnit::Meter), upper_bound: quantity(upper, QuantityUnit::Meter) };
    let (request, _) = drum(Some(Bpp3dRadiusComponent { prototypes: vec![prototype("drum", Bpp3dAxis::Y, 0.4, 0.9)], weight_functions: functions(), objective: None }))
        .domain().expect("radius component must materialize");
    let component = request.continuous_radius_component.expect("component must reach the request");
    assert_eq!(component.prototypes.len(), 1);
    assert_eq!(component.prototypes[0].radius_lower_bound, Some(0.4));
    assert_eq!(component.prototypes[0].radius_upper_bound, Some(0.9));
    assert_eq!(component.registration_plan.registered_variables.len(), 1);
    assert!(component.registration_plan.blocked_variables.is_empty());
    assert_eq!(component.prototypes[0].variable_name, "r_drum_weight");
    // 没有组件时保持为 None，不凭空造一个空组件 / Without a component the request stays None.
    assert!(drum(None).domain().unwrap().0.continuous_radius_component.is_none());
    // 长方体、轴不一致、未知物品、重复物品与非法边界全部拒绝 / Cuboids, wrong axes, unknown or duplicate items and invalid bounds all fail.
    assert!(drum(Some(Bpp3dRadiusComponent { prototypes: vec![prototype("box", Bpp3dAxis::Y, 0.4, 0.9)], weight_functions: functions(), objective: None })).domain().is_err());
    assert!(drum(Some(Bpp3dRadiusComponent { prototypes: vec![prototype("drum", Bpp3dAxis::Z, 0.4, 0.9)], weight_functions: functions(), objective: None })).domain().is_err());
    assert!(drum(Some(Bpp3dRadiusComponent { prototypes: vec![prototype("ghost", Bpp3dAxis::Y, 0.4, 0.9)], weight_functions: functions(), objective: None })).domain().is_err());
    assert!(drum(Some(Bpp3dRadiusComponent { prototypes: vec![prototype("drum", Bpp3dAxis::Y, 0.9, 0.4)], weight_functions: functions(), objective: None })).domain().is_err());
    assert!(drum(Some(Bpp3dRadiusComponent { prototypes: vec![prototype("drum", Bpp3dAxis::Y, -1.0, 0.4)], weight_functions: functions(), objective: None })).domain().is_err());
    assert!(drum(Some(Bpp3dRadiusComponent { prototypes: vec![], weight_functions: functions(), objective: None })).domain().is_err());
    // 权重函数缺失、系数为零、重复键都必须在边界报错，而不是留给 final 阶段一句含糊诊断。
    // A missing weight function, zero coefficients and duplicate keys must fail at the boundary instead of leaving an
    // opaque final-stage diagnostic.
    assert!(drum(Some(Bpp3dRadiusComponent { prototypes: vec![prototype("drum", Bpp3dAxis::Y, 0.4, 0.9)], weight_functions: vec![], objective: None })).domain().is_err());
    assert!(drum(Some(Bpp3dRadiusComponent { prototypes: vec![prototype("drum", Bpp3dAxis::Y, 0.4, 0.9)],
        weight_functions: vec![Bpp3dRadiusWeightFunction { key: "weight".into(), intercept: 0.0, radius_squared_coefficient: 0.0, objective_weight: 1.0 }], objective: None })).domain().is_err());
    assert!(drum(Some(Bpp3dRadiusComponent { prototypes: vec![prototype("drum", Bpp3dAxis::Y, 0.4, 0.9)],
        weight_functions: vec![Bpp3dRadiusWeightFunction { key: "weight".into(), intercept: 0.0, radius_squared_coefficient: 1.0, objective_weight: 1.0 },
            Bpp3dRadiusWeightFunction { key: "weight".into(), intercept: 0.0, radius_squared_coefficient: 1.0, objective_weight: 1.0 }], objective: None })).domain().is_err());
    // 目标策略权重必须有限且非零；显式 None 与缺省都要落到正确的领域变体。
    // Objective weights must be finite and nonzero, and both explicit None and absence must map to the right domain
    // variant.
    assert!(drum(Some(Bpp3dRadiusComponent { prototypes: vec![prototype("drum", Bpp3dAxis::Y, 0.4, 0.9)], weight_functions: functions(),
        objective: Some(Bpp3dRadiusObjective::MinimizeRadiusSquared { weight: 0.0 }) })).domain().is_err());
    assert!(drum(Some(Bpp3dRadiusComponent { prototypes: vec![prototype("drum", Bpp3dAxis::Y, 0.4, 0.9)], weight_functions: functions(),
        objective: Some(Bpp3dRadiusObjective::MinimizeRadiusSquared { weight: f64::NAN }) })).domain().is_err());
    assert!(drum(Some(Bpp3dRadiusComponent { prototypes: vec![prototype("drum", Bpp3dAxis::Y, 0.4, 0.9)], weight_functions: functions(),
        objective: Some(Bpp3dRadiusObjective::MinimizeRadiusSquared { weight: 2.5 }) })).domain().is_ok());
    let (request, _) = drum(Some(Bpp3dRadiusComponent { prototypes: vec![prototype("drum", Bpp3dAxis::Y, 0.4, 0.9)], weight_functions: functions(), objective: None })).domain().unwrap();
    assert_eq!(request.continuous_radius_component.unwrap().weight_functions.len(), 1);
}

#[test]
fn source_roundtrip_uses_original_domain_and_replays_physical_traces() {
    let source = problem();
    let artifact = encode_bpp3d_problem(source).unwrap();
    let (_, request, traces) = decode_bpp3d_problem(&artifact).unwrap();
    assert_eq!(request.item_amounts, [("item".into(), 2)]);
    assert_eq!(request.items[0].width.value, 1.0);
    assert_eq!(request.items[0].weight.value, 0.5);
    assert_eq!(request.bins[0].capacity.value, 1.0);
    assert_eq!(request.initial_layers[0].demand_coverage[0].coefficient, 2.0);
    assert_eq!(traces[0].1.len(), 2);
    assert_eq!(traces[0].1[1].position.x.value, 1.0);
    assert!(crate::typescript_declarations().contains("type Bpp3dProblem"));
}

#[test]
fn source_rejects_invalid_units_shapes_counts_and_unknown_rules() {
    let mut source = problem(); source.items[0].weight.unit = QuantityUnit::Meter; assert!(source.domain().is_err());
    let mut source = problem(); source.bins[0].capacity.unit = QuantityUnit::Second; assert!(source.domain().is_err());
    let mut source = problem(); source.items[0].width.unit = QuantityUnit::Kilogram; assert!(source.domain().is_err());
    for amount in ["0", "01", "-1", "100001", "9007199254740993"] {
        let mut source = problem(); source.items[0].amount = amount.into(); assert!(source.domain().is_err());
    }
    let mut source = problem(); source.items[0].width.value = f64::INFINITY; assert!(source.domain().is_err());
    let mut source = problem(); source.items[0].width.value = 0.0; assert!(source.domain().is_err());
    let mut source = problem(); source.items[0].orientations.clear(); assert!(source.domain().is_err());
    let mut source = problem(); source.items[0].orientations.push(Bpp3dOrientation::Side); assert!(source.domain().is_err());
    let mut value = serde_json::to_value(problem()).unwrap(); value["items"][0]["shape"] = "cylinder".into();
    assert!(serde_json::from_value::<Bpp3dProblem>(value).is_err());
}

#[test]
fn cylinder_sources_bind_axis_and_radius_to_the_original_geometry_guard() {
    let mut source = problem();
    source.items[0].shape = Some(Bpp3dShape::Cylinder { axis: Bpp3dAxis::X, radius: quantity(50.0, QuantityUnit::Centimeter), radius_candidates: None });
    let artifact = encode_bpp3d_problem(source.clone()).unwrap();
    let (_, request, _) = decode_bpp3d_problem(&artifact).unwrap();
    match &request.items[0].shape_spec_override {
        Some(PackageShapeSpec::Cylinder { axis, radius, radius_candidates, radius_lower_bound, radius_upper_bound }) => {
            assert_eq!(*axis, Axis3::X); assert_eq!(radius.value, 0.5);
            assert!(radius_candidates.is_none() && radius_lower_bound.is_none() && radius_upper_bound.is_none());
        }
        other => panic!("expected an axis-aware cylinder, found {other:?}"),
    }
    // 形状参与源指纹，长方体与圆柱不是同一源。 / Geometry participates in the source fingerprint, so a cuboid and a cylinder are different sources.
    assert_ne!(artifact.fingerprint, encode_bpp3d_problem(problem()).unwrap().fingerprint);
    // 半径超出横截面、为零或缺少轴都必须被拒绝。 / A radius beyond the cross-section, a zero radius or a missing axis must be rejected.
    let mut too_large = source.clone();
    too_large.items[0].shape = Some(Bpp3dShape::Cylinder { axis: Bpp3dAxis::X, radius: quantity(60.0, QuantityUnit::Centimeter), radius_candidates: None });
    assert!(too_large.domain().is_err());
    let mut zero = source.clone();
    zero.items[0].shape = Some(Bpp3dShape::Cylinder { axis: Bpp3dAxis::Y, radius: quantity(0.0, QuantityUnit::Meter), radius_candidates: None });
    assert!(zero.domain().is_err());
    let mut malformed = serde_json::to_value(problem()).unwrap();
    malformed["items"][0]["shape"] = serde_json::json!({ "kind": "cylinder" });
    assert!(serde_json::from_value::<Bpp3dProblem>(malformed).is_err());
    let mut unknown = serde_json::to_value(problem()).unwrap();
    unknown["items"][0]["shape"] = serde_json::json!({ "kind": "prism" });
    assert!(serde_json::from_value::<Bpp3dProblem>(unknown).is_err());
}

#[test]
fn cylinder_footprints_use_real_circles_not_bounding_squares() {
    let meters = |value| quantity(value, QuantityUnit::Meter);
    let drum = |id: &str, x: f64, z: f64| Bpp3dPlacement { item_id: id.into(),
        position: Bpp3dPoint { x: meters(x), y: meters(0.0), z: meters(z) }, orientation: Bpp3dOrientation::Upright };
    let source = |radius: f64| Bpp3dProblem {
        items: vec![Bpp3dItem { id: "drum".into(), name: "Drum".into(), width: meters(1.8), height: meters(1.0), depth: meters(1.8),
            weight: quantity(500.0, QuantityUnit::Gram), amount: "2".into(), orientations: vec![Bpp3dOrientation::Upright],
            shape: Some(Bpp3dShape::Cylinder { axis: Bpp3dAxis::Y, radius: meters(radius), radius_candidates: None }) }],
        bins: vec![Bpp3dBin { id: "bin".into(), type_code: "type".into(), width: meters(3.0), height: meters(2.0), depth: meters(3.0),
            capacity: quantity(2.0, QuantityUnit::Kilogram), is_main: true }],
        initial_layers: vec![Bpp3dLayer { id: "layer".into(), bin_id: "bin".into(), depth: meters(1.8),
            placements: vec![drum("drum", 0.2, 0.0), drum("drum", 1.0, 0.8)] }],
        radius_component: None, package_attributes: Vec::new(),
    };
    // 包围方框重叠但圆形足迹不重叠：真实圆几何接受，方形近似会误判。
    // Bounding squares overlap while the circular footprints do not: real circle geometry accepts, a square approximation would not.
    let probe = source(0.5).domain();
    assert!(probe.is_ok(), "{:?}", probe.err());
    // 半径放大到圆形足迹真正重叠时，原领域几何门禁必须拒绝。
    // Growing the radius until the circular footprints really overlap must be rejected by the original geometry guard.
    assert!(source(0.9).domain().is_err());
}

#[test]
fn source_rejects_foreign_overlapping_disabled_out_of_bounds_and_overweight_placements() {
    let mut source = problem(); source.initial_layers[0].placements[0].item_id = "foreign".into(); assert!(source.domain().is_err());
    let mut source = problem(); source.initial_layers[0].bin_id = "foreign".into(); assert!(source.domain().is_err());
    let mut source = problem(); source.initial_layers[0].placements[1].position.x.value = 0.5; assert!(source.domain().is_err());
    let mut source = problem(); source.initial_layers[0].placements[1].position.x.value = 2.0; assert!(source.domain().is_err());
    let mut source = problem(); source.initial_layers[0].placements[1].orientation = Bpp3dOrientation::Lie; assert!(source.domain().is_err());
    let mut source = problem(); source.initial_layers[0].placements[0].position.x.value = -0.1; assert!(source.domain().is_err());
    let mut source = problem(); source.bins[0].capacity.value = 0.9; assert!(source.domain().is_err());
    let mut source = problem(); source.initial_layers[0].placements.clear(); assert!(source.domain().is_err());
}

#[test]
fn same_type_bins_are_distinct_but_must_be_consistent() {
    let mut source = problem();
    let mut second = source.bins[0].clone(); second.id = "bin-2".into();
    second.width = quantity(200.0, QuantityUnit::Centimeter);
    source.bins.push(second);
    let (request, _) = source.domain().unwrap();
    assert_eq!(request.bins.len(), 2);
    assert_eq!(request.bins[0].type_code, request.bins[1].type_code);
    source.bins[1].capacity.value = 2.0; assert!(source.domain().is_err());
    source.bins[1].capacity.value = 1.0; source.bins[1].id = "bin-1".into(); assert!(source.domain().is_err());
}

#[test]
fn artifacts_reject_forged_noncanonical_and_unsupported_sources() {
    let artifact = encode_bpp3d_problem(problem()).unwrap();
    let mut forged = artifact.clone(); forged.fingerprint = "foreign".into();
    assert_eq!(decode_bpp3d_problem(&forged).unwrap_err().code, "FingerprintMismatch");
    let mut forged = artifact.clone(); forged.model_json.push(' ');
    assert_eq!(decode_bpp3d_problem(&forged).unwrap_err().code, "FingerprintMismatch");
    let mut forged = artifact; forged.model_json = forged.model_json.replace("\"schemaVersion\":\"1.0\"", "\"schemaVersion\":\"2.0\"");
    assert_eq!(decode_bpp3d_problem(&forged).unwrap_err().code, "UnsupportedProtocolVersion");
}

fn layered_problem() -> Bpp3dProblem {
    let mut source = problem();
    source.items[0].amount = "4".into();
    source.bins[0].depth = quantity(2.0, QuantityUnit::Meter);
    source.bins[0].capacity = quantity(2.0, QuantityUnit::Kilogram);
    let mut second_bin = source.bins[0].clone(); second_bin.id = "bin-2".into(); source.bins.push(second_bin);
    let mut second_layer = source.initial_layers[0].clone(); second_layer.id = "layer-2".into(); source.initial_layers.push(second_layer);
    source
}

fn assignment(bin: &str, layer: &str) -> Bpp3dAssignment {
    Bpp3dAssignment { bin_id: bin.into(), layer_id: layer.into() }
}

#[test]
fn packing_replays_complete_multi_layer_and_cross_bin_assignments_against_source() {
    let source = encode_bpp3d_problem(layered_problem()).unwrap();
    let packed = pack_bpp3d_layers(&source, &[assignment("bin-1", "layer-2"), assignment("bin-1", "layer")]).unwrap();
    assert_eq!(packed.source_fingerprint, source.fingerprint);
    assert_eq!(packed.bins.len(), 1);
    assert_eq!(packed.bins[0].bin_id, "bin-1");
    assert_eq!(packed.bins[0].placements.len(), 4);
    assert_eq!(packed.bins[0].placements.iter().map(|p| p.position.z.value).collect::<Vec<_>>(), vec![0.0, 0.0, 1.0, 1.0]);
    assert!(packed.bins[0].placements.iter().all(|p| p.item_id == "item" && matches!(p.position.z.unit, QuantityUnit::Meter)));

    let packed = pack_bpp3d_layers(&source, &[assignment("bin-2", "layer"), assignment("bin-1", "layer")]).unwrap();
    assert_eq!(packed.bins.iter().map(|bin| bin.bin_id.as_str()).collect::<Vec<_>>(), vec!["bin-1", "bin-2"]);
    assert!(packed.bins.iter().all(|bin| bin.placements.len() == 2 && bin.placements[0].position.z.value == 0.0));
    assert!(crate::typescript_declarations().contains("type Bpp3dPacking"));
}

#[test]
fn packing_rejects_foreign_duplicate_incomplete_overfull_and_wrong_bin_assignments() {
    let source = encode_bpp3d_problem(layered_problem()).unwrap();
    for assignments in [vec![], vec![assignment("bin-1", "layer")], vec![assignment("foreign", "layer")],
        vec![assignment("bin-1", "foreign")], vec![assignment("bin-1", "layer"), assignment("bin-1", "layer")],
        vec![assignment("bin-1", "layer"), assignment("bin-1", "layer-2"), assignment("bin-2", "layer")]] {
        assert!(pack_bpp3d_layers(&source, &assignments).is_err());
    }
    assert!(pack_bpp3d_layers(&source, &vec![assignment("bin-1", "layer"); MAX_PLACEMENTS + 1]).is_err());
    assert!(serde_json::from_str::<Bpp3dAssignment>(r#"{"binId":"bin-1","layerId":"layer","extra":true}"#).is_err());
    let mut forged = source.clone(); forged.fingerprint = "foreign".into();
    assert_eq!(pack_bpp3d_layers(&forged, &[]).unwrap_err().code, "FingerprintMismatch");

    let mut problem = layered_problem(); problem.bins[1].type_code = "other".into();
    let source = encode_bpp3d_problem(problem).unwrap();
    assert!(pack_bpp3d_layers(&source, &[assignment("bin-1", "layer"), assignment("bin-2", "layer")]).is_err());
}

#[test]
fn packing_enforces_expanded_placement_limit_even_with_few_assignments() {
    let mut problem = problem();
    problem.items[0].amount = (17 * MAX_LAYER_PLACEMENTS).to_string();
    problem.bins[0].width = quantity(MAX_LAYER_PLACEMENTS as f64, QuantityUnit::Meter);
    problem.bins[0].capacity = quantity(128.0, QuantityUnit::Kilogram);
    problem.initial_layers[0].placements = (0..MAX_LAYER_PLACEMENTS).map(|index| Bpp3dPlacement {
        item_id: "item".into(), orientation: Bpp3dOrientation::Upright,
        position: Bpp3dPoint { x: quantity(index as f64, QuantityUnit::Meter), y: quantity(0.0, QuantityUnit::Meter), z: quantity(0.0, QuantityUnit::Meter) },
    }).collect();
    for index in 2..=17 {
        let mut bin = problem.bins[0].clone(); bin.id = format!("bin-{index}"); problem.bins.push(bin);
    }
    let source = encode_bpp3d_problem(problem).unwrap();
    let assignments = (1..=17).map(|index| assignment(&format!("bin-{index}"), "layer")).collect::<Vec<_>>();
    assert!(pack_bpp3d_layers(&source, &assignments).unwrap_err().message.contains("placement limit"));
}

#[test]
fn packing_rechecks_combined_weight_and_stacked_depth_not_only_individual_layers() {
    let plan = [assignment("bin-1", "layer"), assignment("bin-1", "layer-2")];
    let mut problem = layered_problem();
    for bin in &mut problem.bins { bin.capacity = quantity(1.0, QuantityUnit::Kilogram); }
    let source = encode_bpp3d_problem(problem).unwrap();
    assert!(pack_bpp3d_layers(&source, &plan).unwrap_err().message.contains("weight capacity"));

    let mut problem = layered_problem();
    for bin in &mut problem.bins { bin.depth = quantity(1.0, QuantityUnit::Meter); }
    let source = encode_bpp3d_problem(problem).unwrap();
    assert!(pack_bpp3d_layers(&source, &plan).unwrap_err().message.contains("depth"));
}
