fn selection_fixture() -> (PreciseAssignment<f64, Meter>, Vec<f64>) {
    let bin = BinType { width: meters(10.0), height: meters(10.0), depth: meters(10.0), capacity: meters(100.0), type_code: "same-type".into(), is_main: true };
    let layer = BinLayer { iteration: 0, from: "physical".into(), bin: Some(bin.clone()), depth: meters(1.0), demand_coverage: Vec::new() };
    let mut assignment = PreciseAssignment { bins: vec![bin.clone(), bin.clone(), bin], layers: vec![layer.clone(), layer], x: None, v: None,
        load_weight_symbols: Vec::new(), load_volume_symbols: Vec::new(), load_depth_symbols: Vec::new() };
    let mut model = MetaModel::new("precise-selection"); assignment.register(&mut model).unwrap();
    let mut solution = vec![0.0; model.num_tokens()];
    for bin in 0..3 { solution[assignment.v.as_ref().unwrap().model_index(&bin).unwrap()] = 1.0; }
    for (bin, layer) in [(0, 0), (0, 1), (1, 0)] {
        solution[assignment.x.as_ref().unwrap().model_index(&bin, &layer).unwrap()] = 1.0;
    }
    (assignment, solution)
}

#[test]
fn precise_selection_retains_cross_bin_multiplicity_and_empty_activated_bins() {
    let (assignment, solution) = selection_fixture();
    let selection = assignment.extract_selection(&solution, 1e-7).unwrap();
    assert_eq!(selection.activated_bins, vec![0, 1, 2]);
    assert_eq!(selection.assignments, vec![LayerAssignment { bin_index: 0, layer_index: 0 }, LayerAssignment { bin_index: 0, layer_index: 1 }, LayerAssignment { bin_index: 1, layer_index: 0 }]);
    assert_eq!(assignment.extract_selection(&solution, 1e-7).unwrap(), selection);
}

#[test]
fn precise_selection_rejects_partial_nonbinary_nonfinite_inactive_and_unregistered_inputs() {
    let (mut assignment, solution) = selection_fixture();
    assert!(assignment.extract_selection(&solution[..1], 1e-7).is_err());
    let index = assignment.x.as_ref().unwrap().model_index(&0, &0).unwrap();
    for value in [f64::NAN, f64::INFINITY, 0.25, -1.0, 2.0] {
        let mut malformed = solution.clone(); malformed[index] = value;
        assert!(assignment.extract_selection(&malformed, 1e-7).is_err());
    }
    for tolerance in [f64::NAN, f64::INFINITY, -1.0, 0.5] { assert!(assignment.extract_selection(&solution, tolerance).is_err()); }
    let mut inactive = solution.clone(); inactive[assignment.v.as_ref().unwrap().model_index(&0).unwrap()] = 0.0;
    assert!(assignment.extract_selection(&inactive, 1e-7).unwrap_err().contains("inactive"));
    assignment.x = None; assert!(assignment.extract_selection(&solution, 1e-7).unwrap_err().contains("not registered"));
}

#[test]
fn precise_selection_accepts_only_explicit_binary_tolerance() {
    let (assignment, mut solution) = selection_fixture();
    let index = assignment.x.as_ref().unwrap().model_index(&0, &0).unwrap();
    solution[index] = 1.0 - 5e-8;
    assert!(assignment.extract_selection(&solution, 1e-7).is_ok());
    assert!(assignment.extract_selection(&solution, 1e-9).is_err());
}
