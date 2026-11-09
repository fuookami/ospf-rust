use ospf_rust_core::variable::Continuous;
use ospf_rust_framework_geometry2d::{PackingShape2, Placement2, QuantityPoint2, rectangle_shape2};
use crate::domain::material::{Material2, MaterialId2, Pattern2, PatternId2, ProductId2};
use crate::domain::produce::Csp2dPatternAssignment;
use crate::infrastructure::{CutLine2, CutOrientation2, CutTree2, CuttingProcess2, Region2};
use super::*;

fn meters(value: f64) -> Quantity<f64, Meter> { Quantity::new_ct(value) }

fn plan(amount: usize) -> CuttingPlan2<f64, Meter> {
    let material = Material2::sheet(MaterialId2::new("s"), "sheet", meters(4.0), meters(2.0), 10);
    let shape = PackingShape2::Rectangle(rectangle_shape2(meters(1.0), meters(1.0)).unwrap());
    CuttingPlan2::from_pattern(Pattern2::new(PatternId2::new("p"), material,
        Region2::new(meters(0.0), meters(0.0), meters(4.0), meters(2.0)),
        (0..amount).map(|index| Placement2::new(shape.clone(), QuantityPoint2 { x: meters(index as f64), y: meters(0.0) }).with_item_id("a")).collect(), CuttingProcess2::Freeform))
}

fn master(plans: Vec<CuttingPlan2<f64, Meter>>) -> Csp2dMasterProblem {
    let mut master = Csp2dMasterProblem { assignment: Csp2dPatternAssignment { usage_upper_bounds: vec![1; plans.len()], plans },
        demands: vec![Csp2dMasterDemand::new(ProductId2::new("a"), DemandValue2::Amount(3))], material_availability: BTreeMap::from([("s".into(), 10)]) };
    master.demands.push(Csp2dMasterDemand::new(ProductId2::new("a"), DemandValue2::Area(3.0)));
    master
}

fn register(
    model: &mut MetaModel<f64>,
    master: &Csp2dMasterProblem,
    stage: ProduceStage,
) -> ospf_rust_core::Result<ProduceModel> {
    ProduceContext::default().register_model(model, ProduceModelInput { master, stage, kerf: &meters(0.0) })
}

fn values(model: &MetaModel<f64>, values: &[f64]) -> BTreeMap<VariableId, f64> {
    model.tokens().iter().zip(values).map(|(token, &value)| (token.id(), value)).collect()
}

#[test]
fn registration_shares_actual_demand_and_stock_rows_without_heuristic_clipping() {
    let mut master = master(vec![plan(2), plan(2)]);
    master.demands[0].upper_bound = Some(DemandValue2::Amount(5));
    for stage in [ProduceStage::Rmp, ProduceStage::Integer, ProduceStage::FeasibilityLp] {
        let mut model = MetaModel::new("native");
        let registered = register(&mut model, &master, stage).unwrap();
        assert_eq!(registered.usage_variables(), &[0, 1]);
        assert_eq!(registered.demand_rows(), &[0, 2]);
        assert_eq!(registered.upper_bound_rows(), &[Some(1), None]);
        assert_eq!(registered.material_rows()["s"], 3);
        assert_eq!(model.tokens()[0].var_type().is_integer(), stage == ProduceStage::Integer);
        let compiled = model.linear_model_snapshot().unwrap().compile_flt64().unwrap();
        assert_eq!(compiled.model.c[0], if stage == ProduceStage::FeasibilityLp { 0.0 } else { 8.0 });
        let mut supplied = vec![2.0, 0.0];
        if stage == ProduceStage::FeasibilityLp { supplied.extend([0.0, 0.0]); }
        assert_eq!(registered.extract_usages(&model, &values(&model, &supplied), 1e-7).unwrap(), vec![2.0, 0.0]);
    }
}

