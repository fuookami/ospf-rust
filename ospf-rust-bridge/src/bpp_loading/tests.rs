use ospf_rust_core::solver::{ProblemStatus, SolveFingerprints, SolveProof, SolveReport, SolveSolution, TerminationReason, linear_model_fingerprint, linear_model_mapping};
use super::*;

pub(crate) fn problem() -> BppLoadingProblem {
    serde_json::from_str(r#"{"items":[{"id":"a","weight":{"value":4000,"unit":"g"},"required":true,"fixedPositionId":null},{"id":"b","weight":{"value":4,"unit":"kg"},"required":true,"fixedPositionId":null},{"id":"c","weight":{"value":0,"unit":"kg"},"required":false,"fixedPositionId":null}],"positions":[{"id":"p","maxCount":"18446744073709551615","maxWeight":{"value":5,"unit":"kg"},"fixedOccupied":false,"enabled":true},{"id":"q","maxCount":"1","maxWeight":{"value":6,"unit":"kg"},"fixedOccupied":false,"enabled":true},{"id":"r","maxCount":"1","maxWeight":{"value":5,"unit":"kg"},"fixedOccupied":false,"enabled":true}],"allowedAssignments":[{"itemId":"a","positionId":"p"},{"itemId":"a","positionId":"q"},{"itemId":"b","positionId":"r"},{"itemId":"c","positionId":"p"}],"conflicts":[{"first":"p","second":"q"},{"first":"q","second":"r"}],"layoutGroups":[{"id":"g","positionIds":["p","q","r"],"layouts":[{"id":"outer","positionIds":["p","r"]},{"id":"middle","positionIds":["q"]}]}],"weightUnit":"kg"}"#).unwrap()
}

/// 由区间自动补出的互斥边：同空间严格重叠才算，贴边与跨空间都不算。
/// Conflict edges derived from intervals: only a strict overlap inside one space counts; touching edges and
/// different spaces do not.
#[test]
fn intervals_add_only_strict_same_space_conflicts() {
    let mut problem = problem();
    problem.conflicts.clear();
    problem.intervals = Some(vec![
        BppLoadingInterval { id: "p".into(), space_id: "deck".into(), start: quantity(0.0), end: quantity(2.0) },
        BppLoadingInterval { id: "q".into(), space_id: "deck".into(), start: quantity(1.0), end: quantity(3.0) },
        // 与 p 贴边（终点 2.0 等于起点 2.0），不构成冲突。 / Touching p at 2.0, so no conflict.
        BppLoadingInterval { id: "r".into(), space_id: "deck".into(), start: quantity(2.0), end: quantity(4.0) },
    ]);
    let conflicts = problem.domain().unwrap().conflicts;
    let pairs = conflicts.iter().map(|edge| (edge.first.clone(), edge.second.clone())).collect::<Vec<_>>();
    assert!(pairs.iter().any(|(a, b)| (a == "p" && b == "q") || (a == "q" && b == "p")), "strict overlap must conflict: {pairs:?}");
    for edge in &pairs {
        assert!(!((edge.0 == "p" && edge.1 == "r") || (edge.0 == "r" && edge.1 == "p")), "touching intervals must not conflict: {pairs:?}");
    }
    // 换成独立空间后同一坐标不再冲突 / The same coordinates in separate spaces no longer conflict.
    problem.intervals = Some(vec![
        BppLoadingInterval { id: "p".into(), space_id: "deck-a".into(), start: quantity(0.0), end: quantity(2.0) },
        BppLoadingInterval { id: "q".into(), space_id: "deck-b".into(), start: quantity(1.0), end: quantity(3.0) },
    ]);
    assert!(problem.domain().unwrap().conflicts.is_empty());
    // 未知舱位必须是错误 / An unknown interval position must fail.
    problem.intervals = Some(vec![BppLoadingInterval { id: "zz".into(), space_id: "deck".into(), start: quantity(0.0), end: quantity(1.0) }]);
    assert_eq!(problem.domain().unwrap_err().code, "InvalidInput");
}

fn quantity(value: f64) -> PhysicalQuantity { PhysicalQuantity { value, unit: QuantityUnit::Meter } }

/// 二维矩形由原几何内核精确判交：仅同空间真实重叠成边，边贴不算，跨空间不算。
/// 2D rectangles are judged by the original exact geometry kernel: only real overlap inside one space creates an
/// edge, edge contact does not, and separate spaces never conflict.
#[test]
fn rectangles_add_only_exact_same_space_overlaps() {
    let mut problem = problem();
    problem.conflicts.clear();
    problem.intervals = None;
    let rect = |id: &str, space: &str, x: f64, y: f64, w: f64, h: f64| BppLoadingRectangle {
        id: id.into(), space_id: space.into(), x: length(x), y: length(y), width: length(w), height: length(h) };
    problem.rectangles = Some(vec![rect("p", "deck", 0.0, 0.0, 2.0, 2.0), rect("q", "deck", 1.0, 1.0, 2.0, 2.0),
        // 与 p 共边但不重叠 / Shares an edge with p without overlapping.
        rect("r", "deck", 2.0, 0.0, 1.0, 1.0)]);
    let pairs = problem.domain().unwrap().conflicts.iter().map(|edge| (edge.first.clone(), edge.second.clone())).collect::<Vec<_>>();
    assert!(pairs.iter().any(|(a, b)| (a == "p" && b == "q") || (a == "q" && b == "p")), "exact overlap must conflict: {pairs:?}");
    assert!(!pairs.iter().any(|(a, b)| (a == "p" && b == "r") || (a == "r" && b == "p")), "edge contact must not conflict: {pairs:?}");
    // 独立空间同坐标不冲突 / Identical coordinates in separate spaces never conflict.
    problem.rectangles = Some(vec![rect("p", "deck-a", 0.0, 0.0, 2.0, 2.0), rect("q", "deck-b", 1.0, 1.0, 2.0, 2.0),
        rect("r", "deck-a", 5.0, 5.0, 1.0, 1.0)]);
    assert!(problem.domain().unwrap().conflicts.is_empty());
    // 退化矩形与非正尺寸必须由原工厂拒绝 / Degenerate or non-positive extents must be rejected by the original factory.
    problem.rectangles = Some(vec![rect("p", "deck", 0.0, 0.0, 0.0, 2.0), rect("q", "deck", 1.0, 1.0, 2.0, 2.0),
        rect("r", "deck", 5.0, 5.0, 1.0, 1.0)]);
    assert_eq!(problem.domain().unwrap_err().code, "InvalidInput");
    // 同时给区间与矩形必须报错，不猜 / Supplying both intervals and rectangles must fail instead of guessing.
    problem.rectangles = Some(vec![rect("p", "deck", 0.0, 0.0, 2.0, 2.0), rect("q", "deck", 1.0, 1.0, 2.0, 2.0), rect("r", "deck", 3.0, 0.0, 1.0, 1.0)]);
    problem.intervals = Some(vec![BppLoadingInterval { id: "p".into(), space_id: "deck".into(), start: length(0.0), end: length(1.0) }]);
    assert_eq!(problem.domain().unwrap_err().code, "InvalidInput");
}

fn length(value: f64) -> PhysicalQuantity { PhysicalQuantity { value, unit: QuantityUnit::Meter } }

pub(crate) fn solution() -> BppLoadingSolution {
    BppLoadingSolution { assignments: BTreeMap::from([("a".into(), "p".into()), ("b".into(), "r".into())]), occupied_positions: vec!["p".into(), "r".into()], selected_layouts: BTreeMap::from([("g".into(), "outer".into())]) }
}

fn report(context: &BppLoadingContext) -> SolveReport<f64, u64> {
    let model = crate::decode_model(&context.linear_model()).unwrap();
    let mut solution = SolveSolution::vector(context.solution_values(&solution()).unwrap()); solution.objective_value = Some(0.0);
    let report = SolveReport::builder(ProblemStatus::Feasible, TerminationReason::Completed).solution(solution).proof(SolveProof::optimality())
        .fingerprints(SolveFingerprints { model: Some(linear_model_fingerprint(&model).unwrap()), ..Default::default() })
        .model_mapping(Some(linear_model_mapping(&model).unwrap())).build().unwrap();
    serde_json::from_str(&serde_json::to_string(&report).unwrap()).unwrap()
}

#[test]
fn original_registration_exports_stable_sparse_models_and_full_u64_capacities() {
    let first = BppLoadingContext::new(problem()).unwrap(); let restored = BppLoadingContext::from_artifact(&first.artifact()).unwrap();
    assert_eq!(first.artifact().model_json, restored.artifact().model_json);
    assert!(first.artifact().model_json.contains("18446744073709551615"));
    assert_eq!(first.linear_model().model_json, restored.linear_model().model_json);
    assert_eq!(first.linear_model().fingerprint, restored.linear_model().fingerprint);
    let model = crate::decode_model(&first.linear_model()).unwrap(); assert_eq!(model.num_variables(), 9);
    assert!(model.c.iter().all(|coefficient| *coefficient == 0.0));
    assert!(model.A.rows.iter().flat_map(|row| &row.entries).all(|(_, value)| value.abs() < 100.0));
    assert!(!first.conflict_cliques().iter().any(|clique| clique.contains(&"p".into()) && clique.contains(&"r".into())));
    let result = first.verify_solution(&solution()).unwrap();
    assert_eq!(result.loaded_count, "2"); assert_eq!(result.loaded_weight.value, 8.0);
    assert_eq!(result.position_counts["q"], "0"); assert_eq!(result.position_weights["p"].value, 4.0);
    assert_eq!(result.source_fingerprint, first.artifact().fingerprint);
}

#[test]
fn source_codec_rejects_foreign_or_noncanonical_identities_and_versions() {
    let context = BppLoadingContext::new(problem()).unwrap();
    for variant in 0..4 {
        let mut artifact = context.artifact();
        match variant { 0 => artifact.fingerprint = "foreign".into(), 1 => artifact.model_json.push(' '),
            2 => artifact.model_json = artifact.model_json.replace("\"1.0\"", "\"2.0\""), _ => artifact.model_json = " ".repeat(MAX_BYTES + 1) }
        assert_eq!(BppLoadingContext::from_artifact(&artifact).err().unwrap().code, match variant { 2 => "UnsupportedProtocolVersion", 3 => "PayloadTooLarge", _ => "FingerprintMismatch" });
    }
}

#[test]
fn invalid_catalogs_sparse_duplicates_units_counts_and_budget_are_rejected() {
    for variant in 0..21 {
        let mut input = problem();
        match variant {
            0 => input.items[1].id = "a".into(), 1 => input.allowed_assignments.push(input.allowed_assignments[0].clone()),
            2 => input.conflicts.push(BppLoadingConflict { first: "q".into(), second: "p".into() }),
            3 => input.allowed_assignments[0].position_id = "foreign".into(), 4 => input.items[0].fixed_position_id = Some("r".into()),
            5 => input.positions[0].max_count = "0".into(), 6 => input.positions[0].max_count = "01".into(),
            7 => input.positions[0].max_count = "18446744073709551616".into(), 8 => input.items[0].weight.unit = QuantityUnit::Meter,
            9 => input.items[0].weight.value = f64::from_bits(1), 10 => input.items[0].weight.value = -1.0,
            11 => input.positions[0].max_weight.value = f64::NAN, 12 => input.weight_unit = QuantityUnit::Meter,
            13 => input.layout_groups[0].layouts[0].position_ids.push("p".into()), 14 => input.layout_groups[0].position_ids.pop().map(|_| ()).unwrap(),
            15 => input.conflicts[0].second = "p".into(), 16 => { input.positions[0].fixed_occupied = true; input.positions[1].fixed_occupied = true; },
            17 => { input.items[0].fixed_position_id = Some("p".into()); input.positions[0].max_weight.value = 1.0; },
            18 => input.positions.resize(129, input.positions[0].clone()),
            19 => { input.weight_unit = QuantityUnit::Gram; input.items[1].weight.value = f64::MAX; },
            _ => input.conflicts[0].second = "foreign".into(),
        }
        assert!(BppLoadingContext::new(input).is_err(), "variant {variant}");
    }
}

#[test]
fn full_model_and_domain_rules_reject_incomplete_conflicting_or_unrepresentable_plans() {
    let context = BppLoadingContext::new(problem()).unwrap();
    for variant in 0..7 {
        let mut plan = solution();
        match variant { 0 => { plan.assignments.remove("a"); }, 1 => plan.occupied_positions.push("q".into()),
            2 => { plan.selected_layouts.insert("g".into(), "middle".into()); }, 3 => { plan.assignments.insert("foreign".into(), "p".into()); },
            4 => { plan.assignments.insert("b".into(), "p".into()); }, 5 => plan.occupied_positions.push("p".into()), _ => { plan.selected_layouts.clear(); } }
        assert!(context.verify_solution(&plan).is_err(), "variant {variant}");
    }
    let values = context.solution_values(&solution()).unwrap();
    for variant in 0..5 {
        let mut values = values.clone();
        match variant { 0 => { values.pop(); }, 1 => values.push(0.0), 2 => values[0] = 0.5, 3 => values[0] = f64::NAN, _ => values[0] = 0.0 }
        assert!(context.extract_solution(&values).is_err());
    }
    let mut input = problem(); input.items.clear(); input.allowed_assignments.clear(); input.positions[0].fixed_occupied = true; input.positions[0].enabled = false;
    let context = BppLoadingContext::new(input).unwrap(); let mut plan = solution(); plan.assignments.clear();
    assert_eq!(context.verify_solution(&plan).unwrap().loaded_count, "0");
}

#[test]
fn actual_report_identity_full_vector_objective_and_exact_metadata_are_checked() {
    let context = BppLoadingContext::new(problem()).unwrap(); let mut report = report(&context);
    report.statistics.nodes = Some(u64::MAX); let json = serde_json::to_string(&report).unwrap();
    let read = context.read_report(&json).unwrap(); assert_eq!(read.json, json); assert_eq!(read.solution.unwrap().loaded_count, "2");
    assert!(read.json.contains("18446744073709551615"));
    let mut equivalent = problem(); equivalent.items[0].id = "aa".into();
    for key in &mut equivalent.allowed_assignments { if key.item_id == "a" { key.item_id = "aa".into(); } }
    let equivalent = BppLoadingContext::new(equivalent).unwrap();
    assert_ne!(equivalent.linear_model().fingerprint, context.linear_model().fingerprint);
    assert_eq!(equivalent.read_report(&json).err().unwrap().code, "FingerprintMismatch");
    for variant in 0..7 {
        let mut report = report.clone();
        match variant { 0 => report.fingerprints.model = None, 1 => report.model_mapping = None, 2 => report.solution.as_mut().unwrap().values.clear(),
            3 => report.solution.as_mut().unwrap().values[0] = 0.0, 4 => report.solution.as_mut().unwrap().objective_value = Some(1.0),
            5 => { report.solution.as_mut().unwrap().stable_values.insert("foreign".into(), 1.0); }, _ => report.schema_version = "2.0".into() }
        assert!(context.read_report(&serde_json::to_string(&report).unwrap()).is_err(), "variant {variant}");
    }
    let stopped = SolveReport::<f64>::builder(ProblemStatus::Unknown, TerminationReason::TimeLimit)
        .fingerprints(report.fingerprints).model_mapping(report.model_mapping).build().unwrap();
    let stopped = context.read_report(&serde_json::to_string(&stopped).unwrap()).unwrap();
    assert!(stopped.solution.is_none()); assert_eq!(stopped.summary.termination_reason, "TIME_LIMIT");
}

#[test]
fn fixed_binary_exchange_preserves_both_bounds_and_rejects_fractional_or_unbounded_ranges() {
    use ospf_rust_framework::solver::remote::{SerializedVariable, SerializedVariableType};
    for (lower, upper) in [(0.0, 0.0), (0.0, 1.0), (1.0, 1.0)] {
        let mut model = ospf_rust_core::model::MetaModel::new("fixed-binary");
        crate::register_linear_variable(&mut model, &SerializedVariable { stable_id: "x".into(), index: 0, name: "x".into(),
            lower_bound: lower, upper_bound: upper, variable_type: SerializedVariableType::Binary }).unwrap();
        let compiled = model.try_to_linear_triad_model().unwrap(); let restored = crate::decode_model(&crate::encode_model(&compiled).unwrap()).unwrap();
        assert_eq!(restored.lb, vec![lower]); assert_eq!(restored.ub, vec![upper]); assert!(restored.var_types[0].is_integer());
    }
    for (lower, upper) in [(0.5, 1.0), (0.0, f64::INFINITY), (-1.0, 1.0), (1.0, 0.0)] {
        let mut model = ospf_rust_core::model::MetaModel::new("invalid-binary");
        assert!(crate::register_linear_variable(&mut model, &SerializedVariable { stable_id: "x".into(), index: 0, name: "x".into(),
            lower_bound: lower, upper_bound: upper, variable_type: SerializedVariableType::Binary }).is_err());
    }
}
