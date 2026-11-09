use crate::domain::layer_assignment::{LayerAssignment, PreciseAssignmentSelection};

struct StrictReplayFixture {
    selection: PreciseAssignmentSelection,
    bins: Vec<BinType<f64, Meter>>,
    layers: Vec<BinLayer<f64, Meter>>,
    items: Vec<ActualItem<f64, Meter>>,
    blocks: HashMap<usize, Vec<LayerBlockTrace<f64, Meter>>>,
    traces: HashMap<usize, Vec<LayerPlacementTrace<f64, Meter>>>,
}

impl StrictReplayFixture {
    fn new() -> Self {
        let bin = make_bin_type();
        let layer = BinLayer { iteration: 0, from: "strict-test".into(), bin: Some(bin.clone()), depth: meters(2.0), demand_coverage: Vec::new() };
        let item = make_cuboid_packed_item(0, 0.0, 0.0, 0.0, 2.0, 2.0, 2.0).item;
        let trace = LayerPlacementTrace { item_index: 0, item_id: item.id.clone(), position: MetricPoint3 { x: meters(0.0), y: meters(0.0), z: meters(0.0) }, orientation: Orientation::Upright, amount: 1 };
        Self { selection: PreciseAssignmentSelection { activated_bins: vec![0, 1, 2], assignments: vec![
            LayerAssignment { bin_index: 0, layer_index: 0 }, LayerAssignment { bin_index: 0, layer_index: 1 }, LayerAssignment { bin_index: 1, layer_index: 0 }] },
            bins: vec![bin.clone(), bin.clone(), bin], layers: vec![layer.clone(), layer], items: vec![item], blocks: HashMap::new(),
            traces: HashMap::from([(0, vec![trace.clone()]), (1, vec![trace])]) }
    }

    fn request(&self) -> LayerAssignmentTraceReplayRequest<'_, f64, Meter> {
        LayerAssignmentTraceReplayRequest { selection: &self.selection, layers: &self.layers, bins: &self.bins, items: &self.items,
            block_traces: &self.blocks, placement_traces: &self.traces, max_placements: 100, extra_geometry: None }
    }

    fn replay(&self) -> Result<Vec<PackedBin<f64, Meter>>, Vec<String>> { LayerTraceReplayAdapter::new().replay_assignments(self.request()) }
}

#[test]
fn strict_replay_preserves_real_bin_assignments_depth_offsets_multiplicity_and_empty_bins() {
    let fixture = StrictReplayFixture::new(); let packed = fixture.replay().unwrap();
    assert_eq!(packed.len(), 3); assert_eq!(packed[0].name, "assigned-bin-0");
    assert_eq!(packed[0].items.len(), 2); assert_eq!(packed[0].items[1].position.z.value, 2.0);
    assert_eq!(packed[1].items.len(), 1); assert_eq!(packed[1].items[0].position.z.value, 0.0);
    assert!(packed[2].items.is_empty());
    assert_eq!(fixture.traces[&1][0].position.z.value, 0.0);
}

#[test]
fn strict_replay_requires_complete_physical_evidence_and_never_uses_legacy_coverage_fallback() {
    let mut fixture = StrictReplayFixture::new(); fixture.traces.remove(&1);
    fixture.layers[1].demand_coverage = vec![crate::domain::item::Bpp3dLayerDemandCoverage::new(
        crate::domain::item::Bpp3dDemandMode::Item, Bpp3dDemandKey::Item { id: fixture.items[0].id.clone() }, 1.0)];
    assert!(fixture.replay().unwrap_err()[0].contains("coverage fallback forbidden"));
    let legacy = LayerTraceReplayAdapter::new().replay_selected_layers(&fixture.layers[1..], &[1], &fixture.items, &fixture.bins, &fixture.blocks, &fixture.traces);
    assert_eq!(legacy.packed_bins.len(), 1);
}

#[test]
fn strict_replay_rejects_duplicate_unknown_inactive_and_wrong_bin_assignments() {
    let mut fixture = StrictReplayFixture::new();
    fixture.selection.assignments.push(fixture.selection.assignments[0]); assert!(fixture.replay().is_err()); fixture.selection.assignments.pop();
    fixture.selection.assignments[0].layer_index = 99; assert!(fixture.replay().is_err()); fixture.selection.assignments[0].layer_index = 0;
    fixture.selection.activated_bins.pop(); fixture.selection.activated_bins.pop(); assert!(fixture.replay().is_err()); fixture.selection.activated_bins = vec![0, 1, 2];
    fixture.layers[0].bin.as_mut().unwrap().type_code = "foreign".into(); assert!(fixture.replay().is_err());
    fixture.layers[0].bin = Some(fixture.bins[0].clone()); fixture.layers[0].bin.as_mut().unwrap().width = meters(9.0); assert!(fixture.replay().is_err());
}

