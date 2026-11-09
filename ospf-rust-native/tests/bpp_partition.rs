//! 实际分区 LP、定价、资源补全及受限 final 使用原已编译模型执行协议。
//! Actual partition LP, pricing, resource completion and restricted final use the original compiled-model execution protocol.

#![cfg(any(feature = "scip", feature = "gurobi10", feature = "gurobi11", feature = "gurobi12"))]

use std::time::{Duration, Instant};
use ospf_rust_bridge::*;
use ospf_rust_native::NativeEngine;

fn run(backend: NativeBackend) {
    let problem = serde_json::from_str(r#"{"items":[{"id":"cargo","weight":{"value":1000,"unit":"g"},"required":false,"fixedPositionId":null}],"positions":[{"id":"a","maxCount":"18446744073709551615","maxWeight":{"value":2,"unit":"kg"},"fixedOccupied":false,"enabled":true},{"id":"b","maxCount":"1","maxWeight":{"value":2,"unit":"kg"},"fixedOccupied":false,"enabled":true}],"allowedAssignments":[{"itemId":"cargo","positionId":"a"},{"itemId":"cargo","positionId":"b"}],"conflicts":[],"layoutGroups":[],"weightUnit":"kg"}"#).unwrap();
    let objective = serde_json::from_str(r#"{"itemCosts":{"cargo":-5},"positionCosts":{"a":1,"b":1},"layoutCosts":[]}"#).unwrap();
    let resources = serde_json::from_str(r#"[{"id":"ballast","positionId":"a","minimumWeight":{"value":0,"unit":"kg"},"maximumWeight":{"value":1,"unit":"kg"},"minimumPerLoadedWeight":0.5,"costPerWeight":2}]"#).unwrap();
    let source = BppLoadingContext::with_resources(problem, Some(objective), resources).unwrap().artifact();
    let options: BppLoadingPartitionOptions = serde_json::from_str(r#"{"partitions":[{"id":"left","positionIds":["a"]},{"id":"right","positionIds":["b"]}],"refinableResources":["ballast"]}"#).unwrap();
    let mut context = BppLoadingPartitionContext::new(&source, options.clone()).unwrap(); let engine = NativeEngine::new(1); let mut stages = Vec::new();
    let seed = ["left", "right"].into_iter().map(|partition| BppLoadingPartitionColumnInput { partition_id: partition.into(), decisions: context.decisions().iter().filter(|decision| decision.partition_id == partition)
        .map(|decision| (decision.id.clone(), if partition == "left" { 1.0 } else { 0.0 })).collect() }).collect::<Vec<_>>();
    context.add_columns(seed.clone()).unwrap();
    let mut execute = |kind: &str, model: ModelArtifact| {
        let request = serde_json::from_value(serde_json::json!({"protocolVersion":1,"solver":backend.id(),"modelKind":"compiledLinear","model":model,"timeLimitMs":10_000})).unwrap();
        let job = engine.submit(request).unwrap(); let deadline = Instant::now() + Duration::from_secs(15);
        let view = loop { let view = engine.get(&job.id).unwrap(); if view.status != JobStatus::Running { break view; } assert!(Instant::now() < deadline); std::thread::sleep(Duration::from_millis(1)); };
        assert_eq!(view.status, JobStatus::Completed, "{kind}: {:?}", view.error); assert_eq!(view.fingerprint, model.fingerprint);
        let report = view.report_json.unwrap(); assert!(view.progress_json.is_some());
        assert_eq!(engine.cancel(&job.id).unwrap().report_json.as_deref(), Some(report.as_str())); engine.release(&job.id).unwrap();
        stages.push(serde_json::json!({"kind":kind,"model":model,"reportJson":report})); report
    };
    let master = context.master(Default::default()).unwrap(); let lp = execute("lp", master.artifact()); context.accept_lp_report(&master, &lp).unwrap();
    let mut batch = BppLoadingPartitionBatch::default();
    for partition in ["left", "right"] {
        let pricing = context.pricing(&master, partition, false).unwrap(); let report = execute(partition, pricing.artifact());
        let result = pricing.read_report(&report).unwrap(); assert!(result.optimal); assert_eq!(result.report_json, report);
        batch.push(&pricing, result.decisions.into_iter().collect()).unwrap();
    }
    context.add_priced_batch(&master, &batch).unwrap(); assert!(context.add_priced_batch(&master, &batch).is_err());
    let refinement = context.master(BppLoadingPartitionMasterOptions { integer: true, refine_resources: true, ..Default::default() }).unwrap();
    let report = execute("refinement", refinement.artifact()); let values = summarize_report(&report).unwrap().values;
    context.extract_solution(&refinement, &values).unwrap(); context.materialize_completed_resources(&refinement, &values).unwrap();
    let final_master = context.master(BppLoadingPartitionMasterOptions { integer: true, ..Default::default() }).unwrap();
    let report = execute("final", final_master.artifact()); let values = summarize_report(&report).unwrap().values;
    let result = context.extract_solution(&final_master, &values).unwrap(); assert_eq!(result.loaded_count, "1");
    let objective = summarize_report(&report).unwrap().objective_value.unwrap();
    let source_model = decode_model(&BppLoadingContext::from_artifact(&source).unwrap().application_model().unwrap()).unwrap();
    let full = context.source_values(&final_master, &values).unwrap();
    let evaluated = source_model.c.iter().zip(full).map(|(coefficient, value)| coefficient * value).sum::<f64>();
    assert!((objective - evaluated).abs() < 1e-7); assert!(objective <= -2.0 + 1e-7 && objective >= -4.0 - 1e-7);
    if let Some(directory) = std::env::var_os("OSPF_BPP_LOADING_FIXTURE_DIR") {
        let directory = std::path::PathBuf::from(directory); std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(directory.join(format!("bpp-partition-{}.json", backend.id())), serde_json::to_string(&serde_json::json!({"source":source,"options":options,"seed":seed,"stages":stages,"resultJson":serde_json::to_string(&result).unwrap()})).unwrap()).unwrap();
    }
}

#[cfg(feature = "scip")]
#[test]
fn real_scip_roundtrips_actual_partition_lp_pricing_and_resource_completion() { run(NativeBackend::Scip); }

#[cfg(any(feature = "gurobi10", feature = "gurobi11", feature = "gurobi12"))]
#[test]
fn real_gurobi_roundtrips_actual_partition_lp_pricing_and_resource_completion() { run(NativeBackend::Gurobi); }

fn run_application(backend: NativeBackend, finishing: bool) {
    let engine = NativeEngine::new(1); let mut fixtures = Vec::new();
    let cases = if finishing { vec![(false, 64, 10_000, false), (true, 1, 10_000, false), (true, 64, 10_000, true), (true, 64, 2, false)] }
        else { vec![(false, 64, 10_000, false), (true, 64, 10_000, false), (true, 1, 10_000, false), (true, 64, 10_000, true), (true, 64, 1, false)] };
    for (resources, iterations, time, cancel) in cases {
        let problem = serde_json::from_str(r#"{"items":[{"id":"cargo","weight":{"value":1000,"unit":"g"},"required":false,"fixedPositionId":null}],"positions":[{"id":"a","maxCount":"18446744073709551615","maxWeight":{"value":2,"unit":"kg"},"fixedOccupied":false,"enabled":true},{"id":"b","maxCount":"1","maxWeight":{"value":2,"unit":"kg"},"fixedOccupied":false,"enabled":true}],"allowedAssignments":[{"itemId":"cargo","positionId":"a"},{"itemId":"cargo","positionId":"b"}],"conflicts":[],"layoutGroups":[],"weightUnit":"kg"}"#).unwrap();
        let objective = serde_json::from_str(r#"{"itemCosts":{"cargo":-5},"positionCosts":{"a":1,"b":1},"layoutCosts":[]}"#).unwrap();
        let extra = if resources { serde_json::from_str(r#"[{"id":"ballast","positionId":"a","minimumWeight":{"value":0,"unit":"kg"},"maximumWeight":{"value":1,"unit":"kg"},"minimumPerLoadedWeight":0.5,"costPerWeight":2}]"#).unwrap() } else { vec![] };
        let source = if resources { BppLoadingContext::with_resources(problem, Some(objective), extra) }
            else { BppLoadingContext::with_objective(problem, objective) }.unwrap().artifact();
        let mut request: SolveRequest = serde_json::from_value(serde_json::json!({"protocolVersion":1,"solver":"bpp-loading","modelKind":"bppLoading","model":source,"timeLimitMs":time,
            "bppLoadingOptions":{"backend":backend,"partition":{"partition":{"partitions":[{"id":"left","positionIds":["a"]},{"id":"right","positionIds":["b"]}],"refinableResources":if resources { vec!["ballast"] } else { vec![] }},"iterationLimit":iterations}}})).unwrap();
        if finishing {
            for reserve_time_ms in [0, time, time + 1] {
                let mut invalid = request.clone(); invalid.bpp_loading_options.as_mut().unwrap().partition.as_mut().unwrap().source_milp_finishing = Some(BppLoadingSourceFinishingOptions { reserve_time_ms });
                assert_eq!(engine.submit(invalid).unwrap_err().code, "InvalidInput");
            }
            request.bpp_loading_options.as_mut().unwrap().partition.as_mut().unwrap().source_milp_finishing = Some(BppLoadingSourceFinishingOptions { reserve_time_ms: if time <= 2 { 1 } else { 1000 } });
        }
        let job = engine.submit(request).unwrap(); if cancel { engine.cancel(&job.id).unwrap(); } let deadline = Instant::now() + Duration::from_secs(20);
        let view = loop { let view = engine.get(&job.id).unwrap(); if view.status != JobStatus::Running { break view; } assert!(Instant::now() < deadline); std::thread::sleep(Duration::from_millis(1)); };
        assert_eq!(view.status, JobStatus::Completed, "{:?}", view.error);
        let text = view.report_json.unwrap(); let report = decode_bpp_loading_partition(&text, &source).unwrap();
        let acceleration = report.envelope.certification.as_ref().map(|evidence| evidence.previous_application_report_json.as_str()).unwrap_or(&report.envelope.application_report_json);
        let original = decode_bpp_loading_application(acceleration, &source).unwrap();
        let core: serde_json::Value = serde_json::from_str(&original.envelope.report_json).unwrap(); assert!(core["proof"].is_null());
        assert_eq!(report.envelope.certification.is_some(), finishing);
        if finishing && !cancel && time > 2 {
            assert!(report.envelope.certification.as_ref().unwrap().report_json.is_some());
            assert_eq!(report.application.envelope.objective, Some(-4.0));
            let core: ospf_rust_core::solver::SolveReport<f64, u64> = serde_json::from_str(&report.application.envelope.report_json).unwrap(); assert!(core.is_optimal());
        }
        if !cancel && time > 2 { assert!(report.application.solution.is_some()); assert!(view.progress_json.is_some()); assert!(!report.envelope.calls.is_empty()); }
        if cancel { assert!(matches!(report.application.summary.termination_reason.as_str(), "CANCELLED" | "COMPLETED")); }
        assert_eq!(engine.cancel(&job.id).unwrap().report_json.as_deref(), Some(text.as_str())); engine.release(&job.id).unwrap();
        fixtures.push(serde_json::json!({"source":source,"partitionReportJson":text,"resultJson":report.application.solution.map(|plan| serde_json::to_string(&plan).unwrap())}));
    }
    if let Some(directory) = std::env::var_os("OSPF_BPP_LOADING_FIXTURE_DIR") {
        std::fs::write(std::path::PathBuf::from(directory).join(format!("bpp-partition-{}-{}.json", if finishing { "finishing" } else { "application" }, backend.id())), serde_json::to_string(&fixtures).unwrap()).unwrap();
    }
}

#[cfg(feature = "scip")]
#[test]
fn real_scip_partition_application_retains_source_proof_boundaries_and_clock_free_replay() { run_application(NativeBackend::Scip, false); }

#[cfg(any(feature = "gurobi10", feature = "gurobi11", feature = "gurobi12"))]
#[test]
fn real_gurobi_partition_application_retains_source_proof_boundaries_and_clock_free_replay() { run_application(NativeBackend::Gurobi, false); }

#[cfg(feature = "scip")]
#[test]
fn real_scip_partition_finishing_uses_fresh_full_source_and_retains_separate_root_evidence() { run_application(NativeBackend::Scip, true); }

#[cfg(any(feature = "gurobi10", feature = "gurobi11", feature = "gurobi12"))]
#[test]
fn real_gurobi_partition_finishing_uses_fresh_full_source_and_retains_separate_root_evidence() { run_application(NativeBackend::Gurobi, true); }
