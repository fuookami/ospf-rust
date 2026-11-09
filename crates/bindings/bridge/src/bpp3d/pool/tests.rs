use super::*;
use crate::bpp3d::{Bpp3dAxis, Bpp3dShape};
use ospf_rust_framework_bpp3d::{
    application::replay_strict_grouped_packing,
    domain::{
        layer_assignment::{LayerAssignment, PreciseAssignmentSelection},
        layer_generation::{BlockLayerGenerator, LayerGenerationDemandEntry, LayerGenerationRequest, LayerGenerator},
    },
};
use crate::{Bpp3dBin, Bpp3dItem, Bpp3dLayer, Bpp3dPlacement, Bpp3dPoint, Bpp3dProblem, PhysicalQuantity, QuantityUnit, encode_bpp3d_problem,
    Bpp3dRadiusComponent, Bpp3dRadiusPrototype, Bpp3dRadiusWeightFunction};

pub(in crate::bpp3d) fn fixture() -> (ModelArtifact, ColumnGenerationApplicationState, PreciseAssignmentSelection) {
    let meter = |value| PhysicalQuantity { value, unit: QuantityUnit::Meter };
    let source = encode_bpp3d_problem(Bpp3dProblem {
        items: vec![Bpp3dItem { id: "item".into(), name: "Item".into(), width: meter(1.0), height: meter(1.0), depth: meter(1.0),
            weight: PhysicalQuantity { value: 1.0, unit: QuantityUnit::Kilogram }, amount: "2".into(), orientations: vec![Bpp3dOrientation::Upright], shape: None }],
        bins: vec![Bpp3dBin { id: "bin".into(), type_code: "type".into(), width: meter(2.0), height: meter(1.0), depth: meter(1.0),
            capacity: PhysicalQuantity { value: 2.0, unit: QuantityUnit::Kilogram }, is_main: true }],
        initial_layers: vec![Bpp3dLayer { id: "seed".into(), bin_id: "bin".into(), depth: meter(1.0), placements: vec![
            Bpp3dPlacement { item_id: "item".into(), orientation: Bpp3dOrientation::Upright,
                position: Bpp3dPoint { x: meter(0.0), y: meter(0.0), z: meter(0.0) } },
        ] }],
        radius_component: None, package_attributes: Vec::new(),
    }).unwrap();
    let (_, request, traces) = decode_bpp3d_problem(&source).unwrap();
    let generation = LayerGenerationRequest::new(0, request.items.clone()).with_bin(request.bins[0].clone())
        .with_demand_entries(vec![LayerGenerationDemandEntry { mode: Bpp3dDemandMode::Item,
            key: Bpp3dDemandKey::Item { id: "item".into() }, demand: 2.0, satisfied: 0.0 }]);
    let generated = BlockLayerGenerator::new().generate(&generation).into_iter()
        .find(|result| result.block_traces[0].item_count == 2).unwrap();
    let layer_index = request.initial_layers.len();
    let mut layers = request.initial_layers.clone();
    layers.push(generated.layer);
    let state = ColumnGenerationApplicationState { items: request.items, bins: request.bins, initial_layers: request.initial_layers,
        layers, iteration: 1, layer_block_traces: HashMap::from([(layer_index, generated.block_traces)]),
        layer_placement_traces: HashMap::from([(0, traces[0].1.clone()), (layer_index, generated.placement_traces)]),
        continuous_radius_component: None, additional_shadow_prices: HashMap::new(), info: HashMap::new() };
    let selection = PreciseAssignmentSelection { activated_bins: vec![0], assignments: vec![LayerAssignment { bin_index: 0, layer_index }] };
    (source, state, selection)
}

#[test]
fn generated_block_pool_roundtrips_exact_traces_and_strict_replay() {
    let (source, state, selection) = fixture();
    let text = encode_bpp3d_column_pool(&source, &state).unwrap();
    assert!(text.contains("\"grid\":[\"2\",\"1\",\"1\"]"));
    assert!(text.contains("\"itemCount\":\"2\""));
    let decoded = decode_bpp3d_column_pool(&text, &source).unwrap();
    assert_eq!(encode_bpp3d_column_pool(&source, &decoded).unwrap(), text);
    assert_eq!(decoded.layers.iter().map(BinLayer::identity_key).collect::<Vec<_>>(),
        state.layers.iter().map(BinLayer::identity_key).collect::<Vec<_>>());
    let block = &decoded.layer_block_traces[&1][0];
    assert_eq!((block.nx, block.ny, block.nz, block.item_count), (2, 1, 1, 2));
    assert_eq!(decoded.layer_placement_traces[&1][0].amount, 2);
    assert_eq!(replay_strict_grouped_packing(&decoded, &selection, &[2], 4096).unwrap()[0].items.len(), 2);
}

