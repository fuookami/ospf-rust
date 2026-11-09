use super::*;

fn meters(value: f64) -> PhysicalQuantity { PhysicalQuantity { value, unit: QuantityUnit::Meter } }
fn size(width: f64, height: f64) -> Geometry2dSheet { Geometry2dSheet { width: meters(width), height: meters(height) } }
fn region(x: f64, width: f64) -> Csp2dRegion { Csp2dRegion { origin: Geometry2dPoint { x: meters(x), y: meters(0.0) }, size: size(width, 1.0) } }

pub(crate) fn problem() -> Csp2dProblem {
    Csp2dProblem {
        products: vec![Csp2dProduct { id: "a".into(), name: "A".into(), geometry: size(1.0, 1.0), allow_rotate: false }],
        materials: vec![Csp2dMaterial { id: "s".into(), name: "sheet".into(), shape: Csp2dMaterialShape::Sheet { geometry: size(4.0, 2.0) },
            available_batches: "10".into(), process: Csp2dCuttingProcess::Freeform, max_segment_height: None,
            products: vec![Csp2dGenerationProduct { product_id: "a".into(), count: "4".into() }] }],
        demands: vec![Csp2dDemand::Count { product_id: "a".into(), count: "3".into(), upper_bound: None }],
        initial_columns: vec![Csp2dColumn { id: "seed".into(), material_id: "s".into(), region: region(0.0, 2.0),
            placements: (0..2).map(|index| Geometry2dPlacement { shape: crate::Geometry2dShape::Rectangle { width: meters(1.0), height: meters(1.0) },
                position: Geometry2dPoint { x: meters(index as f64), y: meters(0.0) }, rotated: false, item_id: Some("a".into()) }).collect(), cut_tree: None }],
        kerf: meters(0.0), objective: Default::default(),
    }
}

fn guillotine() -> Csp2dProblem {
    let mut source = problem(); source.materials[0].process = Csp2dCuttingProcess::GuillotineTwoStage;
    source.initial_columns[0].cut_tree = Some(Csp2dCutTree { nodes: vec![
        Csp2dCutNode::Leaf { region: region(0.0, 1.0), product_id: Some("a".into()) },
        Csp2dCutNode::Leaf { region: region(1.0, 1.0), product_id: Some("a".into()) },
        Csp2dCutNode::Cut { region: region(0.0, 2.0), orientation: Csp2dCutOrientation::Vertical, position: meters(1.0), stage: 1, lower: 0, upper: 1 },
    ] }); source
}

#[test]
fn canonical_source_preserves_units_exact_counts_independent_bases_and_duplicate_positions() {
    let mut source = problem(); source.products[0].geometry.width = PhysicalQuantity { value: 100.0, unit: QuantityUnit::Centimeter };
    source.demands[0] = Csp2dDemand::Count { product_id: "a".into(), count: "9007199254740991".into(), upper_bound: None };
    source.demands.push(Csp2dDemand::Area { product_id: "a".into(), area: PhysicalQuantity { value: 30_000.0, unit: QuantityUnit::SquareCentimeter }, upper_bound: None });
    source.initial_columns.push(source.initial_columns[0].clone());
    let artifact = encode_csp2d_problem(source).unwrap(); let (_, domain) = decode_csp2d_problem(&artifact).unwrap();
    assert!(artifact.model_json.contains("9007199254740991")); assert_eq!(domain.initial_columns.len(), 2);
    assert_eq!(domain.initial_columns[0].id.as_str(), "seed"); assert_eq!(domain.requests[0].products[0].0.width.value, 1.0);
    assert_eq!(domain.demands[1].demand, DemandValue2::Area(3.0));
    for changed in [ModelArtifact { fingerprint: "foreign".into(), ..artifact.clone() }, ModelArtifact { model_json: format!("{} ", artifact.model_json), ..artifact.clone() }] {
        assert_eq!(decode_csp2d_problem(&changed).unwrap_err().code, "FingerprintMismatch");
    }
    let changed = ModelArtifact { model_json: artifact.model_json.replacen("\"1.0\"", "\"2.0\"", 1), ..artifact.clone() };
    assert_eq!(decode_csp2d_problem(&changed).unwrap_err().code, "UnsupportedProtocolVersion");
    let changed = ModelArtifact { model_json: artifact.model_json.replacen("\"problem\":", "\"extra\":true,\"problem\":", 1), ..artifact };
    assert!(decode_csp2d_problem(&changed).is_err());
}