#[test]
fn empty_phase_one_uses_original_units_and_never_relaxes_inventory_or_demand_caps() {
    let empty = master(Vec::new());
    let mut model = MetaModel::new("phase");
    let registered = register(&mut model, &empty, ProduceStage::FeasibilityLp).unwrap();
    assert_eq!(registered.deficit_variables(), &[0, 1]);
    assert_eq!(registered.extract_deficits(&model, &values(&model, &[3.0, 3.0]), 1e-7).unwrap(), vec![3.0, 3.0]);
    assert!(registered.extract_deficits(&model, &values(&model, &[2.0, 3.0]), 1e-7).is_err());
    let mut master = master(vec![plan(2)]);
    master.demands[0].upper_bound = Some(DemandValue2::Amount(1));
    assert!(register(&mut MetaModel::new("bad"), &master, ProduceStage::FeasibilityLp).is_err());
    master.demands[0].upper_bound = None;
    master.assignment.plans[0].pattern.material.available_batches = 1;
    master.material_availability.insert("s".into(), 1);
    let mut model = MetaModel::new("stock");
    let registration = register(&mut model, &master, ProduceStage::FeasibilityLp).unwrap();
    assert!(registration.extract_usages(&model, &values(&model, &[2.0, 0.0, 0.0]), 1e-7).is_err());
    assert_eq!(registration.extract_deficits(&model, &values(&model, &[1.0, 1.0, 1.0]), 1e-7).unwrap(), vec![1.0, 1.0]);
}

#[test]
fn extraction_keeps_identity_complete_values_extra_rules_and_basis_aware_prices() {
    let master = master(vec![plan(2)]);
    let mut model = MetaModel::new("registered");
    let registration = register(&mut model, &master, ProduceStage::Rmp).unwrap();
    model.register_auto_variable_with_range::<Continuous>("extra", VariableRange::bounded(0.0, 1.0)).unwrap();
    model.add_le_constraint(&[(0, 1.0)], 2.0, "business").unwrap();
    let supplied = values(&model, &[1.5, 0.0]);
    assert_eq!(registration.extract_usages(&model, &supplied, 1e-7).unwrap(), vec![1.5]);
    assert!(registration.extract_usages(&model, &values(&model, &[1.5]), 1e-7).is_err());
    assert!(registration.extract_usages(&model, &values(&model, &[2.5, 0.0]), 1e-7).is_err());
    assert!(registration.extract_usages(&MetaModel::new("foreign"), &supplied, 1e-7).is_err());
    for tolerance in [f64::NAN, -1.0, 0.5] { assert!(registration.extract_usages(&model, &supplied, tolerance).is_err()); }
    let prices = registration.extract_shadow_prices(&model, &[1.0, 2.0, -3.0, 0.0]).unwrap();
    assert_eq!(prices.demands.len(), 2);
    assert_eq!(prices.demands[&Csp2dDemandShadowPriceKey { product_id: "a".into(), mode: DemandMode2::Amount }], 1.0);
    assert_eq!(prices.demands[&Csp2dDemandShadowPriceKey { product_id: "a".into(), mode: DemandMode2::Area }], 2.0);
    assert_eq!(prices.materials["s"], -3.0);
    assert!(registration.extract_shadow_prices(&model, &[0.0]).is_err());
}

#[test]
fn reduced_cost_keeps_distinct_demand_bases_upper_bounds_stock_rows_and_phase_one_costs() {
    let mut master = master(vec![plan(2)]);
    master.demands[0].upper_bound = Some(DemandValue2::Amount(5));
    let mut model = MetaModel::new("prices");
    let registration = register(&mut model, &master, ProduceStage::Rmp).unwrap();
    let prices = registration.extract_shadow_prices(&model, &[4.0, -1.0, 5.0, -2.0]).unwrap();
    let context = ProduceContext::default();
    for (stage, expected) in [(ProduceStage::Rmp, -6.0), (ProduceStage::FeasibilityLp, -14.0)] {
        assert_eq!(context.reduced_cost(ProducePricingInput { candidate: &master.assignment.plans[0], demands: &master.demands, prices: &prices, stage }).unwrap(), expected);
    }
    let mut incomplete = prices.clone(); incomplete.materials.clear();
    assert!(context.reduced_cost(ProducePricingInput { candidate: &master.assignment.plans[0], demands: &master.demands, prices: &incomplete, stage: ProduceStage::Rmp }).is_err());
    incomplete = prices.clone(); incomplete.demands.clear();
    assert!(context.reduced_cost(ProducePricingInput { candidate: &master.assignment.plans[0], demands: &master.demands, prices: &incomplete, stage: ProduceStage::Rmp }).is_err());
    assert!(context.reduced_cost(ProducePricingInput { candidate: &master.assignment.plans[0], demands: &master.demands, prices: &prices, stage: ProduceStage::Integer }).is_err());
}

