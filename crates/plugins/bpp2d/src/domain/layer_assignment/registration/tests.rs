use ospf_rust_core::variable::{Continuous, VariableRange};
use ospf_rust_framework_geometry2d::{PackingShape2, Placement2, QuantityPoint2, Sheet2Shape, rectangle_shape2};
use ospf_rust_quantities::quantity::Quantity;
use crate::domain::item::model::{SheetType, SheetTypeId};
use crate::domain::layer_assignment::model::{LayerAssignmentObjectiveConfig, layer_reduced_cost};
use super::*;

fn meters(value: f64) -> Quantity<f64, Meter> { Quantity::new_ct(value) }

fn column(items: &[&str]) -> Layer<f64, Meter> {
    let sheet = SheetType::new(meters(10.0), meters(8.0), meters(0.0), SheetTypeId::new("s"));
    let units = items.iter().enumerate().map(|(index, item)| {
        Placement2::new(PackingShape2::Rectangle(rectangle_shape2(meters(2.0), meters(2.0)).unwrap()), QuantityPoint2 { x: meters(index as f64 * 2.0), y: meters(0.0) }).with_item_id(*item)
    }).collect();
    Layer::new(0, "fixture", Some(sheet), Sheet2Shape::new(meters(items.len() as f64 * 2.0), meters(2.0))).with_units(units)
}

fn register(
    model: &mut MetaModel<f64>,
    layers: &[Layer<f64, Meter>],
    demands: &[Bpp2dAssignmentDemandEntry],
    stage: LayerAssignmentStage,
) -> ospf_rust_core::Result<LayerAssignmentModel> {
    LayerAssignmentContext::default().register_model(model, LayerAssignmentModelInput { layers, demands, stage })
}

fn values(model: &MetaModel<f64>, values: &[f64]) -> BTreeMap<VariableId, f64> {
    model.tokens().iter().zip(values).map(|(token, value)| (token.id(), *value)).collect()
}

#[test]
fn common_registration_preserves_order_types_rows_and_objective() {
    let layers = [column(&["a", "a"])];
    let demands = [Bpp2dAssignmentDemandEntry::item_amount("a", 3).with_upper_bound(Bpp2dAssignmentDemandValue::Amount(5)), Bpp2dAssignmentDemandEntry::item_area("a", 12.0)];
    for stage in [LayerAssignmentStage::Rmp, LayerAssignmentStage::Integer, LayerAssignmentStage::FeasibilityLp] {
        let mut model = MetaModel::new("master");
        let registration = register(&mut model, &layers, &demands, stage).unwrap();
        assert_eq!(registration.stage(), stage);
        assert_eq!(registration.copy_variables(), &[0]);
        assert_eq!(registration.demand_rows(), &[0, 2]);
        assert_eq!(registration.upper_bound_rows(), &[Some(1), None]);
        let compiled = model.linear_model_snapshot().unwrap().compile_flt64().unwrap();
        assert_eq!(compiled.model.num_variables(), if stage == LayerAssignmentStage::FeasibilityLp { 3 } else { 1 });
        assert_eq!(model.tokens()[0].var_type().is_integer(), stage == LayerAssignmentStage::Integer);
        assert_eq!(compiled.model.c[0], match stage { LayerAssignmentStage::Rmp => 80.0, LayerAssignmentStage::Integer => 80.000001, LayerAssignmentStage::FeasibilityLp => 0.0 });
        if stage == LayerAssignmentStage::FeasibilityLp {
            assert_eq!(registration.deficit_variables(), &[1, 2]);
            let snapshot = model.linear_model_snapshot().unwrap();
            assert_eq!(snapshot.rows.len(), 3);
            assert_eq!(compiled.model.c[1], 1.0 / 3.0);
            assert_eq!(compiled.model.c[2], 1.0 / 12.0);
            assert!(registration.extract_copies(&model, &values(&model, &[0.0, 3.0, 12.0]), 1e-7).is_ok());
        } else {
            assert!(registration.deficit_variables().is_empty());
            assert!(registration.extract_copies(&model, &values(&model, &[0.0]), 1e-7).is_err());
        }
    }
}