#[test]
fn invalid_catalogs_numeric_values_references_and_unsupported_fields_fail_before_execution() {
    for variant in 0..25 {
        let mut source = problem();
        match variant {
            0 => source.products.push(source.products[0].clone()),
            1 => source.materials.push(source.materials[0].clone()),
            2 => source.materials[0].products[0].count = "0".into(),
            3 => source.materials[0].products[0].count = "4097".into(),
            4 => source.materials[0].products[0].count = "01".into(),
            5 => source.materials[0].available_batches = "0".into(),
            6 => source.materials[0].available_batches = "9007199254740992".into(),
            7 => source.demands[0] = Csp2dDemand::Count { product_id: "a".into(), count: "9007199254740992".into(), upper_bound: None },
            8 => source.demands[0] = Csp2dDemand::Count { product_id: "a".into(), count: "3".into(), upper_bound: Some("2".into()) },
            9 => source.demands.push(source.demands[0].clone()),
            10 => source.initial_columns[0].material_id = "foreign".into(),
            11 => source.initial_columns[0].placements[0].item_id = Some("foreign".into()),
            12 => source.initial_columns[0].placements[0].rotated = true,
            13 => source.initial_columns[0].placements[1].position.x = meters(0.5),
            14 => source.initial_columns[0].region.size.width = meters(0.5),
            15 => source.kerf = meters(-1.0),
            16 => source.kerf.unit = QuantityUnit::Second,
            17 => source.products[0].geometry.width = meters(f64::NAN),
            18 => source.objective.area_weight = f64::INFINITY,
            19 => source.materials[0].max_segment_height = Some(meters(1.0)),
            20 => source.materials[0].products[0].product_id = "foreign".into(),
            21 => source.initial_columns[0].placements[0].shape = crate::Geometry2dShape::Circle { radius: meters(0.5) },
            22 => source.initial_columns[0].placements[0].position.x = meters(f64::INFINITY),
            23 => source.materials[0].process = Csp2dCuttingProcess::GuillotineTwoStage,
            _ => source.demands.push(Csp2dDemand::Area { product_id: "a".into(), area: meters(1.0), upper_bound: None }),
        }
        assert!(source.domain().is_err(), "variant {variant}");
    }
    let source = serde_json::to_value(problem()).unwrap();
    for field in ["weight", "machine", "costars", "callback"] {
        let mut value = source.clone(); value.as_object_mut().unwrap().insert(field.into(), serde_json::Value::Null);
        assert!(serde_json::from_value::<Csp2dProblem>(value).is_err());
    }
}

#[test]
fn flat_guillotine_tree_checks_physical_bijection_stage_shape_and_strict_postorder() {
    let source = guillotine(); let artifact = encode_csp2d_problem(source.clone()).unwrap(); let (_, input) = decode_csp2d_problem(&artifact).unwrap();
    assert_eq!(input.initial_columns[0].pattern.cut_tree.as_ref().unwrap().leaves().len(), 2);
    for variant in 0..9 {
        let mut source = source.clone(); let nodes = &mut source.initial_columns[0].cut_tree.as_mut().unwrap().nodes;
        match variant {
            0 => if let Csp2dCutNode::Cut { lower, .. } = &mut nodes[2] { *lower = 2; },
            1 => if let Csp2dCutNode::Cut { upper, .. } = &mut nodes[2] { *upper = 0; },
            2 => if let Csp2dCutNode::Leaf { product_id, .. } = &mut nodes[0] { *product_id = None; },
            3 => if let Csp2dCutNode::Leaf { region, .. } = &mut nodes[0] { region.origin.x = meters(1e-10); },
            4 => if let Csp2dCutNode::Cut { stage, .. } = &mut nodes[2] { *stage = 2; },
            5 => if let Csp2dCutNode::Cut { position, .. } = &mut nodes[2] { position.unit = QuantityUnit::SquareMeter; },
            6 => nodes.push(Csp2dCutNode::Leaf { region: region(0.0, 2.0), product_id: None }),
            7 => nodes.clear(),
            _ => nodes.resize(4097, nodes[0].clone()),
        }
        assert!(source.domain().is_err(), "variant {variant}");
    }
}

#[test]
fn flat_tree_limits_depth_and_rejects_subtree_aliasing_without_recursive_json() {
    let mut source = guillotine(); let tree = source.initial_columns[0].cut_tree.as_mut().unwrap();
    tree.nodes = vec![Csp2dCutNode::Leaf { region: region(0.0, 1.0), product_id: None }];
    for level in 0..129 {
        let child = (tree.nodes.len() - 1) as u32;
        tree.nodes.push(Csp2dCutNode::Leaf { region: region(0.0, 1.0), product_id: None });
        tree.nodes.push(Csp2dCutNode::Cut { region: region(0.0, (level + 2) as f64), orientation: Csp2dCutOrientation::Vertical,
            position: meters(1.0), stage: 1, lower: child + 1, upper: child });
    }
    let geometry = Geometry2d::new(QuantityUnit::Meter).unwrap();
    assert!(tree.domain(&geometry, &ospf_rust_quantities::quantity::Quantity::new_ct(0.0)).unwrap_err().message.contains("depth 128"));
    let mut nodes = guillotine().initial_columns.remove(0).cut_tree.unwrap().nodes;
    nodes.push(Csp2dCutNode::Cut { region: region(0.0, 2.0), orientation: Csp2dCutOrientation::Vertical, position: meters(1.0), stage: 1, lower: 0, upper: 2 });
    assert!(Csp2dCutTree { nodes }.domain(&geometry, &ospf_rust_quantities::quantity::Quantity::new_ct(0.0)).unwrap_err().message.contains("reuses"));
}