#[test]
fn coil_height_and_partial_sheet_regions_keep_original_consumption_semantics() {
    let mut coil = plan(2);
    coil.pattern.material.height = None; coil.pattern.material.length = Some(meters(5.0));
    let mut master = master(vec![coil]);
    let context = ProduceContext { objective: ProduceObjective { area_weight: 1.0, waste_weight: 2.0, height_weight: 3.0, usage_weight: 4.0 } };
    let mut model = MetaModel::new("coil");
    context.register_model(&mut model, ProduceModelInput { master: &master, stage: ProduceStage::Integer, kerf: &meters(0.0) }).unwrap();
    assert_eq!(model.linear_model_snapshot().unwrap().compile_flt64().unwrap().model.c[0], 30.0);
    master.assignment.plans[0].pattern.region.height = meters(6.0);
    assert!(register(&mut MetaModel::new("long"), &master, ProduceStage::Integer).is_err());
}

#[test]
fn invalid_values_conflicting_catalogs_and_unbound_rules_fail_without_model_mutation() {
    for variant in 0..17 {
        let mut master = master(vec![plan(2)]);
        match variant {
            0 => master.demands[0].demand = DemandValue2::Amount(9_007_199_254_740_992),
            1 => master.demands[1].demand = DemandValue2::Area(f64::NAN),
            2 => master.demands.push(master.demands[0].clone()),
            3 => master.demands[0].upper_bound = Some(DemandValue2::Area(3.0)),
            4 => master.assignment.plans[0].pattern.placements[1].position.x = meters(0.5),
            5 => master.assignment.plans[0].pattern.placements[0].position.x = meters(f64::INFINITY),
            6 => master.assignment.plans[0].pattern.placements[0].item_id = None,
            7 => master.material_availability.insert("s".into(), 9).map(|_| ()).unwrap_or(()),
            8 => master.assignment.plans[0].pattern.material.unit_weight = Some(meters(1.0)),
            9 => { let costar = master.assignment.plans[0].pattern.placements[0].clone(); master.assignment.plans[0].pattern.costar_placements.push(costar); },
            10 => master.assignment.plans[0].pattern.region.height = meters(3.0),
            11 => master.assignment.plans[0].pattern.placements[1].shape = PackingShape2::Rectangle(rectangle_shape2(meters(2.0), meters(1.0)).unwrap()),
            12 => master.assignment.usage_upper_bounds.clear(),
            13 => master.assignment.plans[0].pattern.process = CuttingProcess2::GuillotineTwoStage,
            15 => master.assignment.plans[0].pattern.material.length = Some(meters(3.0)),
            16 => { master.assignment.plans[0].pattern.material.width = meters(f64::MAX); master.assignment.plans[0].pattern.material.height = Some(meters(f64::MAX)); },
            _ => master.assignment.plans[0].pattern.material.available_batches = 0,
        }
        let mut model = MetaModel::new("invalid");
        assert!(register(&mut model, &master, ProduceStage::Integer).is_err(), "variant {variant}");
        assert!(model.tokens().is_empty() && model.constraints().is_empty());
    }
    let master = master(vec![plan(2)]);
    for kerf in [-1.0, f64::NAN, 0.01] {
        assert!(ProduceContext::default().register_model(&mut MetaModel::new("kerf"), ProduceModelInput { master: &master, stage: ProduceStage::Rmp, kerf: &meters(kerf) }).is_err());
    }
    let context = ProduceContext { objective: ProduceObjective { area_weight: f64::MAX, ..Default::default() } };
    assert!(context.register_model(&mut MetaModel::new("cost"), ProduceModelInput { master: &master, stage: ProduceStage::Rmp, kerf: &meters(0.0) }).is_err());
}