#[test]
fn phase_one_deficits_use_original_lower_rows_and_never_relax_upper_rows() {
    let layers = [column(&["a", "b"])];
    let demands = [Bpp2dAssignmentDemandEntry::item_amount("a", 2), Bpp2dAssignmentDemandEntry::item_amount("b", 0).with_upper_bound(Bpp2dAssignmentDemandValue::Amount(1))];
    let mut model = MetaModel::new("phase-one");
    let registration = register(&mut model, &layers, &demands, LayerAssignmentStage::FeasibilityLp).unwrap();
    assert_eq!(model.constraints().len(), 3);
    assert!(registration.extract_copies(&model, &values(&model, &[1.0, 1.0, 0.0]), 1e-7).is_ok());
    assert!(registration.extract_copies(&model, &values(&model, &[2.0, 0.0, 0.0]), 1e-7).is_err());
}

#[test]
fn empty_pool_phase_one_is_valid_and_uses_normalized_deficits() {
    let mut model = MetaModel::new("empty");
    let registration = register(&mut model, &[], &[Bpp2dAssignmentDemandEntry::item_amount("a", 3)], LayerAssignmentStage::FeasibilityLp).unwrap();
    assert_eq!(registration.deficit_variables(), &[0]);
    assert_eq!(registration.extract_copies(&model, &values(&model, &[3.0]), 1e-7).unwrap(), Vec::<f64>::new());
    assert_eq!(registration.extract_deficits(&model, &values(&model, &[3.0]), 1e-7).unwrap(), vec![3.0]);
    assert!(registration.extract_deficits(&model, &BTreeMap::new(), 1e-7).is_err());
    for tolerance in [f64::NAN, -1.0, 0.5, f64::INFINITY] {
        assert!(registration.extract_deficits(&model, &values(&model, &[3.0]), tolerance).is_err());
    }
    assert_eq!(model.linear_model_snapshot().unwrap().compile_flt64().unwrap().model.c, vec![1.0 / 3.0]);
}

#[test]
fn large_mixed_demand_does_not_clip_copies_or_total_sheet_area() {
    let mut model = MetaModel::new("copies");
    let registration = register(&mut model, &[column(&["a", "b"])], &[Bpp2dAssignmentDemandEntry::item_amount("a", 100), Bpp2dAssignmentDemandEntry::item_amount("b", 1)], LayerAssignmentStage::Integer).unwrap();
    assert_eq!(registration.extract_copies(&model, &values(&model, &[100.0]), 1e-7).unwrap(), vec![100.0]);
}

#[test]
fn extraction_rejects_short_extra_foreign_fractional_nonfinite_and_infeasible_values() {
    let mut model = MetaModel::new("extract");
    let registration = register(&mut model, &[column(&["a", "a"])], &[Bpp2dAssignmentDemandEntry::item_amount("a", 3)], LayerAssignmentStage::Integer).unwrap();
    assert_eq!(registration.extract_copies(&model, &values(&model, &[2.0]), 1e-7).unwrap(), vec![2.0]);
    for value in [1.0, 1.5, -1.0, f64::NAN, f64::INFINITY] {
        assert!(registration.extract_copies(&model, &values(&model, &[value]), 1e-7).is_err());
    }
    assert!(registration.extract_copies(&model, &BTreeMap::new(), 1e-7).is_err());
    let mut extra = values(&model, &[2.0]);
    extra.insert(VariableId::new(usize::MAX, 0), 0.0);
    assert!(registration.extract_copies(&model, &extra, 1e-7).is_err());
    for tolerance in [f64::NAN, -1.0, 0.5, f64::INFINITY] {
        assert!(registration.extract_copies(&model, &values(&model, &[2.0]), tolerance).is_err());
    }
    assert!(registration.extract_copies(&MetaModel::new("foreign"), &values(&model, &[2.0]), 1e-7).is_err());
}

