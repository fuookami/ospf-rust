//! 实际子模型与源 context 的数学契约比较，不替换实际报告身份。 / Compare actual children with source-context mathematics without replacing actual report identity.

use std::collections::BTreeMap;
use ospf_rust_core::model::intermediate::LinearTriadModel;
use crate::{Result, invalid};

fn column_names(model: &LinearTriadModel) -> Result<BTreeMap<String, usize>> {
    let mut columns = BTreeMap::new();
    for (index, token) in model.variables.iter().enumerate() {
        if columns.insert(token.name().to_owned(), index).is_some() { return Err(invalid("duplicate native child column name")); }
    }
    Ok(columns)
}

pub(crate) fn compare_source_model(actual: &LinearTriadModel, expected: &LinearTriadModel) -> Result<Vec<usize>> {
    let actual_columns = column_names(actual)?;
    let expected_columns = column_names(expected)?;
    if actual_columns.len() != expected_columns.len() || actual.num_constraints() != expected.num_constraints()
        || actual.objective_category != expected.objective_category
        || actual.constraint_group_ids != expected.constraint_group_ids || actual.constraint_lazy_flags != expected.constraint_lazy_flags
        || actual.constraint_priorities != expected.constraint_priorities || actual.constraint_args != expected.constraint_args
        || actual.constraint_source_symbol_ids != expected.constraint_source_symbol_ids {
        return Err(invalid("native child shape, objective sense or execution metadata differs from the source context"));
    }
    let permutation = expected.variables.iter().map(|token| actual_columns.get(token.name()).copied()
        .ok_or_else(|| invalid("missing or foreign native child plan column"))).collect::<Result<Vec<_>>>()?;
    for (expected_column, &actual_column) in permutation.iter().enumerate() {
        if actual.variables[actual_column].var_type() != expected.variables[expected_column].var_type()
            || actual.var_types[actual_column] != expected.var_types[expected_column]
            || actual.lb[actual_column] != expected.lb[expected_column] || actual.ub[actual_column] != expected.ub[expected_column]
            || actual.c[actual_column] != expected.c[expected_column] {
            return Err(invalid("native child plan type, bounds or objective differs from the source context"));
        }
    }
    // 局部行身份不跨 arena 比较；逐行完整数学契约严格相等，原报告身份仍只由实际模型验证。
    // Do not compare arena-local row identities; require exact complete row mathematics, while validating original report identity against the actual model only.
    for (row, expected_row) in expected.A.rows.iter().enumerate() {
        let mut actual_values = vec![0.0; actual.num_variables()];
        let mut expected_values = vec![0.0; expected.num_variables()];
        for &(column, value) in &actual.A.rows[row].entries { actual_values[column] += value; }
        for &(column, value) in &expected_row.entries { expected_values[column] += value; }
        if actual.b[row] != expected.b[row] || permutation.iter().enumerate().any(|(column, &other)| actual_values[other] != expected_values[column]) {
            return Err(invalid("native child rows differ from the source context"));
        }
    }
    Ok(permutation)
}