#[test]
fn positive_kerf_cut_tree_reuses_physical_spacing_without_charging_whole_sheets() {
    let mut plan = plan(2);
    plan.pattern.region.width = meters(2.25); plan.pattern.region.height = meters(1.0);
    plan.pattern.placements[1].position.x = meters(1.25);
    plan.pattern.process = CuttingProcess2::GuillotineTwoStage;
    plan.pattern.cut_tree = Some(CutTree2::cut(CutLine2 { orientation: CutOrientation2::Vertical, position: meters(1.0), stage: 1 },
        plan.pattern.region.clone(), meters(0.25), CutTree2::leaf(Region2::new(meters(0.0), meters(0.0), meters(1.0), meters(1.0)), Some("a".into())),
        CutTree2::leaf(Region2::new(meters(1.25), meters(0.0), meters(1.0), meters(1.0)), Some("a".into()))).unwrap());
    let mut master = master(vec![plan]);
    let mut model = MetaModel::new("kerf-tree");
    ProduceContext::default().register_model(&mut model, ProduceModelInput { master: &master, stage: ProduceStage::Rmp, kerf: &meters(0.25) }).unwrap();
    assert_eq!(model.linear_model_snapshot().unwrap().compile_flt64().unwrap().model.c[0], 2.25);
    master.assignment.plans[0].pattern.placements[1].position.x = meters(1.125);
    assert!(ProduceContext::default().register_model(&mut MetaModel::new("narrow-gap"),
        ProduceModelInput { master: &master, stage: ProduceStage::Rmp, kerf: &meters(0.25) }).is_err());
}

#[test]
fn guillotine_requires_a_coherent_physical_leaf_bijection_not_only_a_valid_cut_tree() {
    let mut plan = plan(2);
    plan.pattern.region.width = meters(2.0); plan.pattern.region.height = meters(1.0);
    plan.pattern.process = CuttingProcess2::GuillotineTwoStage;
    plan.pattern.cut_tree = Some(CutTree2::cut(CutLine2 { orientation: CutOrientation2::Vertical, position: meters(1.0), stage: 1 },
        plan.pattern.region.clone(), meters(0.0), CutTree2::leaf(Region2::new(meters(0.0), meters(0.0), meters(1.0), meters(1.0)), Some("a".into())),
        CutTree2::leaf(Region2::new(meters(1.0), meters(0.0), meters(1.0), meters(1.0)), Some("a".into()))).unwrap());
    let master = master(vec![plan]);
    register(&mut MetaModel::new("tree"), &master, ProduceStage::Integer).unwrap();
    let mut tampered = master.clone();
    if let Some(CutTree2::Cut(node)) = tampered.assignment.plans[0].pattern.cut_tree.as_mut() {
        if let CutTree2::Leaf(leaf) = node.lower.as_mut() { leaf.part_id = None; }
    }
    assert!(register(&mut MetaModel::new("omitted"), &tampered, ProduceStage::Integer).is_err());
    tampered = master.clone();
    if let Some(CutTree2::Cut(node)) = tampered.assignment.plans[0].pattern.cut_tree.as_mut() {
        if let CutTree2::Leaf(leaf) = node.lower.as_mut() { leaf.region.x = meters(1e-10); }
    }
    assert!(tampered.assignment.plans[0].validate(&meters(0.0)).is_ok());
    assert!(register(&mut MetaModel::new("shifted"), &tampered, ProduceStage::Integer).is_err());
    let mut tree = CutTree2::leaf(master.assignment.plans[0].pattern.region.clone(), None);
    for _ in 0..130 {
        tree = CutTree2::Cut(crate::infrastructure::CutNode2 { line: CutLine2 { orientation: CutOrientation2::Vertical, position: meters(1.0), stage: 1 },
            region: master.assignment.plans[0].pattern.region.clone(), kerf: meters(0.0), split: None,
            lower: Box::new(CutTree2::leaf(master.assignment.plans[0].pattern.region.clone(), None)), upper: Box::new(tree) });
    }
    tampered.assignment.plans[0].pattern.cut_tree = Some(tree);
    assert!(register(&mut MetaModel::new("deep"), &tampered, ProduceStage::Integer).unwrap_err().to_string().contains("depth/node"));
}