#[test]
fn demand_prices_include_both_bounds_preserve_area_mode_and_require_complete_duals() {
    let mut model = MetaModel::new("duals");
    let registration = register(&mut model, &[column(&["a", "a"])], &[Bpp2dAssignmentDemandEntry::item_amount("a", 3).with_upper_bound(Bpp2dAssignmentDemandValue::Amount(5)), Bpp2dAssignmentDemandEntry::item_area("a", 12.0)], LayerAssignmentStage::Rmp).unwrap();
    let prices = registration.extract_shadow_prices(&model, &[7.0, -2.0, 3.0]).unwrap();
    assert_eq!(prices[&Bpp2dDemandShadowPriceKey::new(Bpp2dAssignmentDemandMode::ItemAmount, "a")], 5.0);
    assert_eq!(prices[&Bpp2dDemandShadowPriceKey::new(Bpp2dAssignmentDemandMode::ItemArea, "a")], 3.0);
    for invalid in [vec![], vec![7.0], vec![7.0, -2.0, f64::NAN], vec![7.0, -2.0, 3.0, 4.0]] {
        assert!(registration.extract_shadow_prices(&model, &invalid).is_err());
    }
    assert!(registration.extract_shadow_prices(&MetaModel::new("foreign"), &[7.0, -2.0, 3.0]).is_err());
}

#[test]
fn extra_model_rules_can_append_without_reimplementing_registration() {
    let mut model = MetaModel::new("extension");
    let extra = model.register_auto_variable_with_range::<Continuous>("extra", VariableRange::bounded(0.0, 1.0)).unwrap();
    let registration = register(&mut model, &[column(&["a", "a"])], &[Bpp2dAssignmentDemandEntry::item_amount("a", 3)], LayerAssignmentStage::Integer).unwrap();
    model.add_ge_constraint(&[(registration.copy_variables()[0], 1.0), (extra, 1.0)], 3.0, "business_rule").unwrap();
    model.add_linear_objective_input(LinearObjectiveInput::minimize("extra_cost").term(extra, 2.0).constant(7.0));
    assert!(registration.extract_copies(&model, &values(&model, &[0.0, 2.0]), 1e-7).is_err());
    assert_eq!(registration.extract_copies(&model, &values(&model, &[0.0, 3.0]), 1e-7).unwrap(), vec![3.0]);
    let compiled = model.linear_model_snapshot().unwrap().compile_flt64().unwrap();
    assert_eq!(compiled.objective_constant, 7.0);
    assert_eq!(compiled.model.c, vec![2.0, 80.000001]);
}