#[test]
fn generated_pool_preserves_bad_physical_traces_without_claiming_a_valid_packing() {
    let (source, mut state, selection) = fixture();
    state.layer_block_traces.get_mut(&1).unwrap()[0].nx = 1;
    let decoded = decode_bpp3d_column_pool(&encode_bpp3d_column_pool(&source, &state).unwrap(), &source).unwrap();
    assert!(replay_strict_grouped_packing(&decoded, &selection, &[2], 4096).is_err());
    state.layer_block_traces.remove(&1);
    state.layer_placement_traces.remove(&1);
    let decoded = decode_bpp3d_column_pool(&encode_bpp3d_column_pool(&source, &state).unwrap(), &source).unwrap();
    assert!(replay_strict_grouped_packing(&decoded, &selection, &[2], 4096).unwrap_err().contains("no physical trace"));
}

#[test]
fn generated_pool_rejects_foreign_source_and_mutated_source_prefix() {
    let (source, state, _) = fixture();
    let text = encode_bpp3d_column_pool(&source, &state).unwrap();
    let mut foreign = source.clone(); foreign.fingerprint = "foreign".into();
    assert_eq!(decode_bpp3d_column_pool(&text, &foreign).unwrap_err().code, "FingerprintMismatch");
    for case in 0..8 {
        let mut altered = state.clone();
        match case {
            0 => altered.items[0].width.value = 3.0,
            1 => altered.items[0].name = "foreign".into(),
            2 => altered.bins[0].capacity.value = 3.0,
            3 => altered.initial_layers[0].depth.value = 2.0,
            4 => altered.layers[0].iteration = 1,
            5 => altered.layer_placement_traces.get_mut(&0).unwrap()[0].position.x.value = 1.0,
            6 => altered.additional_shadow_prices.insert("foreign".into(), 1.0).map(|_| ()).unwrap_or(()),
            _ => { altered.layer_block_traces.insert(0, Vec::new()); },
        }
        assert!(encode_bpp3d_column_pool(&source, &altered).is_err(), "case {case}");
    }
    let mut altered = state.clone();
    altered.layers[0].depth.value = -0.0;
    assert!(encode_bpp3d_column_pool(&source, &altered).is_err());
    let mut altered = state.clone();
    altered.layer_placement_traces.get_mut(&0).unwrap()[0].position.x.value = -0.0;
    assert!(encode_bpp3d_column_pool(&source, &altered).is_err());
}

#[test]
fn generated_pool_rejects_invalid_schema_bounds_and_noncanonical_integers() {
    let (source, state, _) = fixture();
    let text = encode_bpp3d_column_pool(&source, &state).unwrap();
    let original: serde_json::Value = serde_json::from_str(&text).unwrap();
    for case in 0..9 {
        let mut value = original.clone();
        match case {
            0 => value["schemaVersion"] = "2.0".into(),
            1 => value["sourceFingerprint"] = "foreign".into(),
            2 => value["iteration"] = "2".into(),
            3 => value["iteration"] = "01".into(),
            4 => value["generated"][0]["blocks"][0]["grid"][0] = "01".into(),
            5 => value["generated"][0]["binType"] = "foreign".into(),
            6 => value["generated"][0]["coverage"][0]["itemId"] = "foreign".into(),
            7 => value["generated"][0]["unknown"] = true.into(),
            _ => value["generated"][0]["blocks"][0]["itemCount"] = "18446744073709551616".into(),
        }
        assert!(decode_bpp3d_column_pool(&value.to_string(), &source).is_err(), "case {case}");
    }
    assert_eq!(decode_bpp3d_column_pool(&" ".repeat(MAX_BYTES + 1), &source).unwrap_err().code, "PayloadTooLarge");
}