#[test]
fn strict_replay_rejects_trace_identity_orientation_nonfinite_negative_and_truncated_counts() {
    let mut fixture = StrictReplayFixture::new();
    fixture.traces.get_mut(&0).unwrap()[0].item_id = "foreign".into(); assert!(fixture.replay().is_err());
    fixture.traces.get_mut(&0).unwrap()[0].item_id = fixture.items[0].id.clone();
    fixture.traces.get_mut(&0).unwrap()[0].orientation = Orientation::Side; assert!(fixture.replay().is_err());
    fixture.traces.get_mut(&0).unwrap()[0].orientation = Orientation::Upright;
    for value in [f64::NAN, f64::INFINITY, -1.0] { fixture.traces.get_mut(&0).unwrap()[0].position.x = meters(value); assert!(fixture.replay().is_err()); }
    fixture.traces.get_mut(&0).unwrap()[0].position.x = meters(0.0);
    fixture.traces.get_mut(&0).unwrap()[0].amount = 2; assert!(fixture.replay().unwrap_err()[0].contains("explicit positions"));
    fixture.traces.get_mut(&0).unwrap()[0].amount = 26; assert!(fixture.replay().unwrap_err()[0].contains("explicit positions"));
    fixture.traces.get_mut(&0).unwrap()[0].amount = u64::MAX; assert!(fixture.replay().is_err());
    fixture.traces.get_mut(&0).unwrap()[0].amount = 0; assert!(fixture.replay().is_err());
}

#[test]
fn strict_replay_checks_full_block_grid_and_matching_placement_summaries() {
    let mut fixture = StrictReplayFixture::new(); let trace = fixture.traces[&0][0].clone();
    fixture.blocks.insert(0, vec![LayerBlockTrace { block_index: 0, item_index: 0, item_id: trace.item_id, orientation: trace.orientation,
        nx: 1, ny: 1, nz: 1, item_count: 1, size: MetricSize3 { width: meters(2.0), height: meters(2.0), depth: meters(2.0) }, origin: trace.position }]);
    assert!(fixture.replay().is_ok());
    fixture.traces.get_mut(&0).unwrap()[0].amount = 2; assert!(fixture.replay().unwrap_err()[0].contains("summaries"));
    fixture.traces.remove(&0); fixture.blocks.get_mut(&0).unwrap()[0].item_count = 2; assert!(fixture.replay().is_err());
    fixture.blocks.get_mut(&0).unwrap()[0].item_count = 1; fixture.blocks.get_mut(&0).unwrap()[0].size.width = meters(3.0);
    assert!(fixture.replay().unwrap_err()[0].contains("dimensions"));
    fixture.blocks.get_mut(&0).unwrap()[0].size.width = meters(4.0); fixture.blocks.get_mut(&0).unwrap()[0].nx = 2;
    assert!(fixture.replay().unwrap_err()[0].contains("every item position"));
    fixture.blocks.get_mut(&0).unwrap()[0].item_count = 2; assert!(fixture.replay().is_ok());
    fixture.blocks.get_mut(&0).unwrap()[0].nx = u64::MAX; fixture.blocks.get_mut(&0).unwrap()[0].ny = 2; assert!(fixture.replay().is_err());
}

#[test]
fn strict_replay_checks_limits_layer_depth_overlap_and_extra_geometry_transactionally() {
    let mut fixture = StrictReplayFixture::new(); let mut request = fixture.request(); request.max_placements = 2;
    assert!(LayerTraceReplayAdapter::new().replay_assignments(request).is_err());
    fixture.layers[1].depth = meters(9.0); assert!(fixture.replay().is_err()); fixture.layers[1].depth = meters(2.0);
    let duplicate = fixture.traces[&0][0].clone(); fixture.traces.get_mut(&0).unwrap().push(duplicate);
    assert!(fixture.replay().unwrap_err()[0].contains("overlap")); fixture.traces.get_mut(&0).unwrap().pop();
    #[derive(Debug)] struct BusinessGate;
    impl PackingGeometryContract<f64, Meter> for BusinessGate {
        fn validate(&self, bin: &PackedBin<f64, Meter>) -> Result<(), Vec<String>> {
            if bin.name == "assigned-bin-1" { Err(vec!["business gate".into()]) } else { Ok(()) }
        }
    }
    let mut request = fixture.request(); request.extra_geometry = Some(&BusinessGate);
    assert_eq!(LayerTraceReplayAdapter::new().replay_assignments(request).unwrap_err(), vec!["business gate"]);
}

#[test]
fn geometry_guard_rejects_nonfinite_negative_and_degenerate_coordinates_and_dimensions() {
    for value in [f64::NAN, f64::INFINITY, -1.0] {
        let item = make_cuboid_packed_item(0, value, 0.0, 0.0, 1.0, 1.0, 1.0);
        let bin = PackedBin { name: "invalid".into(), bin_type: make_bin_type(), batch_no: None, items: vec![item] };
        assert!(PackingGeometryGuard::validate(&bin).is_err());
    }
    for value in [f64::NAN, f64::INFINITY, 0.0, -1.0] {
        let mut bin = PackedBin { name: "invalid".into(), bin_type: make_bin_type(), batch_no: None,
            items: vec![make_cuboid_packed_item(0, 0.0, 0.0, 0.0, value, 1.0, 1.0)] };
        assert!(PackingGeometryGuard::validate(&bin).is_err()); bin.items.clear(); bin.bin_type.width = meters(value);
        assert!(PackingGeometryGuard::validate(&bin).is_err());
    }
}