#[test]
fn invalid_inputs_fail_before_mutating_the_model() {
    let good = column(&["a", "a"]);
    let mut demands = vec![vec![], vec![Bpp2dAssignmentDemandEntry::item_amount("", 1)], vec![Bpp2dAssignmentDemandEntry::item_amount("a", 1); 2], vec![Bpp2dAssignmentDemandEntry::item_amount("a", 9_007_199_254_740_993)], vec![Bpp2dAssignmentDemandEntry::item_area("a", f64::NAN)], vec![Bpp2dAssignmentDemandEntry::item_area("a", -1.0)], vec![Bpp2dAssignmentDemandEntry::item_amount("a", 2).with_upper_bound(Bpp2dAssignmentDemandValue::Amount(1))]];
    demands.push(vec![Bpp2dAssignmentDemandEntry::new(Bpp2dAssignmentDemandMode::ItemAmount, "a", Bpp2dAssignmentDemandValue::Area(1.0))]);
    demands.push(vec![Bpp2dAssignmentDemandEntry::item_area("a", 1.0).with_upper_bound(Bpp2dAssignmentDemandValue::Amount(3))]);
    demands.push(vec![Bpp2dAssignmentDemandEntry::item_area("a", f64::from_bits(1))]);
    for demand in demands {
        let mut model = MetaModel::new("invalid");
        assert!(register(&mut model, std::slice::from_ref(&good), &demand, LayerAssignmentStage::FeasibilityLp).is_err());
        assert!(model.tokens().is_empty());
        assert!(model.constraints().is_empty());
    }
    let demand = [Bpp2dAssignmentDemandEntry::item_amount("a", 1)];
    for variant in 0..12 {
        let mut bad = good.clone();
        match variant {
            0 => bad.sheet = None,
            1 => bad.units.clear(),
            2 => bad.units[0].position.x.value = f64::NAN,
            3 => bad.units[0].position.x.value = -1.0,
            4 => bad.units[1].position = bad.units[0].position.clone(),
            5 => bad.units[0].item_id = None,
            6 => bad.sheet.as_mut().unwrap().width.value = f64::INFINITY,
            7 => bad.units[0].position.x.value = 10.0,
            8 => bad.units[0].shape = PackingShape2::Rectangle(rectangle_shape2(meters(1.0), meters(2.0)).unwrap()),
            9 => bad.sheet.as_mut().unwrap().max_load.value = f64::NAN,
            10 => bad.shape.width.value = 0.0,
            _ => { bad.shape.width.value = f64::MAX; bad.shape.height.value = f64::MAX; },
        }
        let mut model = MetaModel::new("invalid-column");
        assert!(register(&mut model, &[bad], &demand, LayerAssignmentStage::Rmp).is_err(), "variant {variant}");
        assert!(model.tokens().is_empty());
    }
}

#[test]
fn objective_and_pricing_use_sheet_area_not_occupied_area() {
    let sparse = column(&["a"]);
    let dense = column(&["a", "a"]);
    assert_eq!(layer_reduced_cost(&sparse, &[], &BTreeMap::new(), 1.0), 80.0);
    assert_eq!(layer_reduced_cost(&dense, &[], &BTreeMap::new(), 1.0), 80.0);
    let demands = [Bpp2dAssignmentDemandEntry::item_amount("a", 3)];
    let mut prices = BTreeMap::new();
    prices.insert(Bpp2dDemandShadowPriceKey::new(demands[0].mode, "a"), 40.0);
    assert_eq!(layer_reduced_cost(&sparse, &demands, &prices, 1.0), 40.0);
    assert_eq!(layer_reduced_cost(&dense, &demands, &prices, 1.0), 0.0);
}

#[test]
fn invalid_objective_costs_and_overflows_are_rejected_atomically() {
    for variant in 0..4 {
        let mut config = LayerAssignmentObjectiveConfig::default();
        match variant {
            0 => config.rmp_area_coefficient = f64::NAN,
            1 => config.sheet_area_weight = -1.0,
            2 => { config.sheet_type_costs.insert(SheetTypeId::new("s"), f64::INFINITY); },
            _ => config.sheet_area_weight = f64::MAX,
        }
        let context = LayerAssignmentContext::new(config);
        let mut model = MetaModel::new("invalid-cost");
        assert!(context.register_model(&mut model, LayerAssignmentModelInput { layers: &[column(&["a"])], demands: &[Bpp2dAssignmentDemandEntry::item_amount("a", 1)], stage: LayerAssignmentStage::Integer }).is_err());
        assert!(model.tokens().is_empty());
    }
}

