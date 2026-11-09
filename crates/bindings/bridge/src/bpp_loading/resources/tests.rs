use ospf_rust_core::solver::{ProblemStatus, SolveFingerprints, SolveReport, SolveSolution, TerminationReason, linear_model_fingerprint, linear_model_mapping};
use crate::{BppLoadingContext, QuantityUnit, decode_model};
use super::*;
use super::super::tests::{problem, solution};

fn resource() -> BppLoadingResource {
    BppLoadingResource { id: "ballast".into(), position_id: "p".into(), minimum_weight: PhysicalQuantity { value: 0.0, unit: QuantityUnit::Kilogram },
        maximum_weight: PhysicalQuantity { value: 1000.0, unit: QuantityUnit::Gram }, minimum_per_loaded_weight: 0.125, cost_per_weight: 2.0 }
}

#[test]
fn continuous_resources_use_original_capacity_rows_and_rebuild_both_model_identities() {
    let context = BppLoadingContext::with_resources(problem(), None, vec![resource()]).unwrap();
    let restored = BppLoadingContext::from_artifact(&context.artifact()).unwrap();
    assert_eq!(context.linear_model().model_json, restored.linear_model().model_json);
    assert_eq!(context.application_model().unwrap().model_json, restored.application_model().unwrap().model_json);
    let model = decode_model(&context.linear_model()).unwrap(); assert_eq!(model.num_variables(), 10);
    assert!(!model.var_types[9].is_integer()); assert_eq!(model.lb[9], 0.0); assert_eq!(model.ub[9], 1.0); assert_eq!(model.c[9], 2.0);
    assert!(context.solution_values(&solution()).is_err());
    let input = BTreeMap::from([("ballast".into(), PhysicalQuantity { value: 500.0, unit: QuantityUnit::Gram })]);
    let values = context.solution_values_with_resources(&solution(), &input).unwrap(); assert_eq!(values[9], 0.5);
    let result = context.extract_solution(&values).unwrap(); assert_eq!(result.loaded_weight.value, 8.0);
    let resources = result.resources.unwrap(); assert_eq!(resources.weights["ballast"].value, 0.5);
    assert_eq!(resources.position_total_weights["p"].value, 4.5); assert_eq!(resources.position_total_weights["r"].value, 4.0);
    assert_eq!(resources.total_weight.value, 8.5);
    let ordinary = BppLoadingContext::new(problem()).unwrap();
    assert!(!serde_json::to_string(&ordinary.verify_solution(&solution()).unwrap()).unwrap().contains("resources"));
    assert!(!ordinary.artifact().model_json.contains("resources"));
}

#[test]
fn resource_units_and_costs_have_explicit_model_weight_basis() {
    let mut problem = problem(); problem.weight_unit = QuantityUnit::Gram;
    let mut resource = resource(); resource.minimum_weight.value = 0.25; resource.cost_per_weight = 0.002;
    let context = BppLoadingContext::with_resources(problem, None, vec![resource]).unwrap();
    let input = BTreeMap::from([("ballast".into(), PhysicalQuantity { value: 0.5, unit: QuantityUnit::Kilogram })]);
    let values = context.solution_values_with_resources(&solution(), &input).unwrap(); assert_eq!(values[9], 500.0);
    let model = decode_model(&context.linear_model()).unwrap(); assert_eq!(model.c[9], 0.002);
    let result = context.verify_solution_with_resources(&solution(), &input).unwrap().resources.unwrap();
    assert_eq!(result.total_weight.value, 8500.0); assert_eq!(result.total_weight.unit, QuantityUnit::Gram);
}

