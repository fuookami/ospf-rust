use ospf_rust_core::solver::{ProblemStatus, SolveFingerprints, SolveProof, SolveReport, SolveSolution, TerminationReason, linear_model_fingerprint, linear_model_mapping};
use super::*;

fn source(resources: bool, objective: bool) -> ModelArtifact {
    let problem = serde_json::from_str(r#"{"items":[{"id":"cargo","weight":{"value":1000,"unit":"g"},"required":false,"fixedPositionId":null}],"positions":[{"id":"a","maxCount":"18446744073709551615","maxWeight":{"value":2,"unit":"kg"},"fixedOccupied":false,"enabled":true},{"id":"b","maxCount":"1","maxWeight":{"value":2,"unit":"kg"},"fixedOccupied":false,"enabled":true}],"allowedAssignments":[{"itemId":"cargo","positionId":"a"},{"itemId":"cargo","positionId":"b"}],"conflicts":[],"layoutGroups":[],"weightUnit":"kg"}"#).unwrap();
    let objective = objective.then(|| serde_json::from_str(r#"{"itemCosts":{"cargo":-5},"positionCosts":{"a":1,"b":1},"layoutCosts":[]}"#).unwrap());
    let resources: Vec<super::super::BppLoadingResource> = if resources { serde_json::from_str(r#"[{"id":"ballast","positionId":"a","minimumWeight":{"value":0,"unit":"kg"},"maximumWeight":{"value":1,"unit":"kg"},"minimumPerLoadedWeight":0.5,"costPerWeight":2}]"#).unwrap() } else { vec![] };
    if resources.is_empty() { if let Some(objective) = objective { BppLoadingContext::with_objective(problem, objective).unwrap().artifact() } else { BppLoadingContext::new(problem).unwrap().artifact() } }
    else { BppLoadingContext::with_resources(problem, objective, resources).unwrap().artifact() }
}

fn options(resources: bool) -> BppLoadingPartitionOptions {
    serde_json::from_value(serde_json::json!({"partitions":[{"id":"left","positionIds":["a"]},{"id":"right","positionIds":["b"]}],"refinableResources":if resources { vec!["ballast"] } else { vec![] }})).unwrap()
}

fn values(context: &BppLoadingPartitionContext, partition: &str, loaded: bool, resource: f64) -> BTreeMap<String, f64> {
    context.decisions().iter().filter(|decision| decision.partition_id == partition).map(|decision| (decision.id.clone(), match decision.role {
        BppLoadingPartitionDecisionRole::Resource => resource, _ => if loaded { 1.0 } else { 0.0 },
    })).collect()
}

fn seed(context: &mut BppLoadingPartitionContext, loaded: bool, resource: f64) {
    let columns = ["left", "right"].into_iter().map(|partition| BppLoadingPartitionColumnInput { partition_id: partition.into(), decisions: values(context, partition, loaded && partition == "left", resource) }).collect();
    context.add_columns(columns).unwrap();
}

fn vector(master: &BppLoadingPartitionMaster) -> Vec<f64> {
    let mut values = vec![0.0; crate::decode_compiled_model(&master.artifact()).unwrap().num_variables()];
    for index in master.column_indices().values() { values[*index as usize] = 1.0; } values
}

fn certified(master: &BppLoadingPartitionMaster) -> String {
    let model = crate::decode_compiled_model(&master.artifact()).unwrap(); let mut solution = SolveSolution::vector(vector(master)); solution.objective_value = Some(0.0);
    solution.dual_solution = Some(vec![0.0; model.num_constraints()]);
    let mut report = SolveReport::builder(ProblemStatus::Feasible, TerminationReason::Completed).solution(solution).proof(SolveProof::optimality())
        .fingerprints(SolveFingerprints { model: Some(linear_model_fingerprint(&model).unwrap()), ..Default::default() }).model_mapping(Some(linear_model_mapping(&model).unwrap())).build().unwrap();
    report.statistics.nodes = Some(usize::MAX); serde_json::to_string(&report).unwrap()
}

#[test]
fn independent_registrations_keep_actual_child_identities_without_relabeling_reports() {
    let source = source(true, false);
    let mut first = BppLoadingPartitionContext::new(&source, options(true)).unwrap();
    let mut second = BppLoadingPartitionContext::new(&source, options(true)).unwrap();
    seed(&mut first, false, 0.0); seed(&mut second, false, 0.0);
    for options in [BppLoadingPartitionMasterOptions::default(),
        BppLoadingPartitionMasterOptions { phase_one: true, ..Default::default() },
        BppLoadingPartitionMasterOptions { integer: true, ..Default::default() },
        BppLoadingPartitionMasterOptions { integer: true, refine_resources: true, ..Default::default() }]
    {
        let master = first.master(options.clone()).unwrap();
        first.seed_pricing("right", false).unwrap();
        let rebuilt = second.master(options).unwrap();
        assert_eq!(serde_json::to_value(master.artifact()).unwrap(), serde_json::to_value(rebuilt.artifact()).unwrap());
    }
    let master = first.master(Default::default()).unwrap();
    let report = certified(&master);
    first.accept_lp_report(&master, &report).unwrap();
    let rebuilt = second.master(Default::default()).unwrap();
    second.accept_lp_report(&rebuilt, &report).unwrap();
    for partition in ["left", "right"] {
        assert_eq!(serde_json::to_value(first.pricing(&master, partition, false).unwrap().artifact()).unwrap(),
            serde_json::to_value(second.pricing(&rebuilt, partition, false).unwrap().artifact()).unwrap());
    }
    assert!(second.source_values(&master, &vector(&master)).is_err());
}

#[test]
fn ownership_is_exhaustive_unique_and_resources_follow_their_real_source_positions() {
    let source = source(true, true); let context = BppLoadingPartitionContext::new(&source, options(true)).unwrap();
    assert_eq!(context.decisions().len(), 5); let resource = context.decisions().iter().find(|decision| decision.resource_id.as_deref() == Some("ballast")).unwrap();
    assert_eq!(resource.partition_id, "left");
    for variant in 0..6 {
        let mut options = options(true);
        match variant { 0 => { options.partitions.pop(); }, 1 => options.partitions[1].position_ids.push("a".into()), 2 => options.partitions[0].id = "right".into(),
            3 => options.refinable_resources = vec!["cargo".into()], 4 => options.refinable_resources.push("ballast".into()), _ => options.partitions[0].position_ids = vec!["unknown".into()] }
        assert!(BppLoadingPartitionContext::new(&source, options).is_err());
    }
    assert!(serde_json::from_str::<BppLoadingPartitionOptions>(r#"{"partitions":[],"callback":true}"#).is_err());
}

#[test]
fn independently_registered_pricing_keeps_layout_auxiliary_rows_and_actual_identity() {
    let mut problem = BppLoadingContext::from_artifact(&source(false, false)).unwrap().problem().clone();
    problem.layout_groups = serde_json::from_str(r#"[{"id":"layout","positionIds":["a","b"],"layouts":[{"id":"left","positionIds":["a"]},{"id":"right","positionIds":["b"]}]}]"#).unwrap();
    let source = BppLoadingContext::new(problem).unwrap().artifact();
    let first = BppLoadingPartitionContext::new(&source, options(false)).unwrap();
    let second = BppLoadingPartitionContext::new(&source, options(false)).unwrap();
    for partition in ["left", "right"] {
        let pricing = first.seed_pricing(partition, false).unwrap();
        let rebuilt = second.seed_pricing(partition, false).unwrap();
        let model = crate::decode_compiled_model(&pricing.artifact()).unwrap();
        assert!(model.num_variables() > pricing.indices.len());
        assert_eq!(serde_json::to_value(pricing.artifact()).unwrap(), serde_json::to_value(rebuilt.artifact()).unwrap());
    }
}

#[test]
fn batch_validation_is_atomic_and_pool_changes_invalidate_old_master_but_never_reuse_column_ids() {
    let mut context = BppLoadingPartitionContext::new(&source(false, true), options(false)).unwrap(); seed(&mut context, true, 0.0);
    let master = context.master(BppLoadingPartitionMasterOptions { integer: true, ..Default::default() }).unwrap(); let values = vector(&master);
    assert_eq!(context.extract_solution(&master, &values).unwrap().loaded_count, "1"); let columns = context.columns();
    let batch = vec![BppLoadingPartitionColumnInput { partition_id: "left".into(), decisions: self::values(&context, "left", false, 0.0) },
        BppLoadingPartitionColumnInput { partition_id: "right".into(), decisions: BTreeMap::new() }];
    assert!(context.add_columns(batch).is_err()); assert_eq!(context.columns().len(), columns.len()); context.extract_solution(&master, &values).unwrap();
    context.remove_columns(vec![columns[0].id.clone()]).unwrap(); assert!(context.extract_solution(&master, &values).is_err());
    context.add_columns(vec![BppLoadingPartitionColumnInput { partition_id: "left".into(), decisions: self::values(&context, "left", false, 0.0) }]).unwrap();
    assert!(context.columns().iter().all(|column| column.id != columns[0].id));
    let foreign = BppLoadingPartitionContext::new(&source(false, true), options(false)).unwrap(); assert!(foreign.source_values(&master, &values).is_err());
}

#[test]
fn resource_completion_releases_only_whitelist_then_materializes_and_restores_frozen_final() {
    let mut context = BppLoadingPartitionContext::new(&source(true, true), options(true)).unwrap(); seed(&mut context, true, 1.0);
    let master = context.master(BppLoadingPartitionMasterOptions { integer: true, refine_resources: true, ..Default::default() }).unwrap(); let mut values = vector(&master);
    let resource = context.decisions().iter().find(|decision| decision.resource_id.is_some()).unwrap().clone();
    let binding = master.global_bindings().iter().find(|binding| binding.source_index == resource.variable_index).unwrap(); values[binding.master_index as usize] = 0.5;
    let result = context.extract_solution(&master, &values).unwrap(); assert_eq!(result.resources.unwrap().weights["ballast"].value, 0.5);
    context.materialize_completed_resources(&master, &values).unwrap(); assert_eq!(context.columns().len(), 3);
    assert!(context.source_values(&master, &values).is_err());
    let final_master = context.master(BppLoadingPartitionMasterOptions { integer: true, ..Default::default() }).unwrap(); assert!(final_master.global_bindings().is_empty());
    let mut final_values = vec![0.0; crate::decode_compiled_model(&final_master.artifact()).unwrap().num_variables()];
    for column in context.columns() {
        if column.partition_id == "right" || column.decisions.get(&resource.id) == Some(&0.5) { final_values[final_master.column_indices()[&column.id] as usize] = 1.0; }
    }
    assert_eq!(context.extract_solution(&final_master, &final_values).unwrap().loaded_count, "1");
}

#[test]
fn full_lp_certificate_issues_current_requests_and_invalid_report_revokes_old_duals() {
    let mut context = BppLoadingPartitionContext::new(&source(false, false), options(false)).unwrap(); seed(&mut context, false, 0.0);
    let master = context.master(Default::default()).unwrap();
    assert!(context.pricing(&master, "left", false).is_err()); let text = certified(&master); context.accept_lp_report(&master, &text).unwrap();
    let request = context.pricing(&master, "left", false).unwrap();
    let mut batch = BppLoadingPartitionBatch::default(); batch.push(&request, vec![values(&context, "left", true, 0.0)]).unwrap();
    assert!(context.accept_lp_report(&master, "{}").is_err()); assert!(context.add_priced_batch(&master, &batch).is_err());
    context.accept_lp_report(&master, &text).unwrap(); assert!(context.add_priced_batch(&master, &batch).is_err(), "old dual epoch cannot be revived");
    let left = context.pricing(&master, "left", false).unwrap(); let right = context.pricing(&master, "right", false).unwrap();
    let mut batch = BppLoadingPartitionBatch::default(); batch.push(&left, vec![values(&context, "left", true, 0.0)]).unwrap(); batch.push(&right, vec![]).unwrap();
    context.add_priced_batch(&master, &batch).unwrap(); assert_eq!(context.columns().len(), 3); assert!(context.add_priced_batch(&master, &batch).is_err());
}

#[test]
fn pricing_report_checks_actual_model_and_preserves_lossless_counters_and_constant() {
    let context = BppLoadingPartitionContext::new(&source(true, true), options(true)).unwrap(); let pricing = context.seed_pricing("left", false).unwrap();
    let model = crate::decode_compiled_model(&pricing.artifact()).unwrap(); let decisions = values(&context, "left", true, 0.5);
    let mut values = vec![0.0; model.num_variables()]; for (id, index) in &pricing.indices { values[*index] = decisions[id]; }
    let mut solution = SolveSolution::vector(values); solution.objective_value = Some(-3.0);
    let report = SolveReport::builder(ProblemStatus::Feasible, TerminationReason::TimeLimit).solution(solution)
        .fingerprints(SolveFingerprints { model: Some(linear_model_fingerprint(&model).unwrap()), ..Default::default() }).model_mapping(Some(linear_model_mapping(&model).unwrap())).build().unwrap();
    let text = serde_json::to_string(&report).unwrap().replace("\"nodes\":null", "\"nodes\":18446744073709551615");
    let read = pricing.read_report(&text).unwrap(); assert_eq!(read.report_json, text); assert_eq!(read.reduced_cost, Some(-3.0)); assert!(!read.optimal);
    let foreign = context.seed_pricing("right", false).unwrap(); assert!(foreign.read_report(&text).is_err());
    assert!(pricing.reduced_cost(&BTreeMap::new()).is_err());
}

#[test]
fn compiled_transport_preserves_actual_relaxation_and_rejects_changed_solver_types_or_digest() {
    let mut context = BppLoadingPartitionContext::new(&source(false, false), options(false)).unwrap(); seed(&mut context, false, 0.0);
    let master = context.master(Default::default()).unwrap(); let artifact = master.artifact();
    let decoded = crate::decode_compiled_model(&artifact).unwrap(); assert!(!decoded.has_integer_variables());
    assert_eq!(linear_model_fingerprint(&decoded).unwrap(), linear_model_fingerprint(&master.value.snapshot().compile_flt64().unwrap().model).unwrap());
    assert!(crate::decode_model(&artifact).is_err());
    for variant in 0..3 {
        let mut artifact = artifact.clone();
        if variant == 0 { artifact.fingerprint = "foreign".into(); }
        else {
            let mut compiled: serde_json::Value = serde_json::from_str(&artifact.model_json).unwrap();
            if variant == 1 { compiled["schemaVersion"] = "2.0".into(); }
            else { compiled["modelJson"] = format!("{} ", compiled["modelJson"].as_str().unwrap()).into(); }
            artifact.model_json = compiled.to_string();
        }
        assert!(crate::decode_compiled_model(&artifact).is_err());
    }
}