#[test]
fn definitions_are_checked_without_deduplicating_or_restricting_valid_rotations() {
    let mut good = column(&["a"]);
    good.units[0].shape = PackingShape2::Rectangle(rectangle_shape2(meters(1.0), meters(2.0)).unwrap());
    let demand = [Bpp2dAssignmentDemandEntry::item_amount("a", 1)];
    let mut rotated = good.clone();
    rotated.units[0].shape = rotated.units[0].shape.rotated();
    rotated.units[0].rotated = true;
    let mut model = MetaModel::new("duplicate-layouts");
    let registration = register(&mut model, &[good.clone(), good.clone(), rotated], &demand, LayerAssignmentStage::Rmp).unwrap();
    assert_eq!(registration.copy_variables(), &[0, 1, 2]);
    for sheet_conflict in [false, true] {
        let mut bad = good.clone();
        if sheet_conflict {
            bad.sheet.as_mut().unwrap().width.value = 11.0;
        } else {
            bad.units[0].shape = PackingShape2::Rectangle(rectangle_shape2(meters(1.5), meters(2.0)).unwrap());
        }
        let mut model = MetaModel::new("conflict");
        assert!(register(&mut model, &[good.clone(), bad], &demand, LayerAssignmentStage::Integer).is_err());
        assert!(model.tokens().is_empty());
    }
}

#[test]
fn extraction_includes_new_variables_and_duals_include_new_rows() {
    let mut model = MetaModel::new("extended-lp");
    let registration = register(&mut model, &[column(&["a"])], &[Bpp2dAssignmentDemandEntry::item_amount("a", 1)], LayerAssignmentStage::Rmp).unwrap();
    let old = values(&model, &[1.0]);
    let extra = model.register_auto_variable_with_range::<Continuous>("extra", VariableRange::bounded(0.0, 1.0)).unwrap();
    model.add_eq_constraint(&[(extra, 1.0)], 1.0, "extra_row").unwrap();
    assert!(registration.extract_copies(&model, &old, 1e-7).is_err());
    assert_eq!(registration.extract_copies(&model, &values(&model, &[1.0, 1.0]), 1e-7).unwrap(), vec![1.0]);
    assert!(registration.extract_shadow_prices(&model, &[80.0]).is_err());
    assert!(registration.extract_shadow_prices(&model, &[80.0, 0.0]).is_ok());
}

