use super::*;
use crate::{Geometry2dPoint, typescript_declarations};

pub(crate) fn meters(value: f64) -> PhysicalQuantity { PhysicalQuantity { value, unit: QuantityUnit::Meter } }

pub(crate) fn layer(amount: usize) -> Bpp2dLayer {
    Bpp2dLayer { sheet_id: "s".into(), envelope: Geometry2dSheet { width: meters(amount as f64), height: meters(1.0) },
        placements: (0..amount).map(|index| Geometry2dPlacement { shape: Geometry2dShape::Rectangle { width: meters(1.0), height: meters(1.0) },
            position: Geometry2dPoint { x: meters(index as f64), y: meters(0.0) }, rotated: false, item_id: Some("a".into()) }).collect() }
}

pub(crate) fn problem() -> Bpp2dProblem {
    Bpp2dProblem { items: vec![Bpp2dItem { id: "a".into(), name: "A".into(), shape: Geometry2dShape::Rectangle { width: meters(1.0), height: meters(1.0) },
        allow_rotate: false, generation_count: "3".into() }], sheets: vec![Bpp2dSheetType { id: "s".into(), geometry: Geometry2dSheet { width: meters(2.0), height: meters(1.0) }, is_main: true }],
        demands: vec![Bpp2dDemand::Count { item_id: "a".into(), count: "3".into(), upper_bound: None }], initial_layers: vec![layer(1)], kerf: meters(0.0), objective: Default::default() }
}

#[test]
fn canonical_source_binds_units_identity_and_preserves_duplicate_initial_positions() {
    let mut source = problem();
    source.items[0].shape = Geometry2dShape::Rectangle { width: PhysicalQuantity { value: 100.0, unit: QuantityUnit::Centimeter }, height: meters(1.0) };
    source.initial_layers.push(source.initial_layers[0].clone());
    source.demands.push(Bpp2dDemand::Area { item_id: "a".into(), area: PhysicalQuantity { value: 30_000.0, unit: QuantityUnit::SquareCentimeter }, upper_bound: Some(PhysicalQuantity { value: 4.0, unit: QuantityUnit::SquareMeter }) });
    let artifact = encode_bpp2d_problem(source).unwrap();
    let (_, input) = decode_bpp2d_problem(&artifact).unwrap();
    assert_eq!(input.request.existing_layers.len(), 2);
    assert_eq!(input.request.items[0].0.shape.bounding_area().value, 1.0);
    assert_eq!(input.demands[1].demand, Bpp2dAssignmentDemandValue::Area(3.0));
    let mut changed = artifact.clone(); changed.model_json.push(' ');
    assert_eq!(decode_bpp2d_problem(&changed).unwrap_err().code, "FingerprintMismatch");
    changed = artifact.clone(); changed.fingerprint = "foreign".into();
    assert_eq!(decode_bpp2d_problem(&changed).unwrap_err().code, "FingerprintMismatch");
    changed = artifact; changed.model_json = changed.model_json.replacen("\"1.0\"", "\"2.0\"", 1);
    assert_eq!(decode_bpp2d_problem(&changed).unwrap_err().code, "UnsupportedProtocolVersion");
}

#[test]
fn source_rejects_unsafe_counts_duplicate_catalogs_foreign_references_and_invalid_units() {
    for variant in 0..18 {
        let mut source = problem();
        match variant {
            0 => source.items.push(source.items[0].clone()),
            1 => source.sheets.push(source.sheets[0].clone()),
            2 => source.items[0].generation_count = "4097".into(),
            3 => source.items[0].generation_count = "01".into(),
            4 => source.demands[0] = Bpp2dDemand::Count { item_id: "a".into(), count: "9007199254740992".into(), upper_bound: None },
            5 => source.demands[0] = Bpp2dDemand::Count { item_id: "a".into(), count: "3".into(), upper_bound: Some("2".into()) },
            6 => source.demands.push(source.demands[0].clone()),
            7 => source.initial_layers[0].sheet_id = "foreign".into(),
            8 => source.initial_layers[0].placements[0].item_id = Some("foreign".into()),
            9 => {
                source.items[0].shape = Geometry2dShape::Rectangle { width: meters(1.0), height: meters(0.5) };
                source.initial_layers[0].placements[0].shape = Geometry2dShape::Rectangle { width: meters(0.5), height: meters(1.0) };
                source.initial_layers[0].placements[0].rotated = true;
            },
            10 => source.initial_layers[0].envelope.width = meters(0.5),
            11 => source.initial_layers[0].placements[0].position.x = meters(2.0),
            12 => source.kerf = meters(-1.0),
            13 => source.kerf.unit = QuantityUnit::SquareMeter,
            14 => source.objective.sheet_type_costs.insert("foreign".into(), 1.0).map(|_| ()).unwrap_or(()),
            15 => source.items[0].shape = Geometry2dShape::Circle { radius: meters(0.0) },
            16 => source.items[0].shape = Geometry2dShape::Rectangle { width: meters(0.00001), height: meters(0.00001) },
            _ => source.demands.push(Bpp2dDemand::Area { item_id: "a".into(), area: meters(3.0), upper_bound: None }),
        }
        assert!(encode_bpp2d_problem(source).is_err(), "variant {variant}");
    }
    let mut source = problem(); source.initial_layers = vec![layer(2)]; source.kerf = meters(0.01);
    assert!(encode_bpp2d_problem(source).is_err());
    let mut source = problem(); source.demands[0] = Bpp2dDemand::Count { item_id: "a".into(), count: "9007199254740991".into(), upper_bound: None };
    assert!(encode_bpp2d_problem(source).is_ok());
    let mut source = problem(); source.items[0].generation_count = "4096".into();
    assert!(encode_bpp2d_problem(source).is_ok());
}

#[test]
fn source_protocol_rejects_unknown_fields_and_generates_matching_camel_case_types() {
    let mut artifact = encode_bpp2d_problem(problem()).unwrap();
    artifact.model_json = artifact.model_json.replacen("\"schemaVersion\"", "\"unknown\":true,\"schemaVersion\"", 1);
    assert!(decode_bpp2d_problem(&artifact).is_err());
    let declarations = typescript_declarations();
    assert!(declarations.contains("generationCount: string"));
    let demand = Bpp2dDemand::decl();
    assert!(demand.contains("itemId: string"), "{demand}");
    assert!(demand.contains("upperBound:"), "{demand}");
}