#[test]
fn generated_pool_retains_full_width_integer_text_without_json_number_loss() {
    let (source, mut state, selection) = fixture();
    state.layer_block_traces.get_mut(&1).unwrap()[0].item_count = u64::MAX;
    let text = encode_bpp3d_column_pool(&source, &state).unwrap();
    assert!(text.contains("\"itemCount\":\"18446744073709551615\""));
    let decoded = decode_bpp3d_column_pool(&text, &source).unwrap();
    assert_eq!(decoded.layer_block_traces[&1][0].item_count, u64::MAX);
    assert!(replay_strict_grouped_packing(&decoded, &selection, &[2], 4096).is_err());
}

pub(in crate::bpp3d) fn radius_fixture() -> (ModelArtifact, ColumnGenerationApplicationState, PreciseAssignmentSelection) {
    let meter = |value| PhysicalQuantity { value, unit: QuantityUnit::Meter };
    let source = encode_bpp3d_problem(Bpp3dProblem {
        items: vec![Bpp3dItem { id: "drum".into(), name: "Drum".into(), width: meter(1.8), height: meter(1.0), depth: meter(1.8),
            weight: PhysicalQuantity { value: 0.5, unit: QuantityUnit::Kilogram }, amount: "2".into(),
            orientations: vec![Bpp3dOrientation::Upright], shape: Some(Bpp3dShape::Cylinder { axis: Bpp3dAxis::Y, radius: meter(0.5), radius_candidates: None }) },
            Bpp3dItem { id: "box".into(), name: "Box".into(), width: meter(1.0), height: meter(1.0), depth: meter(1.0),
                weight: PhysicalQuantity { value: 0.5, unit: QuantityUnit::Kilogram }, amount: "1".into(),
                orientations: vec![Bpp3dOrientation::Upright], shape: None }],
        bins: vec![Bpp3dBin { id: "bin".into(), type_code: "type".into(), width: meter(3.0), height: meter(2.0), depth: meter(3.0),
            capacity: PhysicalQuantity { value: 10.0, unit: QuantityUnit::Kilogram }, is_main: true }],
        initial_layers: vec![Bpp3dLayer { id: "seed".into(), bin_id: "bin".into(), depth: meter(1.8), placements: vec![
            Bpp3dPlacement { item_id: "drum".into(), orientation: Bpp3dOrientation::Upright,
                position: Bpp3dPoint { x: meter(0.2), y: meter(0.0), z: meter(0.0) } },
            Bpp3dPlacement { item_id: "drum".into(), orientation: Bpp3dOrientation::Upright,
                position: Bpp3dPoint { x: meter(1.0), y: meter(0.0), z: meter(0.8) } },
            Bpp3dPlacement { item_id: "box".into(), orientation: Bpp3dOrientation::Upright,
                position: Bpp3dPoint { x: meter(2.0), y: meter(0.0), z: meter(0.0) } }] }],
        radius_component: Some(Bpp3dRadiusComponent {
            prototypes: vec![Bpp3dRadiusPrototype { item_id: "drum".into(), source: "weight".into(), axis: Bpp3dAxis::Y,
                lower_bound: meter(0.4), upper_bound: meter(0.9) }],
            weight_functions: vec![Bpp3dRadiusWeightFunction { key: "weight".into(), intercept: 0.0,
                radius_squared_coefficient: 1.0, objective_weight: 1.0 }], objective: None }),
        package_attributes: Vec::new(),
    }).unwrap();
    let (_, request, traces) = decode_bpp3d_problem(&source).unwrap();
    let state = ColumnGenerationApplicationState { items: request.items.clone(), bins: request.bins.clone(),
        continuous_radius_component: request.continuous_radius_component.clone(), initial_layers: request.initial_layers.clone(),
        layers: request.initial_layers.clone(), iteration: 0, layer_block_traces: Default::default(),
        layer_placement_traces: HashMap::from([(0, traces[0].1.clone())]),
        additional_shadow_prices: Default::default(), info: Default::default() };
    let selection = PreciseAssignmentSelection { activated_bins: vec![0], assignments: vec![LayerAssignment { bin_index: 0, layer_index: 0 }] };
    (source, state, selection)
}