#[cfg(any(feature = "scip", feature = "scip-bundled", feature = "scip-from-source", feature = "gurobi10", feature = "gurobi11", feature = "gurobi12"))]
fn real_backend_contract(solver: &dyn ospf_rust_core::solver::LinearSolver) {
    use std::time::Duration;
    use ospf_rust_core::solver::{ProblemStatus, SolveOptions, linear_model_fingerprint, linear_model_mapping, require_infeasibility_certificate_for_model, require_optimal_lp_certificate_for_linear_model};

    let layers = [column(&["a", "a"])];
    for (stage, area_mode, expected) in [
        (LayerAssignmentStage::Rmp, false, 1.5),
        (LayerAssignmentStage::Rmp, true, 1.5),
        (LayerAssignmentStage::Integer, false, 2.0),
        (LayerAssignmentStage::Integer, true, 2.0),
        (LayerAssignmentStage::FeasibilityLp, false, 1.5),
    ] {
        let demands = [if area_mode { Bpp2dAssignmentDemandEntry::item_area("a", 12.0) } else { Bpp2dAssignmentDemandEntry::item_amount("a", 3) }];
        let mut model = MetaModel::new("real-master");
        let registration = register(&mut model, &layers, &demands, stage).unwrap();
        let snapshot = model.linear_model_snapshot().unwrap();
        let compiled = snapshot.compile_flt64().unwrap();
        let report = solver.solve_linear_report_with_options(&compiled.model, &SolveOptions { time_limit: Some(Duration::from_secs(15)), ..Default::default() }).unwrap();
        report.validate().unwrap();
        assert!(report.is_optimal(), "{stage:?}: {report:?}");
        assert_eq!(report.fingerprints.model, Some(linear_model_fingerprint(&compiled.model).unwrap()));
        assert_eq!(report.model_mapping, Some(linear_model_mapping(&compiled.model).unwrap()));
        let solution = report.solution.as_ref().unwrap();
        let original_values = compiled.model.basic.variables.iter().zip(&solution.values).map(|(token, value)| (token.id(), *value)).collect();
        let copies = registration.extract_copies(&model, &original_values, 1e-7).unwrap();
        assert!((copies[0] - expected).abs() < 1e-7, "{stage:?}: {copies:?}");
        snapshot.evaluate_objective_flt64(&original_values).unwrap().validate_reported(solution.objective_value.or(solution.objective), 1e-7).unwrap();
        if stage != LayerAssignmentStage::Integer {
            let certificate = require_optimal_lp_certificate_for_linear_model(&report, &compiled.model).unwrap();
            let mut by_source = BTreeMap::<&str, f64>::new();
            for (row, dual) in compiled.rows.iter().zip(certificate.dual) { *by_source.entry(&row.source).or_default() += f64::from(row.multiplier) * dual; }
            let duals = snapshot.rows.iter().map(|row| by_source[row.id.as_str()]).collect::<Vec<_>>();
            let prices = registration.extract_shadow_prices(&model, &duals).unwrap();
            let rc = layer_reduced_cost(&layers[0], &demands, &prices, if stage == LayerAssignmentStage::FeasibilityLp { 0.0 } else { 1.0 });
            assert!(rc.abs() < 1e-7, "{stage:?}: {rc}");
            if stage == LayerAssignmentStage::FeasibilityLp { assert!(solution.objective_value.or(solution.objective).unwrap().abs() < 1e-7); }
        }
    }

    // 空池先获得真实 Phase I 对偶；实际候选按零原列成本产生负检验数。
    // An empty pool first obtains actual Phase I duals; an actual candidate has negative reduced cost at zero real-column cost.
    let mut model = MetaModel::new("real-empty-phase");
    let demands = [Bpp2dAssignmentDemandEntry::item_amount("a", 3)];
    let registration = register(&mut model, &[], &demands, LayerAssignmentStage::FeasibilityLp).unwrap();
    let compiled = model.linear_model_snapshot().unwrap().compile_flt64().unwrap();
    let report = solver.solve_linear_report_with_options(&compiled.model, &SolveOptions { time_limit: Some(Duration::from_secs(15)), ..Default::default() }).unwrap();
    let certificate = require_optimal_lp_certificate_for_linear_model(&report, &compiled.model).unwrap();
    assert!((report.solution.as_ref().unwrap().objective_value.or(report.solution.as_ref().unwrap().objective).unwrap() - 1.0).abs() < 1e-7);
    let prices = registration.extract_shadow_prices(&model, &[f64::from(compiled.rows[0].multiplier) * certificate.dual[0]]).unwrap();
    assert!(layer_reduced_cost(&layers[0], &demands, &prices, 0.0) < -1e-7);

    // 相同三件上下界允许 LP 的 1.5 份，但两件一列的整数池不可行。
    // Matching three-piece lower/upper bounds permit 1.5 LP copies, but the two-piece integer pool is infeasible.
    for stage in [LayerAssignmentStage::Rmp, LayerAssignmentStage::Integer] {
        let mut model = MetaModel::new("bounded-master");
        let registration = register(&mut model, &layers, &[Bpp2dAssignmentDemandEntry::item_amount("a", 3).with_upper_bound(Bpp2dAssignmentDemandValue::Amount(3))], stage).unwrap();
        let compiled = model.linear_model_snapshot().unwrap().compile_flt64().unwrap();
        let report = solver.solve_linear_report_with_options(&compiled.model, &SolveOptions { time_limit: Some(Duration::from_secs(15)), ..Default::default() }).unwrap();
        if stage == LayerAssignmentStage::Integer {
            assert_eq!(report.problem_status, ProblemStatus::Infeasible);
            require_infeasibility_certificate_for_model(&report, &linear_model_fingerprint(&compiled.model).unwrap()).unwrap();
        } else {
            require_optimal_lp_certificate_for_linear_model(&report, &compiled.model).unwrap();
            let values = values(&model, &report.solution.as_ref().unwrap().values);
            assert_eq!(registration.extract_copies(&model, &values, 1e-7).unwrap(), vec![1.5]);
        }
    }

    let mut model = MetaModel::new("real-extra");
    let registration = register(&mut model, &layers, &demands, LayerAssignmentStage::Integer).unwrap();
    model.add_ge_constraint(&[(registration.copy_variables()[0], 1.0)], 3.0, "extra_rule").unwrap();
    model.add_linear_objective_input(LinearObjectiveInput::minimize("constant").constant(7.0));
    let compiled = model.linear_model_snapshot().unwrap().compile_flt64().unwrap();
    let report = solver.solve_linear_report_with_options(&compiled.model, &SolveOptions { time_limit: Some(Duration::from_secs(15)), ..Default::default() }).unwrap();
    assert!(report.is_optimal());
    assert_eq!(compiled.objective_constant, 7.0);
    assert_eq!(registration.extract_copies(&model, &values(&model, &report.solution.as_ref().unwrap().values), 1e-7).unwrap(), vec![3.0]);

    for stage in [LayerAssignmentStage::FeasibilityLp, LayerAssignmentStage::Integer] {
        let mut model = MetaModel::new("real-mixed-bounds");
        let mixed = [column(&["a", "b"])];
        let demands = if stage == LayerAssignmentStage::FeasibilityLp {
            vec![Bpp2dAssignmentDemandEntry::item_amount("a", 2), Bpp2dAssignmentDemandEntry::item_area("b", 0.0).with_upper_bound(Bpp2dAssignmentDemandValue::Area(4.0))]
        } else {
            vec![Bpp2dAssignmentDemandEntry::item_amount("a", 100), Bpp2dAssignmentDemandEntry::item_area("b", 4.0)]
        };
        let registration = register(&mut model, &mixed, &demands, stage).unwrap();
        let snapshot = model.linear_model_snapshot().unwrap();
        let compiled = snapshot.compile_flt64().unwrap();
        let report = solver.solve_linear_report_with_options(&compiled.model, &SolveOptions { time_limit: Some(Duration::from_secs(15)), ..Default::default() }).unwrap();
        assert!(report.is_optimal(), "{stage:?}: {report:?}");
        let original_values = compiled.model.basic.variables.iter().zip(&report.solution.as_ref().unwrap().values).map(|(token, value)| (token.id(), *value)).collect();
        let copies = registration.extract_copies(&model, &original_values, 1e-7).unwrap();
        assert_eq!(copies, if stage == LayerAssignmentStage::FeasibilityLp { vec![1.0] } else { vec![100.0] });
        if stage == LayerAssignmentStage::FeasibilityLp {
            require_optimal_lp_certificate_for_linear_model(&report, &compiled.model).unwrap();
            assert!((report.solution.as_ref().unwrap().objective_value.or(report.solution.as_ref().unwrap().objective).unwrap() - 0.5).abs() < 1e-7);
        }
    }
}

#[cfg(any(feature = "scip", feature = "scip-bundled", feature = "scip-from-source"))]
#[test]
fn real_scip_master_registration() {
    use ospf_rust_core::solver::solvers::scip::{SCIPConfig, SCIPSolver};
    real_backend_contract(&SCIPSolver::with_config(SCIPConfig::default().with_output(false).with_lp_subproblem_defaults()));
}

#[cfg(any(feature = "gurobi10", feature = "gurobi11", feature = "gurobi12"))]
#[test]
fn real_gurobi_master_registration() {
    use ospf_rust_core::solver::solvers::gurobi::{GurobiConfig, GurobiSolver};
    real_backend_contract(&GurobiSolver::with_config(GurobiConfig::default().with_output(false)));
}