#[test]
fn coil_bounds_and_partial_sheet_consumption_are_original_domain_semantics() {
    let mut source = problem(); source.materials[0].shape = Csp2dMaterialShape::Coil { width: meters(4.0), length: Some(meters(1.5)) };
    source.materials[0].max_segment_height = Some(meters(1.0));
    let domain = source.domain().unwrap();
    assert_eq!(domain.initial_columns[0].consumed_area().value, 2.0); assert_eq!(domain.requests[0].material.available_batches, 10);
    source.initial_columns[0].region.size.height = meters(1.25); assert!(source.domain().is_err());
    source.initial_columns.clear(); assert!(source.domain().is_ok());
}

#[test]
fn positive_kerf_and_rotated_rectangles_retain_original_physical_contracts() {
    let mut source = guillotine(); source.kerf = PhysicalQuantity { value: 125.0, unit: QuantityUnit::Millimeter };
    source.initial_columns[0].region.size.width = meters(2.125);
    source.initial_columns[0].placements[1].position.x = meters(1.125);
    let nodes = &mut source.initial_columns[0].cut_tree.as_mut().unwrap().nodes;
    if let Csp2dCutNode::Leaf { region, .. } = &mut nodes[1] { region.origin.x = meters(1.125); }
    if let Csp2dCutNode::Cut { region, .. } = &mut nodes[2] { region.size.width = meters(2.125); }
    let artifact = encode_csp2d_problem(source.clone()).unwrap();
    let (_, domain) = decode_csp2d_problem(&artifact).unwrap();
    assert_eq!(domain.requests[0].kerf.value, 0.125);
    source.initial_columns[0].placements[1].position.x = meters(1.0);
    assert!(source.domain().is_err());

    let mut source = problem(); source.products[0].geometry = size(2.0, 1.0); source.products[0].allow_rotate = true;
    source.initial_columns[0].region.size = size(2.0, 2.0);
    for placement in &mut source.initial_columns[0].placements {
        placement.shape = crate::Geometry2dShape::Rectangle { width: meters(1.0), height: meters(2.0) };
        placement.rotated = true;
    }
    let artifact = encode_csp2d_problem(source.clone()).unwrap();
    let (_, domain) = decode_csp2d_problem(&artifact).unwrap();
    assert!(domain.initial_columns[0].pattern.placements.iter().all(|placement| placement.rotated));
    source.products[0].allow_rotate = false; assert!(source.domain().is_err());
}

#[test]
fn source_and_pool_resource_limits_fail_without_truncation() {
    for variant in 0..5 {
        let mut source = problem();
        match variant {
            0 => source.products.resize(65, source.products[0].clone()),
            1 => source.materials.resize(17, source.materials[0].clone()),
            2 => source.demands.resize(129, source.demands[0].clone()),
            3 => source.initial_columns.resize(257, source.initial_columns[0].clone()),
            _ => source.products[0].id = "a".repeat(257),
        }
        assert!(source.domain().is_err(), "source variant {variant}");
    }
    let source = problem(); let input = source.domain().unwrap();
    assert!(decode_csp2d_columns(&input, &vec![source.initial_columns[0].clone(); 513]).is_err());
    let mut placements = source.initial_columns[0].clone(); placements.placements.resize(4097, placements.placements[0].clone());
    assert!(decode_csp2d_columns(&input, &[placements]).is_err());
}

#[test]
fn bounded_payloads_and_generated_contracts_never_copy_wire_integer_fields_as_numbers() {
    let artifact = ModelArtifact { model_json: " ".repeat(CSP2D_MAX_BYTES + 1), fingerprint: "x".into() };
    assert_eq!(decode_csp2d_problem(&artifact).unwrap_err().code, "PayloadTooLarge");
    let declarations = crate::typescript_declarations();
    assert!(declarations.contains("type Csp2dProblem")); assert!(declarations.contains("availableBatches: string"));
    assert!(declarations.contains("type Csp2dCutNode"));
}
