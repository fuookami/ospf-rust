use ospf_rust_core::solver::{evaluate_linear_solution, linear_model_mapping};
use super::*;

fn example() -> LinearTriadModel {
    let mut basic = BasicLinearTriadModel::new("actual-child");
    for (column, kind) in [VariableType::Binary, VariableType::Ternary, VariableType::BalancedTernary, VariableType::Percentage,
        VariableType::Integer, VariableType::UInteger, VariableType::Continuous, VariableType::UContinuous].into_iter().enumerate() {
        let (lower, upper) = kind.default_bounds();
        let token = Token::new(AnyVariable::new(TokenVariableData { id: new_standalone_id(), index: column, name: format!("x{column}"),
            display_name: None, var_type: kind, lower_bound: lower, upper_bound: upper }), column);
        basic.add_variable(token);
    }
    basic.add_constraint_with_metadata(SparseVector { entries: vec![(5, -1.0), (5, -2.0), (7, 1.0)] }, -2.0,
        "model-local-row:9007199254740993:demand".into(), Some(u64::MAX), true, 17, Some("host-policy".into()), Some(u64::MAX - 1));
    let mut model = LinearTriadModel::from_basic(basic);
    model.c = vec![1.0; 8];
    model.objective_category = ObjectiveCategory::Maximum;
    model
}

#[test]
fn exact_types_metadata_sparse_entries_infinite_bounds_and_stable_identity_roundtrip() {
    let model = example();
    let artifact = encode_compiled_linear_model(&model).unwrap();
    assert!(artifact.model_json.contains("18446744073709551615"));
    assert!(artifact.model_json.contains("Infinity"));
    let restored = decode_compiled_linear_model(&artifact).unwrap();
    assert_eq!(linear_model_fingerprint(&restored).unwrap(), linear_model_fingerprint(&model).unwrap());
    assert_eq!(linear_model_mapping(&restored).unwrap(), linear_model_mapping(&model).unwrap());
    assert_eq!(restored.var_types, model.var_types);
    assert_eq!(restored.variables.iter().map(Token::var_type).collect::<Vec<_>>(), model.var_types);
    assert_eq!(restored.constraint_names, model.constraint_names);
    assert_eq!(restored.constraint_group_ids, model.constraint_group_ids);
    assert_eq!(restored.constraint_lazy_flags, model.constraint_lazy_flags);
    assert_eq!(restored.constraint_priorities, model.constraint_priorities);
    assert_eq!(restored.constraint_args, model.constraint_args);
    assert_eq!(restored.constraint_source_symbol_ids, model.constraint_source_symbol_ids);
    assert_eq!(restored.A.rows[0].entries, model.A.rows[0].entries);
    assert_eq!(restored.variables.iter().map(Token::id).collect::<std::collections::BTreeSet<_>>().len(), 8);
    assert_ne!(restored.variables[0].id(), model.variables[0].id());
    assert_eq!(restored.find_variable_index(restored.variables[7].id()), Some(7));
    assert_eq!(encode_compiled_linear_model(&restored).unwrap(), artifact);
    let values = vec![1.0; 8];
    let original = evaluate_linear_solution(&model, &values, 1e-7).unwrap();
    let decoded = evaluate_linear_solution(&restored, &values, 1e-7).unwrap();
    assert_eq!(original.constraint_evaluations, decoded.constraint_evaluations);
}

#[test]
fn absent_metadata_and_independent_solver_relaxation_are_not_normalized_away() {
    let original = example();
    let mut model = original.linear_relaxed();
    assert_eq!(linear_model_fingerprint(&model).unwrap(), linear_model_fingerprint(&original).unwrap());
    assert_ne!(encode_compiled_linear_model(&model).unwrap().content_fingerprint,
        encode_compiled_linear_model(&original).unwrap().content_fingerprint);
    model.basic.constraint_names.clear();
    model.basic.constraint_group_ids.clear();
    model.basic.constraint_lazy_flags.clear();
    model.basic.constraint_priorities.clear();
    model.basic.constraint_args.clear();
    model.basic.constraint_source_symbol_ids.clear();
    let artifact = encode_compiled_linear_model(&model).unwrap();
    let decoded = decode_compiled_linear_model(&artifact).unwrap();
    assert_eq!(decoded.var_types, model.var_types);
    assert_eq!(decoded.variables[5].var_type(), VariableType::UInteger);
    assert_eq!(decoded.var_types[5], VariableType::UContinuous);
    assert!(decoded.constraint_names.is_empty()); assert!(decoded.constraint_lazy_flags.is_empty());
    assert_eq!(linear_model_fingerprint(&decoded).unwrap(), artifact.model_fingerprint);
    assert_eq!(encode_compiled_linear_model(&decoded).unwrap(), artifact);
    assert_eq!(encode_compiled_linear_model(&LinearTriadModel::new("empty")).unwrap().schema_version, "1.0");
}

#[test]
fn version_digests_mapping_unknown_fields_and_noncanonical_or_invalid_payloads_fail() {
    let artifact = encode_compiled_linear_model(&example()).unwrap();
    let mut corrupt = artifact.clone(); corrupt.schema_version = "2.0".into(); assert!(decode_compiled_linear_model(&corrupt).is_err());
    corrupt = artifact.clone(); corrupt.model_fingerprint.value = "foreign".into(); assert!(decode_compiled_linear_model(&corrupt).is_err());
    corrupt = artifact.clone(); corrupt.content_fingerprint.value = "foreign".into(); assert!(decode_compiled_linear_model(&corrupt).is_err());
    corrupt = artifact.clone(); corrupt.model_json.push(' '); assert!(decode_compiled_linear_model(&corrupt).is_err());
    for variant in 0..13 {
        let mut input: serde_json::Value = serde_json::from_str(&artifact.model_json).unwrap();
        match variant {
            0 => input["tokenTypes"][5] = "Integer".into(),
            1 => input["solverTypes"][5] = "Continuous".into(),
            2 => input["constraintGroupIds"][0] = 1.into(),
            3 => input["model"]["constraints"][0]["name"] = "different".into(),
            4 => input["model"]["constraints"][0]["stableId"] = "foreign".into(),
            5 => input["model"]["variables"][0]["type"] = "INTEGER".into(),
            6 => input["model"]["variables"][0]["index"] = 8.into(),
            7 => input["model"]["constraints"][0]["cells"][0]["colIndex"] = 100.into(),
            8 => input["model"]["objective"]["constant"] = 1.into(),
            9 => input["model"]["variables"][0]["lowerBound"] = serde_json::Value::Null,
            10 => input["tokenTypes"] = serde_json::json!([]),
            11 => input["unexpected"] = true.into(),
            _ => input["model"]["unexpected"] = true.into(),
        }
        let mut corrupt = artifact.clone(); corrupt.model_json = serde_json::to_string(&input).unwrap();
        corrupt.content_fingerprint = sha256_fingerprint("ospf.compiled-linear-model.v1", corrupt.model_json.as_bytes());
        assert!(decode_compiled_linear_model(&corrupt).is_err(), "variant {variant}");
    }
    let mut invalid = example(); invalid.c[0] = f64::NAN; assert!(encode_compiled_linear_model(&invalid).is_err());
    invalid = example(); invalid.A.rows[0].entries[0].0 = 20; assert!(encode_compiled_linear_model(&invalid).is_err());
    corrupt = artifact; corrupt.model_json = " ".repeat(MAX_BYTES + 1); assert!(decode_compiled_linear_model(&corrupt).is_err());
}