#[test]
fn invalid_resource_catalog_ranges_units_and_unrepresentable_coefficients_are_rejected() {
    for variant in 0..15 {
        let mut resource = resource(); let mut resources = Vec::new();
        match variant {
            0 => resource.id.clear(), 1 => resource.position_id = "unknown".into(), 2 => resource.minimum_weight.value = -1.0,
            3 => resource.maximum_weight.value = 0.0, 4 => resource.maximum_weight.unit = QuantityUnit::Meter,
            5 => resource.cost_per_weight = f64::INFINITY, 6 => resource.minimum_per_loaded_weight = -1.0,
            7 => resource.minimum_per_loaded_weight = f64::NAN, 8 => resource.minimum_weight = PhysicalQuantity { value: f64::from_bits(1), unit: QuantityUnit::Gram },
            9 => resource.minimum_per_loaded_weight = f64::MAX,
            10 => resources.push(resource.clone()), 11 => { let mut other = resource.clone(); other.id = "other".into(); resources.push(other); },
            12 => { let mut other = resource.clone(); other.position_id = "r".into(); resources.push(other); },
            13 => resources.resize(128, resource.clone()), _ => resource.maximum_weight.value = f64::NAN,
        }
        if variant == 3 { resource.minimum_weight.value = 1.0; }
        resources.push(resource);
        assert!(BppLoadingContext::with_resources(problem(), None, resources).is_err(), "variant {variant}");
    }
    assert!(BppLoadingContext::with_resources(problem(), None, vec![]).is_err());
}

#[test]
fn complete_plans_cannot_omit_resources_or_bypass_coupling_capacity_and_occupancy() {
    let context = BppLoadingContext::with_resources(problem(), None, vec![resource()]).unwrap();
    for (name, weight, unit) in [("ballast", 0.49, QuantityUnit::Kilogram), ("ballast", 2.0, QuantityUnit::Kilogram),
        ("unknown", 0.5, QuantityUnit::Kilogram), ("ballast", f64::NAN, QuantityUnit::Kilogram), ("ballast", 0.5, QuantityUnit::Meter)] {
        assert!(context.verify_solution_with_resources(&solution(), &BTreeMap::from([(name.into(), PhysicalQuantity { value: weight, unit })])).is_err());
    }
    let mut input = problem(); input.items.clear(); input.allowed_assignments.clear();
    let context = BppLoadingContext::with_resources(input, None, vec![resource()]).unwrap();
    let mut plan = solution(); plan.assignments.clear(); plan.occupied_positions.clear();
    assert!(context.verify_solution_with_resources(&plan, &BTreeMap::from([("ballast".into(), PhysicalQuantity { value: 0.5, unit: QuantityUnit::Kilogram })])).is_err());
    let mut input = problem(); input.positions[0].max_weight.value = 4.25;
    let context = BppLoadingContext::with_resources(input, None, vec![resource()]).unwrap();
    assert!(context.verify_solution_with_resources(&solution(), &BTreeMap::from([("ballast".into(), PhysicalQuantity { value: 0.5, unit: QuantityUnit::Kilogram })])).is_err());
}

#[test]
fn original_reports_validate_continuous_values_costs_and_resource_source_identity() {
    let context = BppLoadingContext::with_resources(problem(), None, vec![resource()]).unwrap();
    let model = decode_model(&context.linear_model()).unwrap();
    let mut solution = SolveSolution::vector(context.solution_values_with_resources(&solution(),
        &BTreeMap::from([("ballast".into(), PhysicalQuantity { value: 0.5, unit: QuantityUnit::Kilogram })])).unwrap());
    solution.objective_value = Some(1.0);
    let report = SolveReport::builder(ProblemStatus::Feasible, TerminationReason::Completed).solution(solution)
        .fingerprints(SolveFingerprints { model: Some(linear_model_fingerprint(&model).unwrap()), ..Default::default() })
        .model_mapping(Some(linear_model_mapping(&model).unwrap())).build().unwrap();
    let json = serde_json::to_string(&report).unwrap().replace("\"nodes\":null", "\"nodes\":18446744073709551615");
    let read = context.read_report(&json).unwrap(); assert_eq!(read.json, json); assert_eq!(read.solution.unwrap().resources.unwrap().total_weight.value, 8.5);
    let mut resource = resource(); resource.id = "other".into();
    let other = BppLoadingContext::with_resources(problem(), None, vec![resource]).unwrap();
    assert_eq!(other.read_report(&json).err().unwrap().code, "FingerprintMismatch");
    let mut report = report; report.solution.as_mut().unwrap().values[9] = 0.0;
    assert!(context.read_report(&serde_json::to_string(&report).unwrap()).is_err());
}
